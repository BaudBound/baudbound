import limits from "../../../../schemas/package-limits.json";

export const packageLimits = limits;

export function validatePackageEntryLimits(entries: Array<{ compressedSize?: number; path: string; size?: number }>) {
	const errors: string[] = [];
	if (entries.length > packageLimits.max_entry_count) {
		errors.push(`Package contains ${entries.length} entries. The maximum is ${packageLimits.max_entry_count}.`);
	}

	let totalUncompressed = 0;
	for (const entry of entries) {
		if (entry.size === undefined) {
			errors.push(`${entry.path}: uncompressed size is unavailable.`);
			continue;
		}
		const maximum = entry.path.startsWith("assets/") ? packageLimits.max_asset_bytes : packageLimits.max_metadata_bytes;
		if (entry.size > maximum) {
			errors.push(`${entry.path}: size ${entry.size} bytes exceeds the maximum of ${maximum} bytes.`);
		}
		totalUncompressed += entry.size;
		if (
			entry.size >= packageLimits.expansion_ratio_minimum_bytes &&
			entry.compressedSize !== undefined &&
			(entry.compressedSize === 0 || entry.size / entry.compressedSize > packageLimits.max_expansion_ratio)
		) {
			errors.push(`${entry.path}: archive expansion exceeds ${packageLimits.max_expansion_ratio}:1.`);
		}
	}
	if (totalUncompressed > packageLimits.max_total_uncompressed_bytes) {
		errors.push(
			`Package uncompressed size ${totalUncompressed} bytes exceeds the maximum of ${packageLimits.max_total_uncompressed_bytes} bytes.`,
		);
	}
	return errors;
}
