import type { Edge, Node } from "@xyflow/react";
import type { EditorEdgeStyle } from "@/data/editor/flow-canvas";
import { isEditorEdgeStyle } from "@/data/editor/flow-canvas";
import { getNodeDefinition, getNodePorts, getRuntimeDataOutputs } from "@/data/nodes/registry";
import { targetRuntimes } from "@/data/project/runtimes";
import { variableTypes } from "@/data/project/variables";
import type {
	ActionType,
	AssetKind,
	DefaultVariable,
	EditorAsset,
	EditorComment,
	JsonValue,
	ProjectSettings,
	RiskLevel,
	ScriptNodeData,
	SecretDeclaration,
} from "@/lib/types";
import { isSelfConnection, withEdgeExecutionOrder } from "@/utils/editor-graph";
import { DEFAULT_SCRIPT_VERSION } from "@/utils/script-update";
import { type EditorProject, editorProjectSchemaVersion, type ProjectSummary } from "./model";

export type StoredProjectRecord = {
	assetCount: number;
	assets: StoredAssetMetadata[];
	comments: EditorComment[];
	createdAt: string;
	defaultVariables: DefaultVariable[];
	edgeStyle: EditorEdgeStyle;
	edges: StoredEdge[];
	id: string;
	nodes: StoredScriptNode[];
	revision: number;
	schemaVersion: number;
	secretDeclarations: SecretDeclaration[];
	settings: ProjectSettings;
	updatedAt: string;
};

export type StoredAssetMetadata = {
	createdAt: string;
	fileLastModified: number;
	fileName: string;
	fileType: string;
	id: string;
	kind: AssetKind;
	mediaType: string;
	name: string;
	packagePath: string;
	size: number;
};

export type StoredProjectAsset = {
	assetId: string;
	blob: Blob;
	fingerprint?: string;
	key: string;
	projectId: string;
};

type StoredScriptNode = {
	actionType: ActionType;
	config: Record<string, JsonValue>;
	id: string;
	position: { x: number; y: number };
};

type StoredEdge = {
	executionOrder: number;
	id: string;
	source: string;
	sourceHandle: string;
	target: string;
	targetHandle: string;
};

export function toStoredProject(project: EditorProject): StoredProjectRecord {
	return {
		assetCount: project.assets.length,
		assets: project.assets.map(toStoredAssetMetadata),
		comments: project.comments.map(cloneEditorComment),
		createdAt: project.identity.createdAt,
		defaultVariables: structuredClone(project.defaultVariables),
		edgeStyle: project.edgeStyle,
		edges: project.edges.map(toStoredEdge),
		id: project.identity.id,
		nodes: project.nodes.map(toStoredNode),
		revision: project.revision,
		schemaVersion: editorProjectSchemaVersion,
		secretDeclarations: structuredClone(project.secretDeclarations),
		settings: structuredClone(project.settings),
		updatedAt: project.updatedAt,
	};
}

export function toStoredProjectAsset(projectId: string, asset: EditorAsset): StoredProjectAsset {
	return {
		assetId: asset.id,
		blob: asset.file,
		fingerprint: assetStorageFingerprint(asset),
		key: projectAssetKey(projectId, asset.id),
		projectId,
	};
}

export function assetStorageFingerprint(asset: EditorAsset) {
	return JSON.stringify(toStoredAssetMetadata(asset));
}

export function projectAssetKey(projectId: string, assetId: string) {
	return `${projectId}\u0000${assetId}`;
}

