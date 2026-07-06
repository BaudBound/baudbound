use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::{Read, Seek},
    path::Path,
};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use zip::ZipArchive;

use crate::{Capabilities, EditorMetadata, Manifest, Permissions, Program};

const REQUIRED_PACKAGE_FILES: &[&str] = &[
    "manifest.json",
    "program.json",
    "permissions.json",
    "capabilities.json",
];
const OPTIONAL_ROOT_FILES: &[&str] = &["README.md", "editor.json"];
const ASSET_PACKAGE_DIR: &str = "assets";
const ALLOWED_ASSET_EXTENSIONS: &[&str] = &[
    "csv", "flac", "gif", "jpeg", "jpg", "json", "m4a", "mp3", "ogg", "png", "svg", "txt", "wav",
    "webp",
];

#[derive(Debug, Clone, Serialize)]
pub struct PackageEntry {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageSummary {
    pub asset_count: usize,
    pub file_count: usize,
    pub package_format_version: u32,
    pub script_language_version: u32,
    pub script_name: String,
    pub target_runtime: String,
}

#[derive(Debug, Clone)]
pub struct ScriptPackage {
    pub capabilities: Capabilities,
    pub editor: Option<EditorMetadata>,
    pub entries: Vec<PackageEntry>,
    pub manifest: Manifest,
    pub permissions: Permissions,
    pub program: Program,
}

impl ScriptPackage {
    #[must_use]
    pub fn summary(&self) -> PackageSummary {
        PackageSummary {
            asset_count: self.manifest.assets.len(),
            file_count: self.entries.len(),
            package_format_version: self.manifest.format_version,
            script_language_version: self.manifest.script_language_version,
            script_name: self.manifest.name.clone(),
            target_runtime: self.capabilities.target_runtime.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PackageLoadError {
    #[error("failed to open package: {0}")]
    Open(#[source] std::io::Error),
    #[error("failed to read zip package: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{file_name} contains invalid JSON: {source}")]
    Json {
        file_name: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("package validation failed: {0}")]
    Validation(String),
    #[error("failed to read {file_name}: {source}")]
    Read {
        file_name: &'static str,
        #[source]
        source: std::io::Error,
    },
}

pub fn load_script_package(path: impl AsRef<Path>) -> Result<ScriptPackage, PackageLoadError> {
    let file = File::open(path).map_err(PackageLoadError::Open)?;
    load_script_package_reader(file)
}

pub fn load_script_package_reader<R: Read + Seek>(
    reader: R,
) -> Result<ScriptPackage, PackageLoadError> {
    let mut archive = ZipArchive::new(reader)?;
    let entries = collect_package_entries(&mut archive)?;
    validate_package_entries(&entries)?;

    let manifest = read_json_file::<Manifest, _>(&mut archive, "manifest.json")?;
    let program = read_json_file::<Program, _>(&mut archive, "program.json")?;
    let permissions = read_json_file::<Permissions, _>(&mut archive, "permissions.json")?;
    let capabilities = read_json_file::<Capabilities, _>(&mut archive, "capabilities.json")?;
    let editor = read_optional_json_file::<EditorMetadata, _>(&mut archive, "editor.json")?;

    validate_manifest_assets(&entries, &manifest)?;

    Ok(ScriptPackage {
        capabilities,
        editor,
        entries,
        manifest,
        permissions,
        program,
    })
}

fn collect_package_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<PackageEntry>, PackageLoadError> {
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }

        entries.push(PackageEntry {
            path: file.name().to_owned(),
            size: file.size(),
        });
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn validate_package_entries(entries: &[PackageEntry]) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();
    let paths = entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();

    for required_file in REQUIRED_PACKAGE_FILES {
        if !paths.contains(required_file) {
            errors.push(format!("missing required package file {required_file}"));
        }
    }

    let mut seen_lowercase_paths = BTreeSet::new();
    for entry in entries {
        let lowercase_path = entry.path.to_lowercase();
        if !seen_lowercase_paths.insert(lowercase_path) {
            errors.push(format!("{}: duplicate package path", entry.path));
        }

        if is_root_package_file(&entry.path)
            || entry.path.starts_with(&format!("{ASSET_PACKAGE_DIR}/"))
        {
            continue;
        }

        errors.push(format!("{}: package file is not allowed", entry.path));
    }

    for entry in entries
        .iter()
        .filter(|entry| entry.path.starts_with(&format!("{ASSET_PACKAGE_DIR}/")))
    {
        if let Err(error) = validate_asset_package_path(&entry.path) {
            errors.push(format!("{}: {error}", entry.path));
        }
    }

    finish_validation(errors)
}

fn validate_manifest_assets(
    entries: &[PackageEntry],
    manifest: &Manifest,
) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();
    let asset_zip_paths = entries
        .iter()
        .filter(|entry| entry.path.starts_with(&format!("{ASSET_PACKAGE_DIR}/")))
        .map(|entry| (entry.path.to_lowercase(), entry))
        .collect::<HashMap<_, _>>();
    let mut manifest_paths = HashMap::<String, &str>::new();
    let mut manifest_path_counts = HashMap::<String, usize>::new();

    for asset in &manifest.assets {
        let normalized_path = asset.path.to_lowercase();
        *manifest_path_counts
            .entry(normalized_path.clone())
            .or_default() += 1;
        manifest_paths.insert(normalized_path.clone(), asset.path.as_str());

        match asset_zip_paths.get(&normalized_path) {
            Some(entry) if entry.size == asset.size => {}
            Some(entry) => errors.push(format!(
                "{}: manifest size {} does not match package size {}",
                asset.path, asset.size, entry.size
            )),
            None => errors.push(format!(
                "{} is listed in manifest but missing from package",
                asset.path
            )),
        }
    }

    for (normalized_path, count) in manifest_path_counts {
        if count > 1 {
            errors.push(format!(
                "{}: duplicate manifest asset path",
                manifest_paths
                    .get(&normalized_path)
                    .copied()
                    .unwrap_or(&normalized_path)
            ));
        }
    }

    for entry in asset_zip_paths.values() {
        if !manifest_paths.contains_key(&entry.path.to_lowercase()) {
            errors.push(format!(
                "{}: asset file is not declared in manifest.json assets",
                entry.path
            ));
        }
    }

    finish_validation(errors)
}

pub fn validate_asset_package_path(path: &str) -> Result<(), &'static str> {
    if !path.starts_with(&format!("{ASSET_PACKAGE_DIR}/")) {
        return Err("asset path must be inside assets/");
    }
    if path.contains('\\') || path.starts_with('/') || path.contains(':') {
        return Err("asset path must be relative and cannot contain path traversal");
    }
    if path.ends_with('/') || path == format!("{ASSET_PACKAGE_DIR}/") {
        return Err("asset path must point to a file");
    }
    if path.chars().any(|character| character.is_control()) {
        return Err("asset path cannot contain control characters");
    }
    if path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(
            "asset path must not contain empty, current-directory, or parent-directory segments",
        );
    }
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_lowercase());
    if !extension
        .as_deref()
        .is_some_and(|extension| ALLOWED_ASSET_EXTENSIONS.contains(&extension))
    {
        return Err("asset path uses an unsupported extension");
    }

