use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CollectionPage<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}
