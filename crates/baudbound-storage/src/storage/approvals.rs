use std::{fs, io};

use crate::{
    ApproveScriptRequest, FilesystemScriptStore, ScriptApproval, ScriptStore, StorageError,
    storage::{
        filesystem::{current_unix_timestamp, validate_script_id, write_atomic},
        metadata::ApprovalIndex,
    },
};

pub(crate) fn approve_script(
    store: &FilesystemScriptStore,
    request: ApproveScriptRequest,
) -> Result<ScriptApproval, StorageError> {
    validate_script_id(&request.script_id)?;
    let mut approvals = read_approvals(store)?;
    let approval = ScriptApproval {
        approved_at_unix: current_unix_timestamp(),
        approved_permissions: request.approved_permissions,
        package_hash: request.package_hash,
        script_id: request.script_id,
    };
    approvals
        .approvals
        .insert(approval.script_id.clone(), approval.clone());
    write_approvals(store, &approvals)?;
    Ok(approval)
}

pub(crate) fn find_script_approval(
    store: &FilesystemScriptStore,
    script_reference: &str,
) -> Result<Option<ScriptApproval>, StorageError> {
    let installed = store.find_script(script_reference)?;
    let approvals = read_approvals(store)?;
    Ok(approvals.approvals.get(&installed.id).cloned())
}

pub(crate) fn revoke_script_approval(
    store: &FilesystemScriptStore,
    script_reference: &str,
) -> Result<Option<ScriptApproval>, StorageError> {
    let installed = store.find_script(script_reference)?;
    revoke_script_approval_by_id(store, &installed.id)
}

pub(crate) fn revoke_script_approval_by_id(
    store: &FilesystemScriptStore,
    script_id: &str,
) -> Result<Option<ScriptApproval>, StorageError> {
    validate_script_id(script_id)?;
    let mut approvals = read_approvals(store)?;
    let removed = approvals.approvals.remove(script_id);
    write_approvals(store, &approvals)?;
    Ok(removed)
}

fn read_approvals(store: &FilesystemScriptStore) -> Result<ApprovalIndex, StorageError> {
    let approvals_path = store.approvals_path();
    match fs::read_to_string(&approvals_path) {
        Ok(content) => serde_json::from_str(&content).map_err(|source| StorageError::Json {
            path: approvals_path,
            source,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ApprovalIndex::default()),
        Err(source) => Err(StorageError::Io {
            path: approvals_path,
            source,
        }),
    }
}

fn write_approvals(
    store: &FilesystemScriptStore,
    approvals: &ApprovalIndex,
) -> Result<(), StorageError> {
    store.ensure_layout()?;
    let approvals_path = store.approvals_path();
    let content = serde_json::to_string_pretty(approvals).map_err(|source| StorageError::Json {
        path: approvals_path.clone(),
        source,
    })?;
    write_atomic(&approvals_path, content.as_bytes())
}
