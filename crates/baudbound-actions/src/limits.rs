pub const DEFAULT_MAX_HTTP_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_MAX_FILE_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
pub const DEFAULT_MAX_FILE_READ_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionLimits {
    pub max_file_download_bytes: ResourceLimit,
    pub max_file_read_bytes: ResourceLimit,
    pub max_http_response_bytes: ResourceLimit,
    pub max_generated_text_bytes: ResourceLimit,
    pub max_process_output_bytes: ResourceLimit,
    pub max_processes_per_script: ResourceLimit,
    pub max_process_launches_per_minute: ResourceLimit,
    pub max_file_write_bytes_per_run: ResourceLimit,
}

impl Default for ActionLimits {
    fn default() -> Self {
        Self {
            max_file_download_bytes: ResourceLimit::limited(DEFAULT_MAX_FILE_DOWNLOAD_BYTES),
            max_file_read_bytes: ResourceLimit::limited(DEFAULT_MAX_FILE_READ_BYTES),
            max_http_response_bytes: ResourceLimit::limited(DEFAULT_MAX_HTTP_RESPONSE_BYTES),
            max_generated_text_bytes: ResourceLimit::limited(64 * 1024 * 1024),
            max_process_output_bytes: ResourceLimit::limited(8 * 1024 * 1024),
            max_processes_per_script: ResourceLimit::limited(4),
            max_process_launches_per_minute: ResourceLimit::limited(120),
            max_file_write_bytes_per_run: ResourceLimit::limited(1024 * 1024 * 1024),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSecurityPolicy {
    pub allow_process_execution: bool,
    pub allow_private_http_requests: bool,
    pub allow_shell_commands: bool,
}

impl Default for ActionSecurityPolicy {
    fn default() -> Self {
        Self {
            allow_process_execution: true,
            allow_private_http_requests: false,
            allow_shell_commands: true,
        }
    }
}
use baudbound_runtime::ResourceLimit;
