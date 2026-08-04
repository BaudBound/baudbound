use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use baudbound_script::{
    validate_anonymous_public_https_url, validate_public_https_package_url,
    validate_public_https_repository_url,
};
use baudbound_security::all_network_addresses_are_public;
use reqwest::{
    StatusCode,
    blocking::{Client, Request, Response},
    header::{
        AUTHORIZATION, COOKIE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, LOCATION,
        PROXY_AUTHORIZATION,
    },
};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use thiserror::Error;
use url::Url;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
pub(crate) const MAX_REPOSITORY_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteResourceKind {
    Package,
    Repository,
}

impl RemoteResourceKind {
    fn validate_path(self, url: &Url) -> Result<(), RemoteFetchError> {
        let file_name = url
            .path_segments()
            .and_then(Iterator::last)
            .filter(|value| !value.is_empty())
            .ok_or(RemoteFetchError::InvalidPath(self))?;
        let valid = match self {
            Self::Package => file_name.to_ascii_lowercase().ends_with(".bbs"),
            Self::Repository => file_name == "repository.json",
        };
        if valid {
            Ok(())
        } else {
            Err(RemoteFetchError::InvalidPath(self))
        }
    }
}

#[derive(Debug)]
pub(crate) struct RemoteDownload {
    pub(crate) file: NamedTempFile,
    pub(crate) final_url: Url,
    pub(crate) original_url: Url,
    pub(crate) redirect_urls: Vec<Url>,
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

#[derive(Debug)]
pub(crate) enum RepositoryFetchResult {
    Modified(Box<RepositoryFetchModified>),
    NotModified,
}

#[derive(Debug)]
pub(crate) struct RepositoryFetchModified {
    pub(crate) bytes: Vec<u8>,
    pub(crate) etag: Option<String>,
    pub(crate) final_url: Url,
    pub(crate) last_modified: Option<String>,
    pub(crate) original_url: Url,
    pub(crate) redirect_urls: Vec<Url>,
}

#[derive(Debug, Error)]
pub(crate) enum RemoteFetchError {
    #[error("the remote URL is invalid")]
    InvalidUrl,
    #[error(
        "remote URLs must use HTTPS and cannot contain credentials, query strings, or fragments"
    )]
    UnsafeUrl,
    #[error("the remote URL does not name the expected {0:?} file")]
    InvalidPath(RemoteResourceKind),
    #[error("the remote host could not be resolved")]
    Resolve,
    #[error("the remote host resolves to a local or otherwise restricted network address")]
    RestrictedDestination,
    #[error("the remote server returned a redirect without a valid Location header")]
    InvalidRedirect,
    #[error("the remote server returned too many redirects")]
    TooManyRedirects,
    #[error("the remote server returned a redirect loop")]
    RedirectLoop,
    #[error("the remote server returned HTTP {0}")]
    HttpStatus(StatusCode),
    #[error("the remote request failed")]
    Request,
    #[error("the remote response exceeds the {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("the remote response ended before its declared Content-Length was received")]
    Truncated,
    #[error("the remote package download was cancelled")]
    Cancelled,
    #[error("{0}")]
    Blacklisted(String),
    #[error("failed to create or write the protected temporary download: {0}")]
    TemporaryFile(String),
}

#[derive(Debug, Clone)]
pub(crate) struct RemoteFetchService {
    package_limit: u64,
}

impl RemoteFetchService {
    pub(crate) fn new(package_limit: u64) -> Self {
        Self { package_limit }
    }

    pub(crate) fn fetch_repository(&self, value: &str) -> Result<(Vec<u8>, Url), RemoteFetchError> {
        match self.fetch_repository_with_progress(value, &mut |_, _| true)? {
            RepositoryFetchResult::Modified(result) => Ok((result.bytes, result.final_url)),
            RepositoryFetchResult::NotModified => Err(RemoteFetchError::Request),
        }
    }

