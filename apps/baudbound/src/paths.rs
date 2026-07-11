use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn default_runner_home() -> PathBuf {
    if let Some(path) = std::env::var_os("BAUDBOUND_HOME") {
        return PathBuf::from(path);
    }

    platform_data_dir().join("BaudBound").join("runner")
}

pub fn default_config_path(runner_home: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("BAUDBOUND_CONFIG") {
        return PathBuf::from(path);
    }

    runner_home.join("config.toml")
}

pub fn default_database_path(runner_home: &std::path::Path) -> PathBuf {
    runner_home.join("runner.sqlite3")
}

pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn platform_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path);
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(path);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share");
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
