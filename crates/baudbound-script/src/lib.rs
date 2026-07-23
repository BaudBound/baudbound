//! BaudBound script package models and `.bbs` package loading.

mod color_match;
mod package;
mod repository;
mod types;

pub use color_match::{
    ColorComparisonMode, ColorMatchEvaluation, RgbColor, evaluate_color_match, parse_rgb_color,
};
pub use package::{
    PackageAsset, PackageEntry, PackageLoadError, PackageSummary, ScriptPackage,
    load_script_package, load_script_package_reader, read_package_asset, read_package_asset_reader,
    validate_asset_package_path, validate_resolved_numeric_config,
};
pub use repository::{
    MAX_RELEASE_NOTES_BYTES, MAX_REPOSITORY_BYTES, MAX_REPOSITORY_SCRIPTS,
    SCRIPT_REPOSITORY_FORMAT, SCRIPT_REPOSITORY_FORMAT_VERSION, ScriptRepository,
    ScriptRepositoryEntry, ScriptRepositoryError, ScriptRepositoryRelease, parse_script_repository,
    repository_capability_names, repository_permission_names, validate_public_https_package_url,
    validate_public_https_repository_url, validate_script_repository,
};
pub use types::{
    Capabilities, DefaultVariable, EditorMetadata, Manifest, ManifestAsset, Permissions, Program,
    RiskLevel, SecretDeclaration,
};
