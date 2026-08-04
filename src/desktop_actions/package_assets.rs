use std::io::Cursor;

use baudbound_runtime::RuntimeContext;
use baudbound_script::{PackageAsset, read_package_asset, read_package_asset_reader};

pub(super) fn read_context_package_asset(
    context: &RuntimeContext,
    asset_reference: &str,
) -> Result<PackageAsset, String> {
    if let Some(bytes) = context.package_bytes.as_ref() {
        return read_package_asset_reader(Cursor::new(bytes.as_ref()), asset_reference)
            .map_err(|source| source.to_string());
    }

    let package_path = context
        .package_path
        .as_ref()
        .ok_or_else(|| "an installed package context is required".to_owned())?;
    read_package_asset(package_path, asset_reference).map_err(|source| source.to_string())
}
