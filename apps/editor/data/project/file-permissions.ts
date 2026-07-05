import type { JsonValue, PermissionSummary } from "@/lib/types";

const sensitivePathMarkers = [
	"/.aws/",
	"/.azure/",
	"/.config/",
	"/.docker/",
	"/.gnupg/",
	"/.kube/",
	"/.ssh/",
	"/etc/",
	"/root/",
	"/var/lib/",
	"/windows/system32/",
	"/windows/syswow64/",
	"/programdata/",
	"/appdata/local/",
	"/appdata/roaming/",
];

export const fileReadPermission: PermissionSummary = { name: "file_read", risk: "medium" };
export const readSensitiveFilePermission: PermissionSummary = { name: "read_sensitive_file", risk: "dangerous" };
export const fileWriteLimitedPermission: PermissionSummary = { name: "file_write_limited", risk: "high" };
export const writeAnyFilePermission: PermissionSummary = { name: "write_any_file", risk: "dangerous" };

export function createReadFilePermission(path: JsonValue | undefined): PermissionSummary {
	return isSensitiveOrUnboundedPath(configString(path)) ? readSensitiveFilePermission : fileReadPermission;
}

export function createWriteFilePermission(path: JsonValue | undefined): PermissionSummary {
	return isUnboundedWritePath(configString(path)) ? writeAnyFilePermission : fileWriteLimitedPermission;
}

export function isSensitiveOrUnboundedPath(path: string) {
	const normalized = normalizePathForPolicy(path);
	if (!normalized) {
		return false;
	}

	return pathUsesRuntimeData(path) || isAbsolutePath(normalized) || isSensitivePath(normalized);
}

export function isUnboundedWritePath(path: string) {
	const normalized = normalizePathForPolicy(path);
	if (!normalized) {
		return false;
	}

	return pathUsesRuntimeData(path) || isAbsolutePath(normalized) || isSensitivePath(normalized);
}

function isSensitivePath(normalizedPath: string) {
	return sensitivePathMarkers.some((marker) => normalizedPath.includes(marker));
}

function isAbsolutePath(normalizedPath: string) {
	return (
		normalizedPath.startsWith("/") ||
		/^[a-z]:\//.test(normalizedPath) ||
		normalizedPath.startsWith("//") ||
		normalizedPath.startsWith("~/")
	);
}

function pathUsesRuntimeData(path: string) {
	return /\{\{[^}]+\}\}/.test(path);
}

function normalizePathForPolicy(path: string) {
	return path.trim().replaceAll("\\", "/").replaceAll(/\/+/g, "/").toLowerCase();
}

function configString(value: JsonValue | undefined) {
	return typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? String(value) : "";
}
