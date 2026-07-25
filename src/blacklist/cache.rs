use std::{collections::HashSet, fs, io::Write, path::Path};

use baudbound_security::{BlacklistEntry, normalize_blacklist_entry};

use super::{api::MAX_ENTRIES, error::BlacklistError, models::PersistedState};

const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn load_state(path: &Path) -> Result<PersistedState, BlacklistError> {
    if !path.exists() {
        return Ok(PersistedState::default());
    }
    let metadata = fs::metadata(path).map_err(BlacklistError::Io)?;
    if metadata.len() > MAX_STATE_BYTES {
        return Err(BlacklistError::StateTooLarge);
    }
    let bytes = fs::read(path).map_err(BlacklistError::Io)?;
    let mut state: PersistedState = serde_json::from_slice(&bytes).map_err(BlacklistError::Json)?;
    state.entries = validate_snapshot(state.entries)?;
    Ok(state)
}

fn validate_snapshot(entries: Vec<BlacklistEntry>) -> Result<Vec<BlacklistEntry>, BlacklistError> {
    if entries.len() > MAX_ENTRIES {
        return Err(BlacklistError::TooManyEntries);
    }
    let mut ids = HashSet::new();
    let mut targets = HashSet::new();
    entries
        .into_iter()
        .map(|entry| {
            let entry = normalize_blacklist_entry(entry)
                .map_err(|error| BlacklistError::InvalidResponse(error.to_string()))?;
            if !ids.insert(entry.id.clone()) || !targets.insert((entry.scope, entry.target.clone()))
            {
                return Err(BlacklistError::InvalidResponse(
                    "the cached blacklist contains duplicate entries".to_owned(),
                ));
            }
            Ok(entry)
        })
        .collect()
}

pub(super) fn save_state(path: &Path, state: &PersistedState) -> Result<(), BlacklistError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(BlacklistError::Io)?;
    }
    let bytes = serde_json::to_vec_pretty(state).map_err(BlacklistError::Json)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(BlacklistError::StateTooLarge);
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().unwrap_or_else(|| Path::new(".")))
            .map_err(BlacklistError::Io)?;
    temporary.write_all(&bytes).map_err(BlacklistError::Io)?;
    temporary.as_file().sync_all().map_err(BlacklistError::Io)?;
    temporary
        .persist(path)
        .map_err(|error| BlacklistError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use baudbound_security::{BlacklistScope, BlacklistSeverity};

    use super::*;

    #[test]
    fn state_file_can_be_replaced_without_losing_personal_blocks() {
        let directory = tempfile::tempdir().expect("temporary directory should open");
        let path = directory.path().join("blacklist-state.json");
        let mut state = PersistedState::default();
        state
            .personal_repository_blocks
            .insert("https://example.com/repository.json".to_owned());
        save_state(&path, &state).expect("initial state should save");

        state.entries.push(BlacklistEntry {
            advisory_url: "https://baudbound.app/advisories/test".to_owned(),
            id: "record123456789".to_owned(),
            published_at: "2026-07-25 12:00:00.000Z".to_owned(),
            reason: "A reviewed security concern".to_owned(),
            scope: BlacklistScope::Domain,
            severity: BlacklistSeverity::High,
            subdomains: true,
            target: "malicious.example".to_owned(),
            title: "Test advisory".to_owned(),
            updated: "2026-07-25 12:05:00.000Z".to_owned(),
        });
        save_state(&path, &state).expect("existing state should be replaced");

        let loaded = load_state(&path).expect("state should load");
        assert_eq!(loaded.entries.len(), 1);
        assert!(
            loaded
                .personal_repository_blocks
                .contains("https://example.com/repository.json")
        );
    }
}
