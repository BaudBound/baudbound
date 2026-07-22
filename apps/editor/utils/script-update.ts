export const SCRIPT_UPDATE_FORMAT = "baudbound.script-update" as const;
export const SCRIPT_UPDATE_FORMAT_VERSION = 1 as const;
export const DEFAULT_SCRIPT_VERSION = "1.0.0";
export const MAX_RELEASE_NOTES_LENGTH = 65_536;

export type ScriptUpdateDescriptor = {
	format: typeof SCRIPT_UPDATE_FORMAT;
	format_version: typeof SCRIPT_UPDATE_FORMAT_VERSION;
	script_id: string;
	latest: ScriptUpdateRelease;
};

export type ScriptUpdateRelease = {
	version: string;
	package_url: string;
	sha256: string;
	size: number;
	published_at: string;
	release_notes: string;
};

const semanticVersionPattern =
	/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
const sha256Pattern = /^[0-9a-f]{64}$/;
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isSemanticVersion(value: string) {
	return value.length <= 128 && semanticVersionPattern.test(value);
}

export function getScriptVersionError(value: string) {
	if (!value.trim()) return "Script version is required.";
	if (value !== value.trim()) return "Script version cannot start or end with spaces.";
	return isSemanticVersion(value) ? "" : "Use a semantic version such as 1.0.0.";
}

export function getUpdateDescriptorUrlError(value: string) {
	if (!value.trim()) return "";
	return validateRemoteUrl(value, "update.json");
}

export function getDirectPackageUrlError(value: string) {
	if (!value.trim()) return "Package URL is required.";
	return validateRemoteUrl(value, ".bbs");
}

export async function createScriptUpdateDescriptor(params: {
	bytes: Uint8Array;
	packageUrl: string;
	releaseNotes: string;
	scriptId: string;
	version: string;
	publishedAt?: Date;
}): Promise<ScriptUpdateDescriptor> {
	const packageUrlError = getDirectPackageUrlError(params.packageUrl);
	if (packageUrlError) throw new Error(packageUrlError);
	const versionError = getScriptVersionError(params.version);
	if (versionError) throw new Error(versionError);
	if (!uuidPattern.test(params.scriptId)) throw new Error("Project ID must be a UUID.");
	if (params.bytes.byteLength === 0) throw new Error("The generated package is empty.");
	if (params.releaseNotes.length > MAX_RELEASE_NOTES_LENGTH) {
		throw new Error(`Release notes cannot exceed ${MAX_RELEASE_NOTES_LENGTH} characters.`);
	}
	if (containsUnsafeReleaseNoteCharacters(params.releaseNotes)) {
		throw new Error("Release notes contain unsupported control characters.");
	}

	const descriptor: ScriptUpdateDescriptor = {
		format: SCRIPT_UPDATE_FORMAT,
		format_version: SCRIPT_UPDATE_FORMAT_VERSION,
		script_id: params.scriptId,
		latest: {
			version: params.version,
			package_url: params.packageUrl.trim(),
			sha256: await sha256Hex(params.bytes),
			size: params.bytes.byteLength,
			published_at: (params.publishedAt ?? new Date()).toISOString(),
			release_notes: params.releaseNotes,
		},
	};

	const errors = validateScriptUpdateDescriptor(descriptor);
	if (errors.length > 0) throw new Error(errors.join(" "));
	return descriptor;
}

