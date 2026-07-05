import type { Edge, Node } from "@xyflow/react";
import JSZip from "jszip";
import { getNodeDefinition, getNodePorts, getRuntimeDataOutputs } from "@/data/nodes/registry";
import { toAssetManifestEntry, validateAssetFileContent } from "@/data/project/assets";
import { targetRuntimes } from "@/data/project/runtimes";
import type {
	ActionType,
	AssetKind,
	EditorAsset,
	JsonValue,
	ProjectSettings,
	RiskLevel,
	ScriptNodeData,
	TargetRuntime,
} from "../lib/types";
import { calculateCapabilities, calculatePermissions, calculateRiskLevel, toProgramJson } from "./analysis";
import { createPackageVerificationChecks, getRequiredPackageFiles, summarizeVerification } from "./verification";

type ImportedBbsPackage = {
	assets: EditorAsset[];
	edges: Edge[];
	projectSettings: ProjectSettings;
	nodes: Node<ScriptNodeData>[];
};

const EDITOR_PACKAGE_FILE = "editor.json";
const EDITOR_METADATA_FORMAT_VERSION = 1;

export async function exportBbsPackage(params: {
	projectSettings: ProjectSettings;
	nodes: Node<ScriptNodeData>[];
	edges: Edge[];
	assets: EditorAsset[];
}) {
	const permissions = calculatePermissions(params.nodes);
	const capabilities = calculateCapabilities(params.nodes);
	const assetManifest = params.assets.map(toAssetManifestEntry);
	const now = new Date().toISOString();
	const zip = new JSZip();

	zip.file(
		"manifest.json",
		JSON.stringify(
			compactObject({
				format_version: 1,
				script_language_version: 1,
				id: crypto.randomUUID(),
				name: params.projectSettings.name,
				description: params.projectSettings.description,
				author: params.projectSettings.author,
				website: params.projectSettings.website,
				repository: params.projectSettings.repository,
				created_with: "BaudBound Editor 0.1.0",
				created_at: now,
				updated_at: now,
				tags: params.projectSettings.tags,
				minimum_runner_version: params.projectSettings.minimumRunnerVersion,
				assets: assetManifest.map((asset) => ({
					id: asset.id,
					kind: asset.kind,
					media_type: asset.mediaType,
					name: asset.name,
					path: asset.packagePath,
					size: asset.size,
				})),
			}),
			null,
			2,
		),
	);

	zip.file("program.json", JSON.stringify(toProgramJson(params.nodes, params.edges, params.projectSettings), null, 2));
	zip.file(EDITOR_PACKAGE_FILE, JSON.stringify(toEditorJson(params.nodes), null, 2));
	for (const asset of params.assets) {
		zip.file(asset.packagePath, asset.file, { binary: true });
	}
	zip.file(
		"permissions.json",
		JSON.stringify(
			{
				declared_permissions: permissions.map((permission) => permission.name),
				risk_level: calculateRiskLevel(permissions),
			},
			null,
			2,
		),
	);
	zip.file(
		"capabilities.json",
		JSON.stringify(
			{
				required_capabilities: capabilities.map((capability) => capability.name),
				target_runtime: params.projectSettings.targetRuntime,
			},
			null,
			2,
		),
	);
	zip.file("README.md", `# ${params.projectSettings.name}\n\nExported from BaudBound Editor.\n`);

	const blob = await zip.generateAsync({ type: "blob", compression: "DEFLATE" });
	const url = URL.createObjectURL(blob);
	try {
		const link = document.createElement("a");
		link.href = url;
		link.download = `${slugFromName(params.projectSettings.name)}.bbs`;
		link.click();
	} finally {
		window.setTimeout(() => URL.revokeObjectURL(url), 0);
	}
}

export async function inspectBbsPackage(file: File) {
	const zip = await JSZip.loadAsync(file);
	const names = Object.keys(zip.files).filter((name) => !zip.files[name]?.dir);
	return names.sort();
}

