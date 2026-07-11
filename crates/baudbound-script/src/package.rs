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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAsset {
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub path: String,
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
    #[error("asset {0:?} is not declared in manifest.json")]
    AssetNotFound(String),
    #[error("failed to read asset {path}: {source}")]
    AssetRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
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

pub fn read_package_asset(
    package_path: impl AsRef<Path>,
    asset_reference: &str,
) -> Result<PackageAsset, PackageLoadError> {
    let file = File::open(package_path).map_err(PackageLoadError::Open)?;
    read_package_asset_reader(file, asset_reference)
}

pub fn read_package_asset_reader<R: Read + Seek>(
    reader: R,
    asset_reference: &str,
) -> Result<PackageAsset, PackageLoadError> {
    let mut archive = ZipArchive::new(reader)?;
    let entries = collect_package_entries(&mut archive)?;
    validate_package_entries(&entries)?;

    let manifest = read_json_file::<Manifest, _>(&mut archive, "manifest.json")?;
    validate_manifest_assets(&entries, &manifest)?;
    validate_manifest_secrets(&manifest)?;

    let reference = asset_reference.trim();
    let manifest_asset = if reference.starts_with(&format!("{ASSET_PACKAGE_DIR}/")) {
        validate_asset_package_path(reference)
            .map_err(|error| PackageLoadError::Validation(error.to_owned()))?;
        manifest
            .assets
            .iter()
            .find(|asset| asset.path.eq_ignore_ascii_case(reference))
    } else {
        manifest.assets.iter().find(|asset| asset.id == reference)
    }
    .ok_or_else(|| PackageLoadError::AssetNotFound(reference.to_owned()))?;

    let mut file = archive
        .by_name(&manifest_asset.path)
        .map_err(PackageLoadError::Zip)?;
    let mut bytes = Vec::with_capacity(file.size().try_into().unwrap_or_default());
    file.read_to_end(&mut bytes)
        .map_err(|source| PackageLoadError::AssetRead {
            path: manifest_asset.path.clone(),
            source,
        })?;

    Ok(PackageAsset {
        bytes,
        media_type: manifest_asset.media_type.clone(),
        path: manifest_asset.path.clone(),
    })
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
    validate_manifest_secrets(&manifest)?;

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

fn validate_manifest_secrets(manifest: &Manifest) -> Result<(), PackageLoadError> {
    const SUPPORTED_TYPES: &[&str] = &[
        "string",
        "number",
        "boolean",
        "object",
        "list",
        "http_response",
        "datetime",
        "duration",
        "file_path",
    ];

    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    for secret in &manifest.secrets {
        if !is_variable_identifier(&secret.name) {
            errors.push(format!(
                "manifest secret {:?} must start with a letter or underscore and contain only letters, numbers, or underscores",
                secret.name
            ));
        }
        if secret.name.starts_with("system_") || secret.name.starts_with("manifest_") {
            errors.push(format!(
                "manifest secret {:?} uses a reserved variable prefix",
                secret.name
            ));
        }
        if !names.insert(secret.name.as_str()) {
            errors.push(format!("duplicate manifest secret name {:?}", secret.name));
        }
        if !SUPPORTED_TYPES.contains(&secret.value_type.as_str()) {
            errors.push(format!(
                "manifest secret {:?} uses unsupported type {:?}",
                secret.name, secret.value_type
            ));
        }
    }
    finish_validation(errors)
}

fn is_variable_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
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
    fn loads_valid_secret_declarations() {
        let manifest = r#"{
            "format_version": 1,
            "script_language_version": 1,
            "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "name": "hello-log",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0",
            "secrets": [{
                "name": "api_token",
                "type": "string",
                "description": "API token",
                "required": true
            }]
        }"#;
        let package = load_script_package_reader(Cursor::new(create_test_package_with_manifest(
            manifest,
            &[],
        )))
        .expect("valid secret declaration should load");

        assert_eq!(package.manifest.secrets.len(), 1);
        assert_eq!(package.manifest.secrets[0].name, "api_token");
        assert!(package.manifest.secrets[0].required);
    }

    #[test]
    fn rejects_invalid_secret_declarations() {
        for (manifest, expected) in [
            (
                r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
                    "name": "hello-log",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0",
                    "secrets": [
                        {"name": "api_token", "type": "string"},
                        {"name": "api_token", "type": "string"}
                    ]
                }"#,
                "duplicate manifest secret name",
            ),
            (
                r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
                    "name": "hello-log",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0",
                    "secrets": [{"name": "system_token", "type": "string"}]
                }"#,
                "reserved variable prefix",
            ),
            (
                r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
                    "name": "hello-log",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0",
                    "secrets": [{"name": "api_token", "type": "binary"}]
                }"#,
                "unsupported type",
            ),
        ] {
            let error = load_script_package_reader(Cursor::new(create_test_package_with_manifest(
                manifest,
                &[],
            )))
            .expect_err("invalid secret declaration should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
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

    #[test]
    fn reads_declared_package_asset_by_path_or_id() {
        let package_bytes = create_test_package_with_assets(&[(
            "audio-1",
            "assets/sounds/beep.wav",
            "audio/wav",
            b"RIFFtestWAVE".as_slice(),
        )]);

        let by_path =
            read_package_asset_reader(Cursor::new(package_bytes.clone()), "assets/sounds/beep.wav")
                .expect("declared asset should read by path");
        let by_id = read_package_asset_reader(Cursor::new(package_bytes), "audio-1")
            .expect("declared asset should read by id");

        assert_eq!(by_path.path, "assets/sounds/beep.wav");
        assert_eq!(by_path.media_type, "audio/wav");
        assert_eq!(by_path.bytes, b"RIFFtestWAVE");
        assert_eq!(by_id, by_path);
    }

    #[test]
    fn rejects_undeclared_package_asset_reference() {
        let error = read_package_asset_reader(
            Cursor::new(create_test_package_with_assets(&[])),
            "assets/missing.wav",
        )
        .expect_err("missing asset should fail");

        assert!(matches!(error, PackageLoadError::AssetNotFound(_)));
    }

    fn create_test_package(extra_files: &[(&str, &str)]) -> Vec<u8> {
        create_test_package_with_manifest(
            r#"{
					"format_version": 1,
					"script_language_version": 1,
					"id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
					"name": "hello-log",
					"created_with": "BaudBound Test",
					"created_at": "2026-01-01T00:00:00.000Z",
					"minimum_runner_version": "0.1.0"
				}"#,
            extra_files,
        )
    }

    fn create_test_package_with_manifest(manifest: &str, extra_files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in [
            ("manifest.json", manifest),
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

    fn create_test_package_with_assets(assets: &[(&str, &str, &str, &[u8])]) -> Vec<u8> {
        let manifest_assets = assets
            .iter()
            .map(|(id, path, media_type, bytes)| {
                format!(
                    r#"{{
                        "id": "{id}",
                        "kind": "audio",
                        "media_type": "{media_type}",
                        "name": "{id}",
                        "path": "{path}",
                        "size": {}
                    }}"#,
                    bytes.len()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in [
            (
                "manifest.json",
                format!(
                    r#"{{
                        "format_version": 1,
                        "script_language_version": 1,
                        "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
                        "name": "hello-log",
                        "created_with": "BaudBound Test",
                        "created_at": "2026-01-01T00:00:00.000Z",
                        "minimum_runner_version": "0.1.0",
                        "assets": [{manifest_assets}]
                    }}"#
                ),
            ),
            (
                "program.json",
                r#"{
                    "entry": {
                        "trigger": {"id": "n-1"},
                        "triggers": [],
                        "program": {"type": "block", "steps": [], "edges": []}
                    }
                }"#
                .to_owned(),
            ),
            (
                "permissions.json",
                r#"{"declared_permissions": [], "risk_level": "low"}"#.to_owned(),
            ),
            (
                "capabilities.json",
                r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#.to_owned(),
            ),
        ] {
            writer
                .start_file(path, options)
                .expect("test zip file should start");
            writer
                .write_all(content.as_bytes())
                .expect("test zip content should write");
        }

        for (_, path, _, bytes) in assets {
            writer
                .start_file(*path, options)
                .expect("test asset file should start");
            writer
                .write_all(bytes)
                .expect("test asset bytes should write");
        }

        writer
            .finish()
            .expect("test zip should finish")
            .into_inner()
    }
}
