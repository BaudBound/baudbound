use std::{
    fs,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use crate::StorageError;

pub(crate) fn validate_script_id(script_id: &str) -> Result<(), StorageError> {
    if script_id.is_empty()
        || script_id == "."
        || script_id == ".."
        || script_id
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err(StorageError::InvalidScriptId(script_id.to_owned()));
    }

    Ok(())
}

pub(crate) fn package_file_name_from_path(path: &Path) -> Result<String, StorageError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| StorageError::InvalidPackageFileName(path.display().to_string()))?;
    validate_package_file_name(file_name)?;
    Ok(file_name.to_owned())
}

pub(crate) fn validate_package_file_name(file_name: &str) -> Result<(), StorageError> {
    let lower = file_name.to_ascii_lowercase();
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || !lower.ends_with(".bbs")
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains(':')
        || file_name.chars().any(|character| character.is_control())
    {
        return Err(StorageError::InvalidPackageFileName(file_name.to_owned()));
    }
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, StorageError> {
    let mut file = fs::File::open(path).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|source| StorageError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn copy_file(source: &Path, destination: &Path) -> Result<(), StorageError> {
    if let Some(parent) = destination.parent() {
        create_dir_all(parent)?;
    }

    fs::copy(source, destination).map_err(|source| StorageError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

pub(crate) fn create_dir_all(path: impl AsRef<Path>) -> Result<(), StorageError> {
    let path = path.as_ref();
    fs::create_dir_all(path).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn remove_file_inside_root(root: &Path, target: &Path) -> Result<(), StorageError> {
    let root = root.canonicalize().map_err(|source| StorageError::Io {
        path: root.to_path_buf(),
        source,
    })?;

    if !target.exists() {
        return Ok(());
    }

    let target = target.canonicalize().map_err(|source| StorageError::Io {
        path: target.to_path_buf(),
        source,
    })?;

    if !target.starts_with(&root) {
        return Err(StorageError::PathOutsideRoot { path: target, root });
    }

    fs::remove_file(&target).map_err(|source| StorageError::Io {
        path: target,
        source,
    })
}

pub(crate) fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
