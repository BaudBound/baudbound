use std::{
    fs,
    io::{self, BufRead, Write},
};

use crate::{FilesystemScriptStore, ScriptStore, StorageError, StoredRunRecord};

pub(crate) fn append_run_record(
    store: &FilesystemScriptStore,
    record: StoredRunRecord,
) -> Result<(), StorageError> {
    store.ensure_layout()?;
    let path = store.run_history_path();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| StorageError::Io {
            path: path.clone(),
            source,
        })?;
    let line = serde_json::to_string(&record).map_err(|source| StorageError::Json {
        path: path.clone(),
        source,
    })?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|source| StorageError::Io { path, source })
}

pub(crate) fn list_run_records(
    store: &FilesystemScriptStore,
    script_reference: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<StoredRunRecord>, StorageError> {
    let script_id = match script_reference {
        Some(reference) => Some(store.find_script(reference)?.id),
        None => None,
    };
    let path = store.run_history_path();
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(StorageError::Io { path, source }),
    };

    let mut records = Vec::new();
    for line in io::BufReader::new(file).lines() {
        let line = line.map_err(|source| StorageError::Io {
            path: path.clone(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str::<StoredRunRecord>(&line).map_err(|source| {
            StorageError::Json {
                path: path.clone(),
                source,
            }
        })?;
        if script_id
            .as_deref()
            .is_none_or(|script_id| record.script_id == script_id)
        {
            records.push(record);
        }
    }

    records.sort_by(|left, right| {
        right
            .completed_at_unix
            .cmp(&left.completed_at_unix)
            .then_with(|| right.run_id.cmp(&left.run_id))
    });
    if let Some(limit) = limit {
        records.truncate(limit);
    }
    Ok(records)
}
