use std::sync::OnceLock;

use serde::Deserialize;

const PACKAGE_LIMITS_JSON: &str = include_str!("../../../../schemas/package-limits.json");
const PACKAGE_LIMITS_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub(super) struct PackageLimits {
    pub expansion_ratio_minimum_bytes: u64,
    pub max_asset_bytes: u64,
    pub max_entry_count: usize,
    pub max_expansion_ratio: u64,
    pub max_metadata_bytes: u64,
    pub max_total_uncompressed_bytes: u64,
    version: u32,
}

pub(super) fn package_limits() -> &'static PackageLimits {
    static LIMITS: OnceLock<PackageLimits> = OnceLock::new();
    LIMITS.get_or_init(|| {
        let limits = serde_json::from_str::<PackageLimits>(PACKAGE_LIMITS_JSON)
            .expect("embedded package limits must be valid JSON");
        assert_eq!(
            limits.version, PACKAGE_LIMITS_VERSION,
            "embedded package limits version must be supported"
        );
        assert!(
            limits.max_entry_count > 0
                && limits.max_metadata_bytes > 0
                && limits.max_asset_bytes > 0
                && limits.max_total_uncompressed_bytes >= limits.max_asset_bytes
                && limits.max_expansion_ratio > 0,
            "embedded package limits must be positive and internally consistent"
        );
        limits
    })
}
