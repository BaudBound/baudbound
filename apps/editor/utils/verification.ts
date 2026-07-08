import type { Edge, Node } from "@xyflow/react";
import { isEditorEdgeStyle } from "@/data/editor/flow-canvas";
import {
	getTargetRuntimeCompatibilityErrors as getRegistryTargetRuntimeCompatibilityErrors,
	validateNodeConfig,
	validateNodeGraph,
} from "@/data/nodes/registry";
import { isAllowedPackageFile, validateEditorAssets, validatePackageAssetPaths } from "@/data/project/assets";
import { builtInVariableNames } from "@/data/project/built-in-variables";
import { createNodeOutputVariables, normalizeVariableReferenceName } from "@/data/project/variables";
import type { EditorAsset, PermissionSummary, ScriptNodeData, TargetRuntime } from "@/lib/types";
import { validatePackageJsonContracts } from "./package-contract";

export type VerificationOutcome = "passed" | "warning" | "failed";
export type VerificationStatus = "unverified" | "verified" | "warning" | "failed";

export type VerificationCheck = {
	description: string;
	id: string;
	message: string;
	outcome: VerificationOutcome;
	title: string;
};

type VerificationRule<Context> = {
	description: string;
	id: string;
	run: (context: Context) => Pick<VerificationCheck, "message" | "outcome">;
	title: string;
};

type CreateVerificationChecksOptions = {
	assets: EditorAsset[];
	edges: Edge[];
	nodes: Node<ScriptNodeData>[];
	permissions: PermissionSummary[];
	scriptName: string;
	targetRuntime: TargetRuntime;
};

export type PackageVerificationContext = {
	fileNames: string[];
	jsonFiles: Record<string, unknown>;
	parseErrors: Record<string, string>;
};

export type VerificationSummary = {
	failed: number;
	passed: number;
	status: VerificationStatus;
	warnings: number;
};

export function createVerificationChecks(options: CreateVerificationChecksOptions): VerificationCheck[] {
	return editorVerificationRules.map((rule) => {
		const result = rule.run(options);
		return {
			id: rule.id,
			title: rule.title,
			description: rule.description,
			...result,
		};
	});
}

export function createPackageVerificationChecks(context: PackageVerificationContext): VerificationCheck[] {
	return packageVerificationRules.map((rule) => {
		const result = rule.run(context);
		return {
			id: rule.id,
			title: rule.title,
			description: rule.description,
			...result,
		};
	});
}

export function summarizeVerification(checks: VerificationCheck[]): VerificationSummary {
	const failed = checks.filter((check) => check.outcome === "failed").length;
	const warnings = checks.filter((check) => check.outcome === "warning").length;
	const passed = checks.filter((check) => check.outcome === "passed").length;

	return {
		failed,
		warnings,
		passed,
		status: failed > 0 ? "failed" : warnings > 0 ? "warning" : "verified",
	};
}

