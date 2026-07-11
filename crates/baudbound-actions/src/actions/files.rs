use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use reqwest::blocking::Client;
use serde_json::{Map, Number, Value};

use crate::{config_bool, config_string, failed, required_string, timeout_duration};

mod move_file;

use move_file::move_file;

pub(crate) fn read_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let encoding = config_string(&request.config, "encoding").unwrap_or_else(|| "utf-8".to_owned());
    if encoding != "utf-8" {
        return failed(request, format!("unsupported file encoding {encoding}"));
    }

    let bytes = fs::read(Path::new(&path)).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to read {path}: {source}"),
    })?;
    let content =
        String::from_utf8(bytes.clone()).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("{path} is not valid UTF-8: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("content".to_owned(), Value::String(content)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes.len()))),
        ]),
    })
}
pub(crate) fn download_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let url = required_string(request, "url")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let destination = PathBuf::from(&destination_path);
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let client = Client::builder()
        .timeout(timeout_duration(request)?)
        .build()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to build HTTP client: {source}"),
        })?;
    let response = client
        .get(&url)
        .send()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("download request {url} failed: {source}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        return failed(
            request,
            format!("download request {url} returned {}", status.as_u16()),
        );
    }

    let bytes = response
        .bytes()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read download response body: {source}"),
        })?;
    fs::write(&destination, &bytes).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to write download to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(destination_path)),
            ("url".to_owned(), Value::String(url)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes.len()))),
        ]),
    })
}

pub(crate) fn write_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let content = config_string(&request.config, "content").unwrap_or_default();
    let mode = config_string(&request.config, "mode").unwrap_or_else(|| "overwrite".to_owned());
    let path_buf = PathBuf::from(&path);

    if let Some(parent) = path_buf
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to create parent directory for {path}: {source}"),
        })?;
    }

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    match mode.as_str() {
        "append" => {
            options.append(true);
        }
        "overwrite" => {
            options.truncate(true);
        }
        other => return failed(request, format!("unsupported file write mode {other}")),
    }

    let mut file = options
        .open(&path_buf)
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to open {path} for writing: {source}"),
        })?;
    file.write_all(content.as_bytes())
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to write {path}: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("mode".to_owned(), Value::String(mode)),
            (
                "bytes".to_owned(),
                Value::Number(Number::from(content.len())),
            ),
        ]),
    })
}

pub(crate) fn copy_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = PathBuf::from(&source_path);
    let destination = PathBuf::from(&destination_path);

    ensure_regular_source(request, &source)?;
    ensure_distinct_paths(request, &source, &destination)?;
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let bytes = fs::copy(&source, &destination).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to copy {source_path} to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
            ("bytes".to_owned(), Value::Number(Number::from(bytes))),
        ]),
    })
}

pub(crate) fn move_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = PathBuf::from(&source_path);
    let destination = PathBuf::from(&destination_path);

    ensure_regular_source(request, &source)?;
    ensure_distinct_paths(request, &source, &destination)?;
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;
    move_file(&source, &destination, overwrite).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to move {source_path} to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
        ]),
    })
}

pub(crate) fn delete_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let path_buf = PathBuf::from(&path);
    let metadata = fs::metadata(&path_buf).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to inspect {path}: {source}"),
    })?;
    if !metadata.is_file() {
        return failed(request, format!("{path} is not a regular file"));
    }

    fs::remove_file(&path_buf).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to delete {path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([("path".to_owned(), Value::String(path))]),
    })
}

fn ensure_destination_available(
    request: &RuntimeActionRequest,
    destination: &Path,
    overwrite: bool,
) -> Result<(), RuntimeActionError> {
    if destination.exists() && !overwrite {
        return failed(
            request,
            format!(
                "destination {} already exists and overwrite is disabled",
                destination.display()
            ),
        );
    }
    if destination.exists() && !destination.is_file() {
        return failed(
            request,
            format!(
                "destination {} is not a regular file",
                destination.display()
            ),
        );
    }
    Ok(())
}

fn ensure_regular_source(
    request: &RuntimeActionRequest,
    source: &Path,
) -> Result<(), RuntimeActionError> {
    let metadata = fs::metadata(source).map_err(|source_error| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!(
            "failed to inspect source {}: {source_error}",
            source.display()
        ),
    })?;
    if !metadata.is_file() {
        return failed(
            request,
            format!("source {} is not a regular file", source.display()),
        );
    }
    Ok(())
}

fn ensure_distinct_paths(
    request: &RuntimeActionRequest,
    source: &Path,
    destination: &Path,
) -> Result<(), RuntimeActionError> {
    if !source.exists() || !destination.exists() {
        return Ok(());
    }

    let source = source
        .canonicalize()
        .map_err(|source_error| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to resolve source path {}: {source_error}",
                source.display()
            ),
        })?;
    let destination =
        destination
            .canonicalize()
            .map_err(|source_error| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!(
                    "failed to resolve destination path {}: {source_error}",
                    destination.display()
                ),
            })?;
    if source == destination {
        return failed(
            request,
            format!(
                "source and destination resolve to the same file: {}",
                source.display()
            ),
        );
    }
    Ok(())
}

fn ensure_parent_directory(
    request: &RuntimeActionRequest,
    destination: &Path,
) -> Result<(), RuntimeActionError> {
    let Some(parent) = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!(
            "failed to create parent directory for {}: {source}",
            destination.display()
        ),
    })
}