export function hydrateProject(recordValue: unknown, assetValues: unknown[]): EditorProject {
	const record = requireStoredProjectRecord(recordValue);
	const assetsById = new Map(
		assetValues.map((value) => {
			const asset = requireStoredProjectAsset(value, record.id);
			return [asset.assetId, asset] as const;
		}),
	);

	const assets = record.assets.map((metadata) => {
		const storedAsset = assetsById.get(metadata.id);
		if (!storedAsset) {
			throw new Error(`Project ${record.id} is missing stored asset ${metadata.id}.`);
		}
		if (storedAsset.blob.size !== metadata.size) {
			throw new Error(`Project ${record.id} asset ${metadata.id} has an invalid stored size.`);
		}

		return {
			...metadata,
			file: new File([storedAsset.blob], metadata.fileName, {
				lastModified: metadata.fileLastModified,
				type: metadata.fileType || metadata.mediaType,
			}),
		};
	});

	return {
		assets,
		comments: record.comments,
		defaultVariables: record.defaultVariables,
		edgeStyle: record.edgeStyle,
		edges: record.edges.map(fromStoredEdge),
		identity: { id: record.id, createdAt: record.createdAt },
		nodes: record.nodes.map(fromStoredNode),
		revision: record.revision,
		schemaVersion: editorProjectSchemaVersion,
		secretDeclarations: record.secretDeclarations,
		settings: record.settings,
		updatedAt: record.updatedAt,
	};
}

export function toProjectSummary(recordValue: unknown): ProjectSummary {
	const record = requireStoredProjectRecord(recordValue);
	return {
		assetCount: record.assets.length,
		createdAt: record.createdAt,
		edgeCount: record.edges.length,
		id: record.id,
		name: record.settings.name,
		nodeCount: record.nodes.length,
		revision: record.revision,
		targetRuntime: record.settings.targetRuntime,
		updatedAt: record.updatedAt,
	};
}

export function projectContentSignature(project: EditorProject) {
	const stored = toStoredProject(project);
	return JSON.stringify({
		assets: stored.assets,
		comments: stored.comments,
		defaultVariables: stored.defaultVariables,
		edgeStyle: stored.edgeStyle,
		edges: stored.edges,
		nodes: stored.nodes,
		secretDeclarations: stored.secretDeclarations,
		settings: stored.settings,
	});
}

function toStoredNode(node: Node<ScriptNodeData>): StoredScriptNode {
	return {
		actionType: node.data.actionType,
		config: structuredClone(node.data.config),
		id: node.id,
		position: finitePosition(node.position),
	};
}

function fromStoredNode(node: StoredScriptNode): Node<ScriptNodeData> {
	const definition = getNodeDefinition(node.actionType);
	if (!definition) {
		throw new Error(`Stored project uses unsupported node action type ${node.actionType}.`);
	}
	const ports = getNodePorts(node.actionType, node.config);
	return {
		id: node.id,
		position: finitePosition(node.position),
		type: "scriptNode",
		data: {
			actionType: node.actionType,
			config: structuredClone(node.config),
			inputs: ports.inputs,
			kind: definition.kind,
			label:
				definition.kind === "trigger" && !definition.label.endsWith("Trigger")
					? `${definition.label} Trigger`
					: definition.label,
			outputs: ports.outputs,
			risk: definition.risk as RiskLevel,
			runtimeOutputs: getRuntimeDataOutputs(node.actionType),
		},
	};
}

function toStoredEdge(edge: Edge): StoredEdge {
	const executionOrder = edge.data?.executionOrder;
	if (!Number.isSafeInteger(executionOrder) || (executionOrder as number) < 0) {
		throw new Error(`Edge ${edge.id} is missing a valid execution order.`);
	}
	if (!edge.sourceHandle || !edge.targetHandle || isSelfConnection(edge)) {
		throw new Error(`Edge ${edge.id} is not a valid project connection.`);
	}

	return {
		executionOrder: executionOrder as number,
		id: edge.id,
		source: edge.source,
		sourceHandle: edge.sourceHandle,
		target: edge.target,
		targetHandle: edge.targetHandle,
	};
}