const editorVerificationRules: VerificationRule<CreateVerificationChecksOptions>[] = [
	{
		id: "metadata",
		title: "Script metadata",
		description: "Checking script identity and target runtime.",
		run: ({ scriptName, targetRuntime }) => ({
			outcome: scriptName.trim() ? "passed" : "failed",
			message: scriptName.trim()
				? `${scriptName.trim()} targets ${targetRuntime}.`
				: "Script name is required before export.",
		}),
	},
	{
		id: "target-runtime",
		title: "Target runtime",
		description: "Checking that nodes are compatible with the selected target runtime.",
		run: ({ nodes, targetRuntime }) => {
			const incompatibleNodes = getTargetRuntimeCompatibilityErrors(nodes, targetRuntime);

			return {
				outcome: incompatibleNodes.length === 0 ? "passed" : "failed",
				message:
					incompatibleNodes.length === 0
						? `${targetRuntime} supports all nodes in this script.`
						: incompatibleNodes.join(" "),
			};
		},
	},
	{
		id: "graph",
		title: "Graph structure",
		description: "Checking that the script contains runnable nodes.",
		run: ({ nodes }) => {
			const manualTriggerCount = nodes.filter((node) => node.data.actionType === "trigger.manual").length;
			const failureReasons = [
				...(nodes.length === 0 ? ["No nodes found."] : []),
				...(manualTriggerCount > 1 ? ["Only one Manual Trigger node is allowed."] : []),
			];

			return {
				outcome: failureReasons.length === 0 ? "passed" : "failed",
				message:
					failureReasons.length === 0
						? `${nodes.length} node${nodes.length === 1 ? "" : "s"} found.`
						: failureReasons.join(" "),
			};
		},
	},
	{
		id: "entry-points",
		title: "Entry points",
		description: "Checking trigger nodes that can start the script.",
		run: ({ nodes }) => {
			const triggerCount = nodes.filter((node) => node.data.kind === "trigger").length;
			const invalidTriggerConfig = getInvalidNodeConfigKeys(nodes.filter((node) => node.data.kind === "trigger"));

			if (invalidTriggerConfig.length > 0) {
				return {
					outcome: "failed",
					message: invalidTriggerConfig.join(" "),
				};
			}

			return {
				outcome: triggerCount > 0 ? "passed" : "failed",
				message:
					triggerCount > 0
						? `${triggerCount} trigger${triggerCount === 1 ? "" : "s"} available.`
						: "No trigger node found. Add at least one trigger before export.",
			};
		},
	},
	{
		id: "connections",
		title: "Connections",
		description: "Checking edge endpoints and port references.",
		run: ({ nodes, edges }) => {
			const invalidEdges = getInvalidEdges(nodes, edges);
			return {
				outcome: invalidEdges.length === 0 ? "passed" : "failed",
				message:
					invalidEdges.length === 0
						? `${edges.length} connection${edges.length === 1 ? "" : "s"} validated.`
						: `${invalidEdges.length} invalid connection${invalidEdges.length === 1 ? "" : "s"} found.`,
			};
		},
	},
	{
		id: "permissions",
		title: "Permissions",
		description: "Checking risk level and approval-sensitive capabilities.",
		run: ({ permissions }) => {
			const dangerousPermissions = permissions.filter((permission) => permission.risk === "dangerous");
			const highRiskPermissions = permissions.filter((permission) => permission.risk === "high");
			const mediumRiskPermissions = permissions.filter((permission) => permission.risk === "medium");
			const elevatedPermissionCount =
				dangerousPermissions.length + highRiskPermissions.length + mediumRiskPermissions.length;

			return {
				outcome: elevatedPermissionCount > 0 ? "warning" : "passed",
				message:
					elevatedPermissionCount > 0
						? `${elevatedPermissionCount} medium-or-higher risk permission${elevatedPermissionCount === 1 ? " requires" : "s require"} review.`
						: "No elevated permissions detected.",
			};
		},
	},
	{
		id: "variables",
		title: "Variables",
		description: "Checking variable writes and read-only runtime references.",
		run: ({ assets, edges, nodes }) => {
			const invalidWrites = getInvalidVariableWrites(nodes);
			const invalidGraphConfigs = getInvalidNodeGraphConfigs(nodes, edges, assets);
			const invalidNodeConfigKeys = getInvalidNodeConfigKeys(nodes);
			const errors = [...invalidWrites, ...invalidGraphConfigs, ...invalidNodeConfigKeys];

			return {
				outcome: errors.length === 0 ? "passed" : "failed",
				message:
					errors.length === 0
						? "Variable writes, calculations, and action configs are valid."
						: `${errors.length} variable, calculation, or action config issue${errors.length === 1 ? "" : "s"}: ${errors.join(" ")}`,
			};
		},
	},
	{
		id: "assets",
		title: "Assets",
		description: "Checking packaged assets and file restrictions.",
		run: ({ assets, nodes }) => {
			const validation = validateEditorAssets(assets);
			const invalidReferences = nodes
				.filter((node) => node.data.actionType === "action.sound.play")
				.flatMap((node) => validateNodeGraph(node, { assets, edges: [], nodes }));

			if (validation.errors.length > 0 || invalidReferences.length > 0) {
				return {
					outcome: "failed",
					message: [...validation.errors, ...invalidReferences].join(" "),
				};
			}

			return {
				outcome: validation.warnings.length > 0 ? "warning" : "passed",
				message:
					validation.warnings.length > 0
						? validation.warnings.join(" ")
						: `${assets.length} package asset${assets.length === 1 ? "" : "s"} validated.`,
			};
		},
	},
	{
		id: "serial",
		title: "Serial devices",
		description: "Checking serial device ids and write targets.",
		run: ({ nodes }) => {
			const invalidSerialConfig = nodes
				.filter(
					(node) => node.data.actionType === "trigger.serial_input" || node.data.actionType === "action.serial.write",
				)
				.flatMap((node) => validateNodeGraph(node, { assets: [], edges: [], nodes }));

			return {
				outcome: invalidSerialConfig.length === 0 ? "passed" : "failed",
				message:
					invalidSerialConfig.length === 0 ? "Serial device configuration is valid." : invalidSerialConfig.join(" "),
			};
		},
	},
	{
		id: "export-readiness",
		title: "Export readiness",
		description: "Checking whether the package can be prepared.",
		run: (context) => {
			const invalidEdges = getInvalidEdges(context.nodes, context.edges);
			const triggerCount = context.nodes.filter((node) => node.data.kind === "trigger").length;
			const manualTriggerCount = context.nodes.filter((node) => node.data.actionType === "trigger.manual").length;
			const invalidVariableWrites = getInvalidVariableWrites(context.nodes);
			const invalidGraphConfigs = getInvalidNodeGraphConfigs(context.nodes, context.edges, context.assets);
			const invalidNodeConfigKeys = getInvalidNodeConfigKeys(context.nodes);
			const invalidAssets = validateEditorAssets(context.assets).errors;
			const invalidTargetRuntime = getTargetRuntimeCompatibilityErrors(context.nodes, context.targetRuntime);
			const ready =
				context.scriptName.trim() &&
				context.nodes.length > 0 &&
				triggerCount > 0 &&
				invalidEdges.length === 0 &&
				manualTriggerCount <= 1 &&
				invalidVariableWrites.length === 0 &&
				invalidGraphConfigs.length === 0 &&
				invalidNodeConfigKeys.length === 0 &&
				invalidAssets.length === 0 &&
				invalidTargetRuntime.length === 0;

			return {
				outcome: ready ? "passed" : "failed",
				message: ready ? "Script is ready for package export." : "Resolve failed verification steps before export.",
			};
		},
	},
];

