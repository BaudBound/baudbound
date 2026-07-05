import type { AssetKind, AssetManifestEntry, EditorAsset } from "@/lib/types";

export const ASSET_PACKAGE_DIR = "assets";

export type AssetValidationResult = {
	errors: string[];
	warnings: string[];
};

export type PackageAssetEntry = {
	path: string;
	size?: number;
};

type AssetTypeRule = {
	extensions: string[];
	kind: AssetKind;
	mediaTypes: string[];
};

type AssetContentValidation =
	| {
			mediaType?: string;
			ok: true;
	  }
	| {
			ok: false;
			reason: string;
	  };

const assetTypeRules: AssetTypeRule[] = [
	{
		kind: "audio",
		extensions: ["mp3", "wav", "ogg", "flac", "m4a"],
		mediaTypes: ["audio/mpeg", "audio/wav", "audio/x-wav", "audio/ogg", "audio/flac", "audio/mp4"],
	},
	{
		kind: "image",
		extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg"],
		mediaTypes: ["image/png", "image/jpeg", "image/webp", "image/gif", "image/svg+xml"],
	},
	{
		kind: "text",
		extensions: ["txt", "json", "csv"],
		mediaTypes: ["text/plain", "application/json", "text/csv"],
	},
];

const allowedExtensions = new Set(assetTypeRules.flatMap((rule) => rule.extensions));
export const supportedAssetExtensions = [...allowedExtensions].sort();

export async function createEditorAssets(files: File[], existingAssets: EditorAsset[]) {
	const accepted: EditorAsset[] = [];
	const rejected: string[] = [];
	const reservedPaths = new Set(existingAssets.map((asset) => asset.packagePath.toLowerCase()));

	for (const file of files) {
		const extension = getExtension(file.name);
		const rule = extension ? getAssetRuleByExtension(extension) : undefined;

		if (!rule || !extension) {
			rejected.push(`${file.name}: unsupported asset type.`);
			continue;
		}

		if (file.type && !rule.mediaTypes.includes(file.type)) {
			rejected.push(`${file.name}: browser media type ${file.type} does not match .${extension}.`);
			continue;
		}

		const contentValidation = await validateAssetFileContent(file, extension);
		if (!contentValidation.ok) {
			rejected.push(`${file.name}: ${contentValidation.reason}`);
			continue;
		}

		const name = sanitizeAssetFileName(file.name);
		const packagePath = createUniqueAssetPath(name, reservedPaths);
		reservedPaths.add(packagePath.toLowerCase());

		accepted.push({
			id: `asset-${crypto.randomUUID()}`,
			createdAt: new Date().toISOString(),
			file,
			kind: rule.kind,
			mediaType: (contentValidation.mediaType ?? file.type) || inferMediaType(extension, rule.kind),
			name,
			packagePath,
			size: file.size,
		});
	}

	return { accepted, rejected };
}

export function toAssetManifestEntry(asset: EditorAsset): AssetManifestEntry {
	return {
		id: asset.id,
		kind: asset.kind,
		mediaType: asset.mediaType,
		name: asset.name,
		packagePath: asset.packagePath,
		size: asset.size,
	};
}

export function validateEditorAssets(assets: EditorAsset[]): AssetValidationResult {
	const errors: string[] = [];
	const warnings: string[] = [];
	const paths = new Set<string>();

	for (const asset of assets) {
		const pathError = validateAssetPackagePath(asset.packagePath);
		if (pathError) {
			errors.push(`${asset.name}: ${pathError}`);
		}

		const normalizedPath = asset.packagePath.toLowerCase();
		if (paths.has(normalizedPath)) {
			errors.push(`${asset.name}: duplicate package path ${asset.packagePath}.`);
		}
		paths.add(normalizedPath);

		const extension = getExtension(asset.name);
		const rule = extension ? getAssetRuleByExtension(extension) : undefined;
		if (!rule || !rule.mediaTypes.includes(asset.mediaType)) {
			errors.push(`${asset.name}: unsupported asset type.`);
		} else if (rule.kind !== asset.kind) {
			errors.push(`${asset.name}: asset kind ${asset.kind} does not match ${asset.mediaType}.`);
		}
	}

	return { errors, warnings };
}

export function validatePackageAssetPaths(fileNames: string[]): AssetValidationResult {
	return validatePackageAssetEntries(fileNames.map((fileName) => ({ path: fileName })));
}

