use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::LOCATION,
};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use thiserror::Error;
use url::Url;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
pub(crate) const MAX_DESCRIPTOR_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteResourceKind {
    Descriptor,
    Package,
}

impl RemoteResourceKind {
    fn validate_path(self, url: &Url) -> Result<(), RemoteFetchError> {
        let file_name = url
            .path_segments()
            .and_then(Iterator::last)
            .filter(|value| !value.is_empty())
            .ok_or(RemoteFetchError::InvalidPath(self))?;
        let valid = match self {
            Self::Descriptor => file_name.eq_ignore_ascii_case("update.json"),
            Self::Package => file_name.to_ascii_lowercase().ends_with(".bbs"),
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
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

#[derive(Debug, Error)]
pub(crate) enum RemoteFetchError {
    #[error("the remote URL is invalid")]
    InvalidUrl,
    #[error("remote URLs must use HTTPS and cannot contain credentials or fragments")]
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

    pub(crate) fn fetch_descriptor(&self, value: &str) -> Result<(Vec<u8>, Url), RemoteFetchError> {
        self.fetch_descriptor_with_progress(value, &mut |_, _| true)
    }

    pub(crate) fn fetch_descriptor_with_progress(
        &self,
        value: &str,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<(Vec<u8>, Url), RemoteFetchError> {
        let url = validate_url(value, RemoteResourceKind::Descriptor)?;
        let (mut response, final_url) = self.send(url)?;
        let expected_length = response.content_length();
        let bytes = read_bounded(
            &mut response,
            expected_length,
            MAX_DESCRIPTOR_BYTES,
            progress,
        )?;
        Ok((bytes, final_url))
    }

    pub(crate) fn fetch_package_with_progress(
        &self,
        value: &str,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<RemoteDownload, RemoteFetchError> {
        let url = validate_url(value, RemoteResourceKind::Package)?;
        let (mut response, _) = self.send(url)?;
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
        Ok(RemoteDownload { file, sha256, size })
    }

    fn send(&self, mut url: Url) -> Result<(Response, Url), RemoteFetchError> {
        for redirect_count in 0..=MAX_REDIRECTS {
            let addresses = resolve_public_addresses(&url)?;
            let host = url.host_str().ok_or(RemoteFetchError::InvalidUrl)?;
            let client = pinned_client(host, &addresses)?;
            let response = client
                .get(url.clone())
                .send()
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
                url = url
                    .join(location)
                    .map_err(|_| RemoteFetchError::InvalidRedirect)?;
                validate_transport_url(&url)?;
                continue;
            }
            if !response.status().is_success() {
                return Err(RemoteFetchError::HttpStatus(response.status()));
            }
            return Ok((response, url));
        }
        Err(RemoteFetchError::TooManyRedirects)
    }
}

fn validate_url(value: &str, kind: RemoteResourceKind) -> Result<Url, RemoteFetchError> {
    let url = Url::parse(value).map_err(|_| RemoteFetchError::InvalidUrl)?;
    validate_parsed_url(&url, kind)?;
    Ok(url)
}

fn validate_parsed_url(url: &Url, kind: RemoteResourceKind) -> Result<(), RemoteFetchError> {
    validate_transport_url(url)?;
    kind.validate_path(url)
}

fn validate_transport_url(url: &Url) -> Result<(), RemoteFetchError> {
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(RemoteFetchError::UnsafeUrl);
    }
    Ok(())
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
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(RemoteFetchError::RestrictedDestination);
    }
    Ok(addresses)
}

fn pinned_client(host: &str, addresses: &[SocketAddr]) -> Result<Client, RemoteFetchError> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, addresses)
        .user_agent(concat!("BaudBound/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| RemoteFetchError::Request)
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

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || octets[0] == 0
        || octets[0] >= 240
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || segments[0] == 0
        || segments[0] == 0x0064 && segments[1] == 0xff9b
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || segments[0] == 0x2002
        || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_resource_urls() {
        assert!(
            validate_url(
                "https://example.com/update.json",
                RemoteResourceKind::Descriptor
            )
            .is_ok()
        );
        assert!(
            validate_url(
                "https://example.com/releases/test.bbs?token=value",
                RemoteResourceKind::Package
            )
            .is_ok()
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
                RemoteResourceKind::Descriptor
            )
            .is_err()
        );
    }

    #[test]
    fn allows_safe_redirect_targets_without_requiring_the_original_file_name() {
        assert!(
            validate_transport_url(
                &Url::parse("https://objects.example.com/download/opaque-id?signature=value")
                    .unwrap()
            )
            .is_ok()
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
            assert!(!is_public_ip(
                address.parse().expect("test address should parse")
            ));
        }
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
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
}