export async function verifyBbsPackage(file: File) {
	const zip = await JSZip.loadAsync(file);
	const fileNames = Object.keys(zip.files)
		.filter((name) => !zip.files[name]?.dir)
		.sort();
	const jsonFiles: Record<string, unknown> = {};
	const parseErrors: Record<string, string> = {};

	for (const fileName of getPackageJsonFiles(fileNames)) {
		const entry = zip.file(fileName);
		if (!entry) {
			continue;
		}

		try {
			jsonFiles[fileName] = JSON.parse(await entry.async("text")) as unknown;
		} catch (error) {
			parseErrors[fileName] = error instanceof Error ? error.message : "Unable to parse JSON.";
		}
	}

	const checks = createPackageVerificationChecks({ fileNames, jsonFiles, parseErrors });

	return {
		checks,
		contents: fileNames,
		summary: summarizeVerification(checks),
	};
}

export async function importBbsPackage(file: File): Promise<ImportedBbsPackage> {
	const zip = await JSZip.loadAsync(file);
	const fileNames = Object.keys(zip.files)
		.filter((name) => !zip.files[name]?.dir)
		.sort();
	const jsonFiles = await readPackageJsonFiles(zip);
	const checks = createPackageVerificationChecks({ fileNames, jsonFiles, parseErrors: {} });
	const summary = summarizeVerification(checks);

	if (summary.status === "failed") {
		throw new Error("Package failed verification and cannot be imported.");
	}

	const manifest = requireRecord(jsonFiles["manifest.json"], "manifest.json");
	const capabilities = requireRecord(jsonFiles["capabilities.json"], "capabilities.json");
	const program = requireRecord(jsonFiles["program.json"], "program.json");
	const editorMetadata = isRecord(jsonFiles[EDITOR_PACKAGE_FILE]) ? jsonFiles[EDITOR_PACKAGE_FILE] : null;

	const projectSettings = toProjectSettings(manifest, capabilities);
	const assets = await readPackageAssets(zip, manifest);
	const { nodes, edges } = toEditorGraph(program, editorMetadata);

	return {
		assets,
		edges,
		nodes,
		projectSettings,
	};
}

function compactObject(value: Record<string, unknown>) {
	return Object.fromEntries(
		Object.entries(value).filter(([, entry]) => {
			if (Array.isArray(entry)) {
				return entry.length > 0;
			}

			return entry !== "";
		}),
	);
}

function slugFromName(name: string) {
	return (
		name
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/^-+|-+$/g, "") || "untitled-script"
	);
}

function toEditorJson(nodes: Node<ScriptNodeData>[]) {
	return {
		format_version: EDITOR_METADATA_FORMAT_VERSION,
		created_with: "BaudBound Editor 0.1.0",
		nodes: nodes.map((node) => ({
			id: node.id,
			position: {
				x: finiteNumberOrZero(node.position.x),
				y: finiteNumberOrZero(node.position.y),
			},
		})),
	};
}

async function readPackageJsonFiles(zip: JSZip) {
	const jsonFiles: Record<string, unknown> = {};

	for (const fileName of getRequiredPackageFiles()) {
		const entry = zip.file(fileName);
		if (!entry) {
			throw new Error(`Package is missing ${fileName}.`);
		}

		jsonFiles[fileName] = JSON.parse(await entry.async("text")) as unknown;
	}

	const editorMetadataEntry = zip.file(EDITOR_PACKAGE_FILE);
	if (editorMetadataEntry) {
		jsonFiles[EDITOR_PACKAGE_FILE] = JSON.parse(await editorMetadataEntry.async("text")) as unknown;
	}

	return jsonFiles;
}

