use std::{io::Read, time::Duration};

use reqwest::blocking::{Client, Response};
use serde::de::DeserializeOwned;
use url::Url;

use super::{error::BackendApiError, pagination::CollectionPage};

const PRODUCTION_API_URL: &str = "https://api.baudbound.app/";
const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CollectionRequest<'a> {
    pub collection: &'a str,
    pub fields: &'a str,
    pub filter: Option<&'a str>,
    pub maximum_records: usize,
    pub maximum_response_bytes: usize,
    pub sort: Option<&'a str>,
}

pub(crate) struct BackendApiClient {
    base_url: Url,
    client: Client,
}

impl BackendApiClient {
    pub(crate) fn production() -> Result<Self, BackendApiError> {
        let base_url =
            Url::parse(PRODUCTION_API_URL).map_err(|_| BackendApiError::InvalidBaseUrl)?;
        Self::new(base_url)
    }

    fn new(base_url: Url) -> Result<Self, BackendApiError> {
        validate_base_url(&base_url)?;
        Self::with_base_url(base_url)
    }

    fn with_base_url(base_url: Url) -> Result<Self, BackendApiError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("BaudBound/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(BackendApiError::ClientBuild)?;
        Ok(Self { base_url, client })
    }

    #[cfg(test)]
    fn for_test(base_url: Url) -> Result<Self, BackendApiError> {
        Self::with_base_url(base_url)
    }

    pub(crate) fn list_collection<T>(
        &self,
        request: CollectionRequest<'_>,
    ) -> Result<Vec<T>, BackendApiError>
    where
        T: DeserializeOwned,
    {
        if request.maximum_records == 0 || request.maximum_response_bytes == 0 {
            return Err(BackendApiError::InvalidResponse(
                "collection limits must be greater than zero".to_owned(),
            ));
        }
        let endpoint = self.collection_url(request.collection)?;
        let maximum_pages = request.maximum_records.div_ceil(PAGE_SIZE);
        let mut records = Vec::new();
        let mut page = 1;

        loop {
            let mut query = vec![
                ("page", page.to_string()),
                ("perPage", PAGE_SIZE.to_string()),
                ("fields", request.fields.to_owned()),
            ];
            if let Some(filter) = request.filter {
                query.push(("filter", filter.to_owned()));
            }
            if let Some(sort) = request.sort {
                query.push(("sort", sort.to_owned()));
            }

            let response = self
                .client
                .get(endpoint.clone())
                .query(&query)
                .send()
                .map_err(BackendApiError::Request)?;
            if !response.status().is_success() {
                return Err(BackendApiError::Status(response.status().as_u16()));
            }
            let page_response: CollectionPage<T> =
                serde_json::from_slice(&read_bounded(response, request.maximum_response_bytes)?)
                    .map_err(BackendApiError::Json)?;
            validate_page(&page_response, page, maximum_pages)?;

            records.extend(page_response.items);
            if records.len() > request.maximum_records {
                return Err(BackendApiError::TooManyRecords);
            }
            if page >= page_response.total_pages {
                break;
            }
            page += 1;
        }

        Ok(records)
    }

    fn collection_url(&self, collection: &str) -> Result<Url, BackendApiError> {
        if collection.is_empty()
            || !collection
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(BackendApiError::InvalidCollection);
        }
        self.base_url
            .join(&format!("api/collections/{collection}/records"))
            .map_err(|_| BackendApiError::InvalidCollection)
    }
}

fn validate_base_url(base_url: &Url) -> Result<(), BackendApiError> {
    if base_url.scheme() != "https"
        || base_url.host_str().is_none()
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(BackendApiError::InvalidBaseUrl);
    }
    Ok(())
}

fn validate_page<T>(
    page: &CollectionPage<T>,
    requested_page: usize,
    maximum_pages: usize,
) -> Result<(), BackendApiError> {
    if page.page != requested_page
        || !(1..=PAGE_SIZE).contains(&page.per_page)
        || page.total_pages > maximum_pages
    {
        return Err(BackendApiError::InvalidResponse(
            "pagination metadata is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn read_bounded(response: Response, maximum: usize) -> Result<Vec<u8>, BackendApiError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(BackendApiError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    response
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(BackendApiError::Io)?;
    if bytes.len() > maximum {
        return Err(BackendApiError::ResponseTooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct TestRecord {
        id: String,
    }

    #[test]
    fn production_endpoint_is_https_and_uses_the_expected_host() {
        let client = BackendApiClient::production().expect("client should build");
        let endpoint = client
            .collection_url("blacklist")
            .expect("collection should be valid");

        assert_eq!(endpoint.scheme(), "https");
        assert_eq!(endpoint.host_str(), Some("api.baudbound.app"));
        assert_eq!(endpoint.path(), "/api/collections/blacklist/records");
    }

    #[test]
    fn collection_names_cannot_escape_the_api_path() {
        let client = BackendApiClient::production().expect("client should build");

        assert!(matches!(
            client.collection_url("../users"),
            Err(BackendApiError::InvalidCollection)
        ));
        assert!(matches!(
            client.collection_url("blacklist?filter=true"),
            Err(BackendApiError::InvalidCollection)
        ));
    }

    #[test]
    fn pagination_rejects_mismatched_pages_and_excessive_totals() {
        let page = CollectionPage::<serde_json::Value> {
            items: Vec::new(),
            page: 2,
            per_page: PAGE_SIZE,
            total_pages: 2,
        };
        assert!(validate_page(&page, 1, 2).is_err());

        let page = CollectionPage::<serde_json::Value> {
            items: Vec::new(),
            page: 1,
            per_page: PAGE_SIZE,
            total_pages: 3,
        };
        assert!(validate_page(&page, 1, 2).is_err());
    }

    #[test]
    fn collection_requests_follow_pagination_and_decode_records() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("address should be available");
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for (index, body) in [
                r#"{"page":1,"perPage":100,"totalPages":2,"items":[{"id":"first"}]}"#,
                r#"{"page":2,"perPage":100,"totalPages":2,"items":[{"id":"second"}]}"#,
            ]
            .into_iter()
            .enumerate()
            {
                let (mut stream, _) = listener.accept().expect("request should connect");
                let mut bytes = [0_u8; 4096];
                let count = stream.read(&mut bytes).expect("request should read");
                requests.push(String::from_utf8_lossy(&bytes[..count]).into_owned());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nX-Test-Page: {}\r\n\r\n{}",
                    body.len(),
                    index + 1,
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("response should write");
            }
            requests
        });
        let client = BackendApiClient::for_test(
            Url::parse(&format!("http://{address}/")).expect("test URL should parse"),
        )
        .expect("test client should build");

        let records = client
            .list_collection::<TestRecord>(CollectionRequest {
                collection: "notices",
                fields: "id",
                filter: Some("active = true"),
                maximum_records: 200,
                maximum_response_bytes: 4096,
                sort: Some("-created"),
            })
            .expect("collection should load");
        let requests = server.join().expect("server should finish");

        assert_eq!(
            records,
            vec![
                TestRecord {
                    id: "first".to_owned()
                },
                TestRecord {
                    id: "second".to_owned()
                }
            ]
        );
        assert!(requests[0].starts_with("GET /api/collections/notices/records?"));
        assert!(requests[0].contains("page=1"));
        assert!(requests[0].contains("fields=id"));
        assert!(requests[1].contains("page=2"));
    }
}