    pub(crate) fn fetch_repository_with_progress(
        &self,
        value: &str,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<RepositoryFetchResult, RemoteFetchError> {
        self.fetch_repository_conditional(value, None, None, progress)
    }

    pub(crate) fn fetch_repository_conditional(
        &self,
        value: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<RepositoryFetchResult, RemoteFetchError> {
        let url = validate_url(value, RemoteResourceKind::Repository)?;
        let (mut response, provenance) = self.send(url, etag, last_modified)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            ensure_continues(progress, 0, Some(0))?;
            return Ok(RepositoryFetchResult::NotModified);
        }
        let response_etag = header_value(&response, ETAG);
        let response_last_modified = header_value(&response, LAST_MODIFIED);
        let expected_length = response.content_length();
        let bytes = read_bounded(
            &mut response,
            expected_length,
            MAX_REPOSITORY_BYTES,
            progress,
        )?;
        Ok(RepositoryFetchResult::Modified(Box::new(
            RepositoryFetchModified {
                bytes,
                etag: response_etag,
                final_url: provenance.final_url,
                last_modified: response_last_modified,
                original_url: provenance.original_url,
                redirect_urls: provenance.redirect_urls,
            },
        )))
    }

    pub(crate) fn fetch_package_with_progress(
        &self,
        value: &str,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<RemoteDownload, RemoteFetchError> {
        let url = validate_url(value, RemoteResourceKind::Package)?;
        let (mut response, provenance) = self.send(url, None, None)?;
        let expected_length = response.content_length();
        if expected_length.is_some_and(|length| length > self.package_limit) {
            return Err(RemoteFetchError::TooLarge {
                limit: self.package_limit,
            });
        }

        let mut file = tempfile::Builder::new()
            .prefix("baudbound-remote-")
            .suffix(".bbs")
            .tempfile()
            .map_err(|error| RemoteFetchError::TemporaryFile(error.to_string()))?;
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        ensure_continues(progress, size, expected_length)?;
        loop {
            let read = response
                .read(&mut buffer)
                .map_err(|_| RemoteFetchError::Request)?;
            if read == 0 {
                break;
            }
            size = size.saturating_add(read as u64);
            if size > self.package_limit {
                return Err(RemoteFetchError::TooLarge {
                    limit: self.package_limit,
                });
            }
            digest.update(&buffer[..read]);
            file.write_all(&buffer[..read])
                .map_err(|error| RemoteFetchError::TemporaryFile(error.to_string()))?;
            ensure_continues(progress, size, expected_length)?;
        }
        if expected_length.is_some_and(|length| length != size) {
            return Err(RemoteFetchError::Truncated);
        }
        file.as_file_mut()
            .sync_all()
            .map_err(|error| RemoteFetchError::TemporaryFile(error.to_string()))?;
        let sha256 = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(RemoteDownload {
            file,
            final_url: provenance.final_url,
            original_url: provenance.original_url,
            redirect_urls: provenance.redirect_urls,
            sha256,
            size,
        })
    }

    fn send(
        &self,
        mut url: Url,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(Response, RemoteProvenance), RemoteFetchError> {
        let original_url = url.clone();
        let mut redirect_urls = Vec::new();
        let mut visited_urls = BTreeSet::from([url.as_str().to_owned()]);
        for redirect_count in 0..=MAX_REDIRECTS {
            if let Some(blacklist) = crate::blacklist::global() {
                blacklist
                    .ensure_url_distribution_allowed(&url)
                    .map_err(|error| RemoteFetchError::Blacklisted(error.to_string()))?;
            }
            let addresses = resolve_public_addresses(&url)?;
            let host = url.host_str().ok_or(RemoteFetchError::InvalidUrl)?;
            let client = pinned_client(host, &addresses)?;
            let request = build_anonymous_request(
                &client,
                url.clone(),
                (redirect_count == 0).then_some(etag).flatten(),
                (redirect_count == 0).then_some(last_modified).flatten(),
            )?;
            let response = client
                .execute(request)
                .map_err(|_| RemoteFetchError::Request)?;
            if response.status().is_redirection() {
                if redirect_count == MAX_REDIRECTS {
                    return Err(RemoteFetchError::TooManyRedirects);
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(RemoteFetchError::InvalidRedirect)?;
                let redirect_url = url
                    .join(location)
                    .map_err(|_| RemoteFetchError::InvalidRedirect)?;
                validate_transport_url(&redirect_url)?;
                if !visited_urls.insert(redirect_url.as_str().to_owned()) {
                    return Err(RemoteFetchError::RedirectLoop);
                }
                url = redirect_url;
                redirect_urls.push(url.clone());
                continue;
            }
            if !response.status().is_success() && response.status() != StatusCode::NOT_MODIFIED {
                return Err(RemoteFetchError::HttpStatus(response.status()));
            }
            return Ok((
                response,
                RemoteProvenance {
                    final_url: url,
                    original_url,
                    redirect_urls,
                },
            ));
        }
        Err(RemoteFetchError::TooManyRedirects)
    }
}

struct RemoteProvenance {
    final_url: Url,
    original_url: Url,
    redirect_urls: Vec<Url>,
}

fn header_value(response: &Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn validate_url(value: &str, kind: RemoteResourceKind) -> Result<Url, RemoteFetchError> {
    let url = match kind {
        RemoteResourceKind::Package => {
            validate_public_https_package_url(value).map(|url| url.as_url().clone())
        }
        RemoteResourceKind::Repository => {
            validate_public_https_repository_url(value).map(|url| url.as_url().clone())
        }
    }
    .map_err(|_| RemoteFetchError::UnsafeUrl)?;
    kind.validate_path(&url)?;
    Ok(url)
}

fn validate_transport_url(url: &Url) -> Result<(), RemoteFetchError> {
    validate_anonymous_public_https_url(url.as_str())
        .map(|_| ())
        .map_err(|_| RemoteFetchError::UnsafeUrl)
}

fn resolve_public_addresses(url: &Url) -> Result<Vec<SocketAddr>, RemoteFetchError> {
    let host = url.host_str().ok_or(RemoteFetchError::InvalidUrl)?;
    let port = url
        .port_or_known_default()
        .ok_or(RemoteFetchError::InvalidUrl)?;
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| RemoteFetchError::Resolve)?
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(RemoteFetchError::Resolve);
    }
    if !all_network_addresses_are_public(addresses.iter().map(|address| address.ip())) {
        return Err(RemoteFetchError::RestrictedDestination);
    }
    Ok(addresses)
}

fn pinned_client(host: &str, addresses: &[SocketAddr]) -> Result<Client, RemoteFetchError> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .https_only(true)
        .no_proxy()
        .referer(false)
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, addresses)
        .user_agent(concat!("BaudBound/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| RemoteFetchError::Request)
}

fn build_anonymous_request(
    client: &Client,
    url: Url,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<Request, RemoteFetchError> {
    let mut request = client.get(url);
    if let Some(etag) = etag {
        request = request.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = last_modified {
        request = request.header(IF_MODIFIED_SINCE, last_modified);
    }
    let request = request.build().map_err(|_| RemoteFetchError::Request)?;
    if [AUTHORIZATION, PROXY_AUTHORIZATION, COOKIE]
        .iter()
        .any(|name| request.headers().contains_key(name))
    {
        return Err(RemoteFetchError::UnsafeUrl);
    }
    Ok(request)
}

fn read_bounded(
    reader: &mut dyn Read,
    expected_length: Option<u64>,
    limit: u64,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<Vec<u8>, RemoteFetchError> {
    if expected_length.is_some_and(|length| length > limit) {
        return Err(RemoteFetchError::TooLarge { limit });
    }
    let mut bytes = Vec::with_capacity(
        expected_length
            .unwrap_or_default()
            .min(limit)
            .try_into()
            .unwrap_or_default(),
    );
    ensure_continues(progress, 0, expected_length)?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| RemoteFetchError::Request)?;
        if read == 0 {
            break;
        }
        if (bytes.len() as u64).saturating_add(read as u64) > limit {
            return Err(RemoteFetchError::TooLarge { limit });
        }
        bytes.extend_from_slice(&buffer[..read]);
        ensure_continues(progress, bytes.len() as u64, expected_length)?;
    }
    if expected_length.is_some_and(|length| length != bytes.len() as u64) {
        return Err(RemoteFetchError::Truncated);
    }
    Ok(bytes)
}

fn ensure_continues(
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    transferred: u64,
    total: Option<u64>,
) -> Result<(), RemoteFetchError> {
    if progress(transferred, total) {
        Ok(())
    } else {
        Err(RemoteFetchError::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_resource_urls() {
        assert!(
            validate_url(
                "https://example.com/repository.json",
                RemoteResourceKind::Repository
            )
            .is_ok()
        );
        assert!(
            validate_url(
                "https://example.com/releases/test.bbs?token=value",
                RemoteResourceKind::Package
            )
            .is_err()
        );
        assert!(validate_url("http://example.com/test.bbs", RemoteResourceKind::Package).is_err());
        assert!(
            validate_url(
                "https://user@example.com/test.bbs",
                RemoteResourceKind::Package
            )
            .is_err()
        );
        assert!(
            validate_url(
                "https://example.com/other.json",
                RemoteResourceKind::Repository
            )
            .is_err()
        );
        assert!(
            validate_url(
                "https://example.com/repository.json",
                RemoteResourceKind::Package
            )
            .is_err()
        );
    }

    #[test]
    fn redirects_remain_anonymous_without_requiring_the_original_file_name() {
        assert!(
            validate_transport_url(
                &Url::parse("https://objects.example.com/download/opaque-id").unwrap()
            )
            .is_ok()
        );
        assert!(
            validate_transport_url(
                &Url::parse("https://objects.example.com/download/opaque-id?signature=value")
                    .unwrap()
            )
            .is_err()
        );
        assert!(
            validate_transport_url(&Url::parse("http://example.com/download").unwrap()).is_err()
        );
        assert!(
            validate_transport_url(&Url::parse("https://user@example.com/download").unwrap())
                .is_err()
        );
        assert!(
            validate_transport_url(&Url::parse("https://example.com/download#fragment").unwrap())
                .is_err()
        );
    }

    #[test]
    fn rejects_non_public_addresses() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "64:ff9b::c0a8:101",
            "2002:c0a8:0101::1",
            "2001:db8::1",
        ] {
            assert!(!all_network_addresses_are_public([address
                .parse()
                .expect("test address should parse")]));
        }
        assert!(all_network_addresses_are_public([
            "1.1.1.1".parse().unwrap(),
            "2606:4700:4700::1111".parse().unwrap(),
        ]));
        assert!(!all_network_addresses_are_public([
            "1.1.1.1".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ]));
    }

    #[test]
    fn bounded_reads_report_progress_and_support_cancellation() {
        let payload = vec![7_u8; 128 * 1024];
        let mut reader = std::io::Cursor::new(payload);
        let mut observations = Vec::new();
        let result = read_bounded(
            &mut reader,
            Some(128 * 1024),
            256 * 1024,
            &mut |transferred, total| {
                observations.push((transferred, total));
                transferred < 64 * 1024
            },
        );

        assert!(matches!(result, Err(RemoteFetchError::Cancelled)));
        assert_eq!(observations[0], (0, Some(128 * 1024)));
        assert_eq!(observations[1], (64 * 1024, Some(128 * 1024)));
    }

    #[test]
    fn bounded_reads_reject_responses_over_the_limit() {
        let mut reader = std::io::Cursor::new(vec![0_u8; 8]);
        let result = read_bounded(&mut reader, Some(8), 4, &mut |_, _| true);
        assert!(matches!(
            result,
            Err(RemoteFetchError::TooLarge { limit: 4 })
        ));
    }

    #[test]
    fn production_repository_requests_never_include_credentials() {
        let client = pinned_client("example.com", &["1.1.1.1:443".parse().unwrap()]).unwrap();
        let request = build_anonymous_request(
            &client,
            Url::parse("https://example.com/repository.json").unwrap(),
            Some("repository-etag"),
            Some("Tue, 04 Aug 2026 10:00:00 GMT"),
        )
        .unwrap();
        assert!(!request.headers().contains_key(AUTHORIZATION));
        assert!(!request.headers().contains_key(PROXY_AUTHORIZATION));
        assert!(!request.headers().contains_key(COOKIE));
        assert_eq!(request.headers()[IF_NONE_MATCH], "repository-etag");
    }
}