function toProjectSettings(manifest: Record<string, unknown>, capabilities: Record<string, unknown>): ProjectSettings {
	const targetRuntime = asTargetRuntime(capabilities.target_runtime);

	return {
		name: stringOrDefault(manifest.name, "untitled-script"),
		description: stringOrDefault(manifest.description, ""),
		author: stringOrDefault(manifest.author, ""),
		website: stringOrDefault(manifest.website, ""),
		repository: stringOrDefault(manifest.repository, ""),
		tags: Array.isArray(manifest.tags) ? manifest.tags.filter((tag): tag is string => typeof tag === "string") : [],
		targetRuntime,
		minimumRunnerVersion: stringOrDefault(manifest.minimum_runner_version, "0.1.0"),
	};
}

async function readPackageAssets(zip: JSZip, manifest: Record<string, unknown>): Promise<EditorAsset[]> {
	if (!Array.isArray(manifest.assets)) {
		return [];
	}

	const assets: EditorAsset[] = [];
	for (const entry of manifest.assets) {
		const asset = requireRecord(entry, "manifest asset");
		const packagePath = stringOrDefault(asset.path, "");
		const packageEntry = packagePath ? zip.file(packagePath) : null;

		if (!packageEntry) {
			throw new Error(`Package asset ${packagePath || "(missing path)"} is listed in manifest but missing from zip.`);
		}

		const name = stringOrDefault(asset.name, assetFileNameFromPath(packagePath));
		const mediaType = stringOrDefault(asset.media_type, "application/octet-stream");
		const kind = asAssetKind(asset.kind);
		const blob = await packageEntry.async("blob");
		const file = new File([blob], name, { type: mediaType });
		const extension = getExtension(packagePath);
		const contentValidation = await validateAssetFileContent(file, extension);

		if (!contentValidation.ok) {
			throw new Error(`${packagePath}: ${contentValidation.reason}`);
		}

		assets.push({
			id: stringOrDefault(asset.id, `asset-${crypto.randomUUID()}`),
			createdAt: new Date().toISOString(),
			file,
			kind,
			mediaType: contentValidation.mediaType ?? mediaType,
			name,
			packagePath,
			size: typeof asset.size === "number" ? asset.size : file.size,
		});
	}

	return assets;
}

function toEditorGraph(program: Record<string, unknown>, editorMetadata: Record<string, unknown> | null) {
	const entry = requireRecord(program.entry, "program.entry");
	const programBlock = requireRecord(entry.program, "program.entry.program");
	const triggers = Array.isArray(entry.triggers) ? entry.triggers : entry.trigger ? [entry.trigger] : [];
	const steps = Array.isArray(programBlock.steps) ? programBlock.steps : [];
	const positionsByNodeId = getEditorNodePositions(editorMetadata);
	const importedNodes = [...triggers, ...steps].map((nodeValue, index) =>
		toEditorNode(nodeValue, index, positionsByNodeId),
	);
	const nodeIds = new Set(importedNodes.map((node) => node.id));
	const edges = toEditorEdges(programBlock.edges, nodeIds);

	return {
		nodes: importedNodes,
		edges,
	};
}

function toEditorNode(
	value: unknown,
	index: number,
	positionsByNodeId: ReadonlyMap<string, { x: number; y: number }>,
): Node<ScriptNodeData> {
	const record = requireRecord(value, "program node");
	const id = stringOrDefault(record.id, `n-imported-${index + 1}`);
	const actionType = asActionType(record.action_type);
	const definition = getNodeDefinition(actionType);

	if (!definition) {
		throw new Error(`Unsupported node action type: ${actionType}.`);
	}

	const config = asConfig(record.config);
	const ports = getNodePorts(actionType, config);
	const runtimeOutputs = getRuntimeDataOutputs(actionType);
	const columns = 4;
	const columnGap = 300;
	const rowGap = 190;

	return {
		id,
		type: "scriptNode",
		position: positionsByNodeId.get(id) ?? {
			x: 96 + (index % columns) * columnGap,
			y: 80 + Math.floor(index / columns) * rowGap,
		},
		data: {
			label:
				definition.kind === "trigger" && !definition.label.endsWith("Trigger")
					? `${definition.label} Trigger`
					: definition.label,
			kind: definition.kind,
			actionType,
			risk: definition.risk as RiskLevel,
			config,
			inputs: ports.inputs,
			outputs: ports.outputs,
			runtimeOutputs,
		},
	};
}