export function validateScriptUpdateDescriptor(value: unknown) {
	const errors: string[] = [];
	if (!isRecord(value)) return ["Update descriptor must be an object."];
	validateExactKeys(value, ["format", "format_version", "script_id", "latest"], "Update descriptor", errors);
	if (value.format !== SCRIPT_UPDATE_FORMAT) errors.push("Update descriptor format is unsupported.");
	if (value.format_version !== SCRIPT_UPDATE_FORMAT_VERSION) {
		errors.push("Update descriptor format version is unsupported.");
	}
	if (typeof value.script_id !== "string" || !uuidPattern.test(value.script_id)) {
		errors.push("Update descriptor script_id must be a UUID.");
	}
	if (!isRecord(value.latest)) {
		errors.push("Update descriptor latest release must be an object.");
		return errors;
	}

	const latest = value.latest;
	validateExactKeys(
		latest,
		["version", "package_url", "sha256", "size", "published_at", "release_notes"],
		"Update descriptor latest release",
		errors,
	);
	if (typeof latest.version !== "string" || !isSemanticVersion(latest.version)) {
		errors.push("Update descriptor latest version must be a semantic version.");
	}
	if (typeof latest.package_url !== "string") {
		errors.push("Update descriptor package URL must be a string.");
	} else {
		const error = getDirectPackageUrlError(latest.package_url);
		if (error) errors.push(error);
	}
	if (typeof latest.sha256 !== "string" || !sha256Pattern.test(latest.sha256)) {
		errors.push("Update descriptor SHA256 must contain 64 lowercase hexadecimal characters.");
	}
	if (!Number.isSafeInteger(latest.size) || (latest.size as number) < 1) {
		errors.push("Update descriptor package size must be a positive safe integer.");
	}
	if (typeof latest.published_at !== "string" || !isUtcTimestamp(latest.published_at)) {
		errors.push("Update descriptor publication time must be a UTC timestamp.");
	}
	if (typeof latest.release_notes !== "string" || latest.release_notes.length > MAX_RELEASE_NOTES_LENGTH) {
		errors.push(`Update descriptor release notes cannot exceed ${MAX_RELEASE_NOTES_LENGTH} characters.`);
	} else if (containsUnsafeReleaseNoteCharacters(latest.release_notes)) {
		errors.push("Update descriptor release notes contain unsupported control characters.");
	}
	return errors;
}

export function downloadBytes(bytes: Uint8Array, filename: string, type: string) {
	const blob = new Blob([bytes.slice().buffer], { type });
	const url = URL.createObjectURL(blob);
	const link = document.createElement("a");
	try {
		link.href = url;
		link.download = filename;
		link.hidden = true;
		document.body.append(link);
		link.click();
	} finally {
		link.remove();
		window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
	}
}

function validateRemoteUrl(value: string, requiredFilename: "update.json" | ".bbs") {
	if (value.length > 2048) return "URL cannot exceed 2048 characters.";
	if (value !== value.trim()) return "URL cannot start or end with spaces.";
	try {
		const url = new URL(value);
		if (url.protocol !== "https:") return "Use an HTTPS URL.";
		if (!url.hostname) return "URL must include a host.";
		if (url.username || url.password) return "URL cannot contain a username or password.";
		if (url.hash) return "URL cannot contain a fragment.";
		const lastSegment = decodeURIComponent(url.pathname.split("/").at(-1) ?? "");
		if (requiredFilename === "update.json" && lastSegment !== requiredFilename) {
			return "Update URL must point to a file named update.json.";
		}
		if (requiredFilename === ".bbs" && !lastSegment.toLowerCase().endsWith(requiredFilename)) {
			return "Package URL must point to a .bbs file.";
		}
		return "";
	} catch {
		return "Use a valid URL.";
	}
}

async function sha256Hex(bytes: Uint8Array) {
	const digest = await crypto.subtle.digest("SHA-256", bytes.slice().buffer);
	return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function isUtcTimestamp(value: string) {
	return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value) && Number.isFinite(Date.parse(value));
}

function validateExactKeys(value: Record<string, unknown>, keys: string[], label: string, errors: string[]) {
	const expected = new Set(keys);
	for (const key of Object.keys(value)) {
		if (!expected.has(key)) errors.push(`${label} contains unknown field ${key}.`);
	}
	for (const key of keys) {
		if (!(key in value)) errors.push(`${label} is missing ${key}.`);
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function containsUnsafeReleaseNoteCharacters(value: string) {
	for (const character of value) {
		const codePoint = character.codePointAt(0) ?? 0;
		const allowedWhitespace = character === "\n" || character === "\r" || character === "\t";
		const controlCharacter = (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) && !allowedWhitespace;
		const bidiControl =
			codePoint === 0x061c ||
			codePoint === 0x200e ||
			codePoint === 0x200f ||
			(codePoint >= 0x202a && codePoint <= 0x202e) ||
			(codePoint >= 0x2066 && codePoint <= 0x2069);
		if (controlCharacter || bidiControl) return true;
	}
	return false;
}