function fromStoredEdge(edge: StoredEdge): Edge {
	return withEdgeExecutionOrder(
		{
			id: edge.id,
			source: edge.source,
			sourceHandle: edge.sourceHandle,
			target: edge.target,
			targetHandle: edge.targetHandle,
		},
		edge.executionOrder,
	);
}

function toStoredAssetMetadata(asset: EditorAsset): StoredAssetMetadata {
	return {
		createdAt: asset.createdAt,
		fileLastModified: asset.file.lastModified,
		fileName: asset.file.name,
		fileType: asset.file.type,
		id: asset.id,
		kind: asset.kind,
		mediaType: asset.mediaType,
		name: asset.name,
		packagePath: asset.packagePath,
		size: asset.size,
	};
}

function requireStoredProjectRecord(value: unknown): StoredProjectRecord {
	value = migrateStoredProjectRecord(value);
	if (!isRecord(value)) {
		throw new Error("Stored project record is not an object.");
	}
	if (
		value.schemaVersion !== editorProjectSchemaVersion ||
		typeof value.id !== "string" ||
		!isUuid(value.id) ||
		!isIsoDate(value.createdAt) ||
		!isIsoDate(value.updatedAt) ||
		!Number.isSafeInteger(value.revision) ||
		(value.revision as number) < 1 ||
		!isProjectSettings(value.settings) ||
		!Array.isArray(value.nodes) ||
		!Array.isArray(value.edges) ||
		!Array.isArray(value.comments) ||
		!Array.isArray(value.assets) ||
		!Array.isArray(value.secretDeclarations) ||
		!Array.isArray(value.defaultVariables) ||
		!value.nodes.every(isStoredNode) ||
		!value.edges.every(isStoredEdge) ||
		!value.comments.every(isEditorComment) ||
		!value.assets.every(isStoredAssetMetadata) ||
		!value.secretDeclarations.every(isSecretDeclaration) ||
		!value.defaultVariables.every(isDefaultVariable) ||
		typeof value.edgeStyle !== "string" ||
		!isEditorEdgeStyle(value.edgeStyle)
	) {
		throw new Error("Stored project record does not match the current editor project schema.");
	}

	return value as StoredProjectRecord;
}

function requireStoredProjectAsset(value: unknown, projectId: string): StoredProjectAsset {
	if (
		!isRecord(value) ||
		value.projectId !== projectId ||
		typeof value.assetId !== "string" ||
		typeof value.key !== "string" ||
		(value.fingerprint !== undefined && typeof value.fingerprint !== "string") ||
		!(value.blob instanceof Blob)
	) {
		throw new Error(`Project ${projectId} contains an invalid asset record.`);
	}
	return value as StoredProjectAsset;
}

function isProjectSettings(value: unknown): value is ProjectSettings {
	return (
		isRecord(value) &&
		typeof value.name === "string" &&
		typeof value.version === "string" &&
		typeof value.updateUrl === "string" &&
		typeof value.description === "string" &&
		typeof value.author === "string" &&
		typeof value.website === "string" &&
		typeof value.source === "string" &&
		Array.isArray(value.tags) &&
		value.tags.every((tag) => typeof tag === "string") &&
		typeof value.targetRuntime === "string" &&
		targetRuntimes.includes(value.targetRuntime as ProjectSettings["targetRuntime"]) &&
		typeof value.minimumRunnerVersion === "string"
	);
}

function migrateStoredProjectRecord(value: unknown): unknown {
	if (!isRecord(value) || value.schemaVersion !== 1 || !isRecord(value.settings)) {
		return value;
	}

	return {
		...value,
		schemaVersion: editorProjectSchemaVersion,
		settings: {
			...value.settings,
			version: typeof value.settings.version === "string" ? value.settings.version : DEFAULT_SCRIPT_VERSION,
			updateUrl: typeof value.settings.updateUrl === "string" ? value.settings.updateUrl : "",
		},
	};
}

