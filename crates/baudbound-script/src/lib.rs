//! BaudBound script package models and `.bbs` package loading.

mod package;
mod types;

pub use package::{
    PackageAsset, PackageEntry, PackageLoadError, PackageSummary, ScriptPackage,
    load_script_package, load_script_package_reader, read_package_asset, read_package_asset_reader,
    validate_asset_package_path,
};
pub use types::{
    Capabilities, EditorMetadata, Manifest, ManifestAsset, Permissions, Program, RiskLevel,
    SecretDeclaration,
};