const packageVerificationRules: VerificationRule<PackageVerificationContext>[] = [
	{
		id: "package-files",
		title: "Package files",
		description: "Checking required and allowed files inside the imported package.",
		run: ({ fileNames }) => {
			const missingFiles = getRequiredPackageFiles().filter((fileName) => !fileNames.includes(fileName));
			const disallowedFiles = fileNames.filter((fileName) => !isAllowedPackageFile(fileName));

			return {
				outcome: missingFiles.length === 0 && disallowedFiles.length === 0 ? "passed" : "failed",
				message:
					[
						missingFiles.length > 0
							? `Missing required file${missingFiles.length === 1 ? "" : "s"}: ${missingFiles.join(", ")}.`
							: "",
						disallowedFiles.length > 0
							? `Package contains disallowed file${disallowedFiles.length === 1 ? "" : "s"}: ${disallowedFiles.join(", ")}.`
							: "",
					]
						.filter(Boolean)
						.join(" ") || "All required package files are present and package contents are allowed.",
			};
		},
	},
	{
		id: "package-assets",
		title: "Package assets",
		description: "Checking asset paths and supported file types.",
		run: ({ fileNames }) => {
			const validation = validatePackageAssetPaths(fileNames);

			if (validation.errors.length > 0) {
				return {
					outcome: "failed",
					message: validation.errors.join(" "),
				};
			}

			return {
				outcome: validation.warnings.length > 0 ? "warning" : "passed",
				message: validation.warnings.length > 0 ? validation.warnings.join(" ") : "Package assets are valid.",
			};
		},
	},
	{
		id: "package-json",
		title: "Package JSON",
		description: "Checking that JSON package files can be parsed.",
		run: ({ parseErrors }) => {
			const failedFiles = Object.keys(parseErrors);

			return {
				outcome: failedFiles.length === 0 ? "passed" : "failed",
				message:
					failedFiles.length === 0
						? "Package JSON files parsed successfully."
						: `Invalid JSON in ${failedFiles.join(", ")}.`,
			};
		},
	},
	{
		id: "package-contract",
		title: "Package contract",
		description: "Checking package schemas and recalculated declarations.",
		run: ({ jsonFiles, parseErrors }) => {
			if (Object.keys(parseErrors).length > 0) {
				return {
					outcome: "failed",
					message: "Package contract checks require valid JSON.",
				};
			}

			const errors = validatePackageJsonContracts(jsonFiles);
			return {
				outcome: errors.length === 0 ? "passed" : "failed",
				message: errors.length === 0 ? "Package contract is internally consistent." : errors.join(" "),
			};
		},
	},
	{
		id: "manifest",
		title: "Manifest",
		description: "Checking required manifest metadata.",
		run: ({ jsonFiles }) => {
			const manifest = asRecord(jsonFiles["manifest.json"]);
			const missingFields = [
				"format_version",
				"script_language_version",
				"id",
				"name",
				"created_with",
				"created_at",
				"minimum_runner_version",
			].filter((field) => !manifest || manifest[field] === undefined || manifest[field] === "");

			return {
				outcome: missingFields.length === 0 ? "passed" : "failed",
				message:
					missingFields.length === 0
						? `Manifest found for ${String(manifest?.name ?? "imported script")}.`
						: `Manifest is missing: ${missingFields.join(", ")}.`,
			};
		},
	},
	{
		id: "editor-metadata",
		title: "Editor metadata",
		description: "Checking optional editor layout metadata.",
		run: ({ jsonFiles }) => {
			if (jsonFiles["editor.json"] === undefined) {
				return {
					outcome: "passed",
					message: "No editor metadata found. Nodes will be laid out automatically on import.",
				};
			}

			const editorMetadata = asRecord(jsonFiles["editor.json"]);
			const nodes = Array.isArray(editorMetadata?.nodes) ? editorMetadata.nodes : null;
			const canvas = asRecord(editorMetadata?.canvas);
			const validCanvas =
				editorMetadata?.canvas === undefined ||
				(!!canvas &&
					(canvas.edge_style === undefined ||
						(typeof canvas.edge_style === "string" && isEditorEdgeStyle(canvas.edge_style))));
			const invalidNodes =
				nodes?.filter((node) => {
					const nodeRecord = asRecord(node);
					const position = asRecord(nodeRecord?.position);
					return (
						typeof nodeRecord?.id !== "string" ||
						typeof position?.x !== "number" ||
						typeof position?.y !== "number" ||
						!Number.isFinite(position.x) ||
						!Number.isFinite(position.y)
					);
				}) ?? [];

			return {
				outcome: editorMetadata && nodes && invalidNodes.length === 0 && validCanvas ? "passed" : "failed",
				message:
					editorMetadata && nodes && invalidNodes.length === 0 && validCanvas
						? `${nodes.length} editor node position${nodes.length === 1 ? "" : "s"} and canvas preferences validated.`
						: "Editor metadata must define finite node positions and a valid canvas edge style when present.",
			};
		},
	},
	{
		id: "capabilities",
		title: "Capabilities",
		description: "Checking target runtime metadata.",
		run: ({ jsonFiles }) => {
			const capabilities = asRecord(jsonFiles["capabilities.json"]);
			const targetRuntime = capabilities?.target_runtime;

			return {
				outcome: typeof targetRuntime === "string" && targetRuntime.trim() ? "passed" : "failed",
				message:
					typeof targetRuntime === "string" && targetRuntime.trim()
						? `Target runtime: ${targetRuntime}.`
						: "Package capabilities must define target_runtime.",
			};
		},
	},
];