function isStoredNode(value: unknown): value is StoredScriptNode {
	return (
		isRecord(value) &&
		typeof value.id === "string" &&
		typeof value.actionType === "string" &&
		getNodeDefinition(value.actionType as ActionType) !== undefined &&
		isJsonObject(value.config) &&
		isFinitePosition(value.position)
	);
}

function isStoredEdge(value: unknown): value is StoredEdge {
	return (
		isRecord(value) &&
		Number.isSafeInteger(value.executionOrder) &&
		(value.executionOrder as number) >= 0 &&
		["id", "source", "sourceHandle", "target", "targetHandle"].every(
			(key) => typeof value[key] === "string" && value[key].length > 0,
		) &&
		value.source !== value.target
	);
}

function isEditorComment(value: unknown): value is EditorComment {
	return (
		isRecord(value) &&
		typeof value.id === "string" &&
		typeof value.text === "string" &&
		isFinitePosition(value.position) &&
		isRecord(value.size) &&
		typeof value.size.width === "number" &&
		Number.isFinite(value.size.width) &&
		typeof value.size.height === "number" &&
		Number.isFinite(value.size.height) &&
		["amber", "blue", "green", "rose", "violet"].includes(String(value.color)) &&
		typeof value.fontSize === "number" &&
		Number.isFinite(value.fontSize)
	);
}

function isStoredAssetMetadata(value: unknown): value is StoredAssetMetadata {
	return (
		isRecord(value) &&
		["id", "mediaType", "name", "packagePath", "createdAt", "fileName", "fileType"].every(
			(key) => typeof value[key] === "string",
		) &&
		["audio", "image", "text"].includes(String(value.kind)) &&
		typeof value.size === "number" &&
		Number.isSafeInteger(value.size) &&
		value.size >= 0 &&
		typeof value.fileLastModified === "number" &&
		Number.isFinite(value.fileLastModified) &&
		isIsoDate(value.createdAt)
	);
}

function isSecretDeclaration(value: unknown): value is SecretDeclaration {
	return (
		isRecord(value) &&
		typeof value.name === "string" &&
		typeof value.description === "string" &&
		typeof value.required === "boolean" &&
		typeof value.type === "string" &&
		variableTypes.includes(value.type as SecretDeclaration["type"])
	);
}

function cloneEditorComment(comment: EditorComment): EditorComment {
	return {
		id: comment.id,
		text: comment.text,
		position: structuredClone(comment.position),
		size: structuredClone(comment.size),
		color: comment.color,
		fontSize: comment.fontSize,
	};
}

function finitePosition(position: { x: number; y: number }) {
	return {
		x: Number.isFinite(position.x) ? position.x : 0,
		y: Number.isFinite(position.y) ? position.y : 0,
	};
}

function isUuid(value: string) {
	return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function isIsoDate(value: unknown): value is string {
	return typeof value === "string" && Number.isFinite(Date.parse(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isDefaultVariable(value: unknown): value is DefaultVariable {
	return (
		isRecord(value) &&
		typeof value.name === "string" &&
		typeof value.description === "string" &&
		(value.scope === "runtime" || value.scope === "persistent") &&
		typeof value.type === "string" &&
		variableTypes.includes(value.type as DefaultVariable["type"]) &&
		isJsonValue(value.value)
	);
}

function isFinitePosition(value: unknown): value is { x: number; y: number } {
	return (
		isRecord(value) &&
		typeof value.x === "number" &&
		Number.isFinite(value.x) &&
		typeof value.y === "number" &&
		Number.isFinite(value.y)
	);
}

function isJsonObject(value: unknown): value is Record<string, JsonValue> {
	return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isJsonValue(value: unknown): value is JsonValue {
	if (value === null || typeof value === "string" || typeof value === "boolean") return true;
	if (typeof value === "number") return Number.isFinite(value);
	if (Array.isArray(value)) return value.every(isJsonValue);
	return isRecord(value) && Object.values(value).every(isJsonValue);
}
