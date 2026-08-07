//! BaudBound script package models and `.bbs` package loading.

mod color_match;
mod hotkey;
mod identifier;
mod package;
mod repository;
mod safe_integer;
mod types;

pub use color_match::{
    ColorComparisonMode, ColorMatchEvaluation, RgbColor, evaluate_color_match, parse_rgb_color,
};
pub use hotkey::{hotkey_error, is_hotkey};
pub use identifier::is_user_identifier;
pub use package::{
    MAX_SCRIPT_SETTING_CONTAINER_ITEMS, MAX_SCRIPT_SETTING_VALUE_DEPTH, PackageAsset, PackageEntry,
    PackageLoadError, PackageSummary, ScriptPackage, load_script_package,
    load_script_package_reader, max_package_archive_bytes, read_package_asset,
    read_package_asset_reader, validate_asset_package_path, validate_resolved_numeric_config,
    validate_script_setting_value_limits,
};
pub use repository::{
    MAX_RELEASE_NOTES_BYTES, MAX_REPOSITORY_BYTES, MAX_REPOSITORY_SCRIPTS, PublicPackageUrl,
    PublicRepositoryUrl, SCRIPT_REPOSITORY_FORMAT, SCRIPT_REPOSITORY_FORMAT_VERSION,
    ScriptRepository, ScriptRepositoryEntry, ScriptRepositoryError, ScriptRepositoryRelease,
    parse_script_repository, repository_capability_names, repository_permission_names,
    validate_anonymous_public_https_url, validate_public_https_package_url,
    validate_public_https_repository_url, validate_script_repository,
};
pub use safe_integer::{MAX_SAFE_INTEGER, is_safe_integer};
pub use types::{
    Capabilities, DefaultVariable, EditorMetadata, Manifest, ManifestAsset, Permissions, Program,
    RiskLevel, ScriptSettingDeclaration, SecretDeclaration,
};