function getEditorNodePositions(editorMetadata: Record<string, unknown> | null) {
	const positions = new Map<string, { x: number; y: number }>();
	if (!editorMetadata || !Array.isArray(editorMetadata.nodes)) {
		return positions;
	}

	for (const value of editorMetadata.nodes) {
		if (!isRecord(value) || typeof value.id !== "string" || !isRecord(value.position)) {
			continue;
		}

		const x = value.position.x;
		const y = value.position.y;
		if (typeof x === "number" && typeof y === "number" && Number.isFinite(x) && Number.isFinite(y)) {
			positions.set(value.id, { x, y });
		}
	}

	return positions;
}

function toEditorEdges(value: unknown, nodeIds: ReadonlySet<string>): Edge[] {
	if (!Array.isArray(value)) {
		return [];
	}

	return value.flatMap((edgeValue, index) => {
		const edge = requireRecord(edgeValue, "program edge");
		const source = stringOrDefault(edge.source, "");
		const target = stringOrDefault(edge.target, "");

		if (!source || !target || !nodeIds.has(source) || !nodeIds.has(target)) {
			return [];
		}

		const sourceHandle = optionalString(edge.source_handle);
		const targetHandle = optionalString(edge.target_handle);

		return [
			{
				id: `${source}-${sourceHandle ?? "out"}-${target}-${targetHandle ?? "input"}-${index}`,
				source,
				sourceHandle,
				target,
				targetHandle,
				type: "smoothstep",
			},
		];
	});
}

function asConfig(value: unknown): Record<string, JsonValue> {
	if (!isRecord(value)) {
		return {};
	}

	return Object.fromEntries(
		Object.entries(value).filter((entry): entry is [string, JsonValue] => isJsonValue(entry[1])),
	);
}

function asActionType(value: unknown): ActionType {
	if (typeof value !== "string" || !getNodeDefinition(value as ActionType)) {
		throw new Error(`Unsupported node action type: ${String(value)}.`);
	}

	return value as ActionType;
}

function asTargetRuntime(value: unknown): TargetRuntime {
	return typeof value === "string" && targetRuntimes.includes(value as TargetRuntime)
		? (value as TargetRuntime)
		: "Generic Headless";
}

function asAssetKind(value: unknown): AssetKind {
	return value === "audio" || value === "image" || value === "text" ? value : "text";
}

function requireRecord(value: unknown, label: string): Record<string, unknown> {
	if (!isRecord(value)) {
		throw new Error(`${label} must be an object.`);
	}

	return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isJsonValue(value: unknown): value is JsonValue {
	if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
		return true;
	}

	if (Array.isArray(value)) {
		return value.every(isJsonValue);
	}

	return isRecord(value) && Object.values(value).every(isJsonValue);
}

function stringOrDefault(value: unknown, fallback: string) {
	return typeof value === "string" ? value : fallback;
}

function optionalString(value: unknown) {
	return typeof value === "string" ? value : null;
}

function assetFileNameFromPath(packagePath: string) {
	return packagePath.split("/").pop() || "asset";
}

function getExtension(fileName: string) {
	const extension = fileName.split(".").pop()?.trim().toLowerCase();
	return extension && extension !== fileName.toLowerCase() ? extension : "";
}

function getPackageJsonFiles(fileNames: string[]) {
	return [...getRequiredPackageFiles(), ...(fileNames.includes(EDITOR_PACKAGE_FILE) ? [EDITOR_PACKAGE_FILE] : [])];
}

function finiteNumberOrZero(value: number) {
	return Number.isFinite(value) ? value : 0;
}
