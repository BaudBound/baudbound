//! BaudBound script package models and `.bbs` package loading.

mod color_match;
mod package;
mod types;

pub use color_match::{
    ColorComparisonMode, ColorMatchEvaluation, RgbColor, evaluate_color_match, parse_rgb_color,
};
pub use package::{
    PackageAsset, PackageEntry, PackageLoadError, PackageSummary, ScriptPackage,
    load_script_package, load_script_package_reader, read_package_asset, read_package_asset_reader,
    validate_asset_package_path, validate_resolved_numeric_config,
};
pub use types::{
    Capabilities, DefaultVariable, EditorMetadata, Manifest, ManifestAsset, Permissions, Program,
    RiskLevel, SecretDeclaration,
};