export function validatePackageAssetEntries(entries: PackageAssetEntry[]): AssetValidationResult {
	const errors: string[] = [];
	const warnings: string[] = [];
	const assetEntries = entries.filter((entry) => entry.path.startsWith(`${ASSET_PACKAGE_DIR}/`));
	const paths = new Set<string>();

	for (const entry of assetEntries) {
		const pathError = validateAssetPackagePath(entry.path);
		if (pathError) {
			errors.push(`${entry.path}: ${pathError}`);
			continue;
		}

		const normalizedPath = entry.path.toLowerCase();
		if (paths.has(normalizedPath)) {
			errors.push(`${entry.path}: duplicate asset path.`);
		}
		paths.add(normalizedPath);

		const extension = getExtension(entry.path);
		if (!extension || !allowedExtensions.has(extension)) {
			errors.push(`${entry.path}: unsupported asset extension.`);
		}

		if (entry.size !== undefined) {
			if (!Number.isFinite(entry.size) || entry.size < 0) {
				errors.push(`${entry.path}: asset size is invalid.`);
			}
		}
	}

	return { errors, warnings };
}

export function isAllowedPackageFile(fileName: string) {
	return (
		fileName === "manifest.json" ||
		fileName === "program.json" ||
		fileName === "editor.json" ||
		fileName === "permissions.json" ||
		fileName === "capabilities.json" ||
		fileName === "README.md" ||
		fileName.startsWith(`${ASSET_PACKAGE_DIR}/`)
	);
}

export function formatBytes(bytes: number) {
	if (bytes < 1024) {
		return `${bytes} B`;
	}

	if (bytes < 1024 * 1024) {
		return `${(bytes / 1024).toFixed(1)} KB`;
	}

	return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function validateAssetPackagePath(packagePath: string) {
	if (!packagePath.startsWith(`${ASSET_PACKAGE_DIR}/`)) {
		return `asset path must be inside ${ASSET_PACKAGE_DIR}/.`;
	}

	if (hasControlCharacter(packagePath)) {
		return "asset path cannot contain control characters.";
	}

	if (packagePath.includes("\\") || packagePath.startsWith("/") || packagePath.includes(":")) {
		return "asset path must be relative and cannot contain path traversal.";
	}

	if (packagePath.endsWith("/") || packagePath === `${ASSET_PACKAGE_DIR}/`) {
		return "asset path must point to a file.";
	}

	const segments = packagePath.split("/");
	if (segments.some((segment) => segment === "" || segment === "." || segment === "..")) {
		return "asset path must not contain empty, current-directory, or parent-directory segments.";
	}

	return "";
}

function createUniqueAssetPath(fileName: string, reservedPaths: Set<string>) {
	const extension = getExtension(fileName);
	const baseName = extension ? fileName.slice(0, -(extension.length + 1)) : fileName;
	let candidate = `${ASSET_PACKAGE_DIR}/${fileName}`;
	let index = 2;

	while (reservedPaths.has(candidate.toLowerCase())) {
		candidate = `${ASSET_PACKAGE_DIR}/${baseName}-${index}.${extension}`;
		index += 1;
	}

	return candidate;
}

function sanitizeAssetFileName(fileName: string) {
	const extension = getExtension(fileName);
	const rawBaseName = extension ? fileName.slice(0, -(extension.length + 1)) : fileName;
	const baseName =
		rawBaseName
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9._-]+/g, "-")
			.replace(/^-+|-+$/g, "") || "asset";

	return extension ? `${baseName}.${extension}` : baseName;
}

function getAssetRuleByExtension(extension: string) {
	return assetTypeRules.find((rule) => rule.extensions.includes(extension));
}

export function getAssetKindForMediaType(mediaType: string): AssetKind | null {
	return assetTypeRules.find((rule) => rule.mediaTypes.includes(mediaType))?.kind ?? null;
}

function inferMediaType(extension: string, kind: AssetKind) {
	if (extension === "json") {
		return "application/json";
	}

	if (extension === "svg") {
		return "image/svg+xml";
	}

	if (extension === "jpg") {
		return "image/jpeg";
	}

	if (extension === "mp3") {
		return "audio/mpeg";
	}

	return `${kind}/${extension}`;
}

function getExtension(fileName: string) {
	const extension = fileName.split(".").pop()?.trim().toLowerCase();
	return extension && extension !== fileName.toLowerCase() ? extension : "";
}

