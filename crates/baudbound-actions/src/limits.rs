pub const DEFAULT_MAX_HTTP_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_MAX_FILE_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
pub const DEFAULT_MAX_FILE_READ_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionLimits {
    pub max_file_download_bytes: u64,
    pub max_file_read_bytes: u64,
    pub max_http_response_bytes: u64,
}

impl Default for ActionLimits {
    fn default() -> Self {
        Self {
            max_file_download_bytes: DEFAULT_MAX_FILE_DOWNLOAD_BYTES,
            max_file_read_bytes: DEFAULT_MAX_FILE_READ_BYTES,
            max_http_response_bytes: DEFAULT_MAX_HTTP_RESPONSE_BYTES,
        }
    }
}