export function getRequiredPackageFiles() {
	return ["manifest.json", "program.json", "permissions.json", "capabilities.json"];
}

function asRecord(value: unknown) {
	return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function getInvalidEdges(nodes: Node<ScriptNodeData>[], edges: Edge[]) {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));

	return edges.filter((edge) => {
		const sourceNode = nodesById.get(edge.source);
		const targetNode = nodesById.get(edge.target);

		if (!sourceNode || !targetNode) {
			return true;
		}

		const hasSourceHandle = sourceNode.data.outputs.some((output) => output.id === edge.sourceHandle);
		const hasTargetHandle = targetNode.data.inputs.some((input) => input.id === edge.targetHandle);

		return !hasSourceHandle || !hasTargetHandle;
	});
}

function getTargetRuntimeCompatibilityErrors(nodes: Node<ScriptNodeData>[], targetRuntime: TargetRuntime) {
	return getRegistryTargetRuntimeCompatibilityErrors(
		nodes.map((node) => ({
			actionType: node.data.actionType,
			config: node.data.config,
			id: node.id,
			label: node.data.label,
		})),
		targetRuntime,
	);
}

function getInvalidNodeConfigKeys(nodes: Node<ScriptNodeData>[]) {
	return nodes.flatMap((node) =>
		validateNodeConfig(node.data.actionType, node.data.config).map((error) => `${node.id} ${error}`),
	);
}

function getInvalidNodeGraphConfigs(nodes: Node<ScriptNodeData>[], edges: Edge[], assets: EditorAsset[]) {
	return nodes.flatMap((node) => validateNodeGraph(node, { assets, edges, nodes }));
}

function getInvalidVariableWrites(nodes: Node<ScriptNodeData>[]) {
	const readOnlyNames = new Set([
		...builtInVariableNames,
		...createNodeOutputVariables(nodes).map((variable) => variable.name),
	]);

	return nodes
		.filter((node) => node.data.actionType === "runtime.set_variable")
		.flatMap((node) => {
			const name = configString(node, "name");
			const normalizedName = normalizeVariableReferenceName(name);

			if (normalizedName && readOnlyNames.has(normalizedName)) {
				return [`${node.id} tries to change read-only variable "${normalizedName}".`];
			}

			return [];
		});
}

function configString(node: Node<ScriptNodeData>, key: string) {
	const value = node.data.config[key];
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined || value === null) {
		return "";
	}

	return String(value);
}