export async function validateAssetFileContent(file: File, extension: string): Promise<AssetContentValidation> {
	if (file.size === 0) {
		return { ok: false, reason: "empty files are not allowed." };
	}

	if (extension === "svg") {
		return validateSvgAsset(await file.text());
	}

	if (extension === "json") {
		return validateJsonAsset(await file.text());
	}

	if (extension === "txt" || extension === "csv") {
		return validatePlainTextAsset(await file.arrayBuffer(), extension);
	}

	const header = new Uint8Array(await file.slice(0, 64).arrayBuffer());
	const detectedMediaType = detectBinaryAssetMediaType(header);

	if (!detectedMediaType) {
		return { ok: false, reason: `file content does not match a supported .${extension} signature.` };
	}

	const expectedMediaTypes = getExpectedMediaTypesForExtension(extension);
	return expectedMediaTypes.includes(detectedMediaType)
		? { ok: true, mediaType: detectedMediaType }
		: {
				ok: false,
				reason: `file content looks like ${detectedMediaType}, not .${extension}.`,
			};
}

function validateSvgAsset(text: string): AssetContentValidation {
	const trimmed = stripUtf8Bom(text).trimStart();

	if (!/^<\?xml[\s\S]*?<svg[\s>]/i.test(trimmed) && !/^<svg[\s>]/i.test(trimmed)) {
		return { ok: false, reason: "SVG content must start with an <svg> document." };
	}

	const unsafePatterns = [
		/<script[\s>]/i,
		/<foreignObject[\s>]/i,
		/\son[a-z]+\s*=/i,
		/javascript\s*:/i,
		/data\s*:\s*text\/html/i,
	];

	if (unsafePatterns.some((pattern) => pattern.test(text))) {
		return { ok: false, reason: "SVG assets cannot contain scripts, event handlers, or embedded HTML." };
	}

	return { ok: true, mediaType: "image/svg+xml" };
}

function validateJsonAsset(text: string): AssetContentValidation {
	try {
		JSON.parse(stripUtf8Bom(text));
		return { ok: true, mediaType: "application/json" };
	} catch {
		return { ok: false, reason: "JSON assets must contain valid JSON." };
	}
}

function validatePlainTextAsset(buffer: ArrayBuffer, extension: string): AssetContentValidation {
	const bytes = new Uint8Array(buffer);

	if (bytes.includes(0)) {
		return { ok: false, reason: "text assets cannot contain binary null bytes." };
	}

	const controlBytes = bytes.filter((byte) => byte < 32 && byte !== 9 && byte !== 10 && byte !== 13);
	if (controlBytes.length > 0) {
		return { ok: false, reason: "text assets contain unsupported control characters." };
	}

	try {
		new TextDecoder("utf-8", { fatal: true }).decode(bytes);
	} catch {
		return { ok: false, reason: "text assets must be valid UTF-8." };
	}

	return { ok: true, mediaType: extension === "csv" ? "text/csv" : "text/plain" };
}

function detectBinaryAssetMediaType(header: Uint8Array) {
	if (startsWithBytes(header, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
		return "image/png";
	}

	if (startsWithBytes(header, [0xff, 0xd8, 0xff])) {
		return "image/jpeg";
	}

	if (startsWithAscii(header, "GIF87a") || startsWithAscii(header, "GIF89a")) {
		return "image/gif";
	}

	if (startsWithAscii(header, "RIFF") && asciiAt(header, 8, 4) === "WEBP") {
		return "image/webp";
	}

	if (startsWithAscii(header, "RIFF") && asciiAt(header, 8, 4) === "WAVE") {
		return "audio/wav";
	}

	if (startsWithAscii(header, "OggS")) {
		return "audio/ogg";
	}

	if (startsWithAscii(header, "fLaC")) {
		return "audio/flac";
	}

	if (startsWithAscii(header, "ID3") || (header[0] === 0xff && (header[1] & 0xe0) === 0xe0)) {
		return "audio/mpeg";
	}

	if (asciiAt(header, 4, 4) === "ftyp") {
		return "audio/mp4";
	}

	return "";
}

function getExpectedMediaTypesForExtension(extension: string) {
	if (extension === "jpg" || extension === "jpeg") {
		return ["image/jpeg"];
	}

	if (extension === "wav") {
		return ["audio/wav"];
	}

	if (extension === "m4a") {
		return ["audio/mp4"];
	}

	const rule = getAssetRuleByExtension(extension);
	return rule?.mediaTypes ?? [];
}

function startsWithBytes(bytes: Uint8Array, expected: number[]) {
	return expected.every((byte, index) => bytes[index] === byte);
}

function startsWithAscii(bytes: Uint8Array, expected: string) {
	return asciiAt(bytes, 0, expected.length) === expected;
}

function asciiAt(bytes: Uint8Array, start: number, length: number) {
	return String.fromCharCode(...bytes.slice(start, start + length));
}

function stripUtf8Bom(text: string) {
	return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

function hasControlCharacter(value: string) {
	return [...value].some((character) => {
		const code = character.charCodeAt(0);
		return code < 32 || code === 127;
	});
}