    Ok(())
}

fn read_json_file<T: DeserializeOwned, R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    file_name: &'static str,
) -> Result<T, PackageLoadError> {
    let mut file = archive.by_name(file_name)?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|source| PackageLoadError::Read { file_name, source })?;
    serde_json::from_str(strip_utf8_bom(&content))
        .map_err(|source| PackageLoadError::Json { file_name, source })
}

fn read_optional_json_file<T: DeserializeOwned, R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    file_name: &'static str,
) -> Result<Option<T>, PackageLoadError> {
    match archive.by_name(file_name) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|source| PackageLoadError::Read { file_name, source })?;
            serde_json::from_str(strip_utf8_bom(&content))
                .map(Some)
                .map_err(|source| PackageLoadError::Json { file_name, source })
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn is_root_package_file(path: &str) -> bool {
    REQUIRED_PACKAGE_FILES.contains(&path) || OPTIONAL_ROOT_FILES.contains(&path)
}

fn finish_validation(errors: Vec<String>) -> Result<(), PackageLoadError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(PackageLoadError::Validation(errors.join("; ")))
    }
}

fn strip_utf8_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    #[test]
    fn validates_supported_asset_paths() {
        assert!(validate_asset_package_path("assets/notify.wav").is_ok());
        assert_eq!(
            validate_asset_package_path("assets/../evil.txt"),
            Err(
                "asset path must not contain empty, current-directory, or parent-directory segments"
            )
        );
        assert_eq!(
            validate_asset_package_path("assets/script.exe"),
            Err("asset path uses an unsupported extension")
        );
    }

    #[test]
    fn loads_valid_package() {
        let package = load_script_package_reader(Cursor::new(create_test_package(&[])))
            .expect("valid package should load");

        assert_eq!(package.summary().script_name, "hello-log");
        assert_eq!(package.summary().asset_count, 0);
    }

    #[test]
    fn rejects_orphaned_asset_file() {
        let error = load_script_package_reader(Cursor::new(create_test_package(&[(
            "assets/orphan.txt",
            "orphan",
        )])))
        .expect_err("orphaned asset should be rejected");

        assert!(error.to_string().contains("asset file is not declared"));
    }

    fn create_test_package(extra_files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in [
            (
                "manifest.json",
                r#"{
					"format_version": 1,
					"script_language_version": 1,
					"id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
					"name": "hello-log",
					"created_with": "BaudBound Test",
					"created_at": "2026-01-01T00:00:00.000Z",
					"minimum_runner_version": "0.1.0"
				}"#,
            ),
            (
                "program.json",
                r#"{
					"entry": {
						"trigger": {"id": "n-1"},
						"triggers": [],
						"program": {"type": "block", "steps": [], "edges": []}
					}
				}"#,
            ),
            (
                "permissions.json",
                r#"{"declared_permissions": [], "risk_level": "low"}"#,
            ),
            (
                "capabilities.json",
                r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
            ),
        ] {
            writer
                .start_file(path, options)
                .expect("test zip file should start");
            writer
                .write_all(content.as_bytes())
                .expect("test zip content should write");
        }

        for (path, content) in extra_files {
            writer
                .start_file(path, options)
                .expect("test zip file should start");
            writer
                .write_all(content.as_bytes())
                .expect("test zip content should write");
        }

        writer
            .finish()
            .expect("test zip should finish")
            .into_inner()
    }
}
