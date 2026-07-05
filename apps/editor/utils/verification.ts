import type { Edge, Node } from "@xyflow/react";
import { isAllowedPackageFile, validateEditorAssets, validatePackageAssetPaths } from "@/data/project/assets";
import { builtInVariableNames } from "@/data/project/built-in-variables";
import { validateCalculationExpression } from "@/data/project/calculation";
import { normalizeSerialDeviceId } from "@/data/project/serial";
import {
	createNodeOutputVariables,
	normalizeVariableOperation,
	normalizeVariableReferenceName,
	type VariableType,
	validateVariableName,
	validateVariableOperationValue,
	variableTypes,
} from "@/data/project/variables";
import type { EditorAsset, PermissionSummary, ScriptNodeData, TargetRuntime } from "@/lib/types";

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
			const invalidTriggerConfig = getInvalidTriggerConfigs(nodes);

			if (invalidTriggerConfig.length > 0) {
				return {
					outcome: "failed",
					message: invalidTriggerConfig.join(" "),
				};
			}

			return {
				outcome: triggerCount > 0 ? "passed" : "warning",
				message:
					triggerCount > 0
						? `${triggerCount} trigger${triggerCount === 1 ? "" : "s"} available.`
						: "No trigger node found. The script may not start automatically.",
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
			const elevatedPermissionCount = dangerousPermissions.length + highRiskPermissions.length;

			return {
				outcome: elevatedPermissionCount > 0 ? "warning" : "passed",
				message:
					elevatedPermissionCount > 0
						? `${elevatedPermissionCount} elevated permission${elevatedPermissionCount === 1 ? "" : "s"} require review.`
						: "No elevated permissions detected.",
			};
		},
	},
	{
		id: "variables",
		title: "Variables",
		description: "Checking variable writes and read-only runtime references.",
		run: ({ nodes }) => {
			const invalidWrites = getInvalidVariableWrites(nodes);
			const invalidCalculations = getInvalidCalculateConfigs(nodes);
			const invalidFileActions = getInvalidFileActionConfigs(nodes);
			const invalidPixelActions = getInvalidPixelActionConfigs(nodes);
			const invalidFlowConfigs = getInvalidControlFlowConfigs(nodes);
			const invalidRuntimeActionConfigs = getInvalidRuntimeActionConfigs(nodes);
			const errors = [
				...invalidWrites,
				...invalidCalculations,
				...invalidFileActions,
				...invalidPixelActions,
				...invalidFlowConfigs,
				...invalidRuntimeActionConfigs,
			];

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
			const invalidReferences = getInvalidAssetReferences(nodes, assets);

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
			const invalidSerialConfig = getInvalidSerialConfig(nodes);

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
			const manualTriggerCount = context.nodes.filter((node) => node.data.actionType === "trigger.manual").length;
			const invalidVariableWrites = getInvalidVariableWrites(context.nodes);
			const invalidCalculateConfigs = getInvalidCalculateConfigs(context.nodes);
			const invalidFileActionConfigs = getInvalidFileActionConfigs(context.nodes);
			const invalidPixelActionConfigs = getInvalidPixelActionConfigs(context.nodes);
			const invalidFlowConfigs = getInvalidControlFlowConfigs(context.nodes);
			const invalidRuntimeActionConfigs = getInvalidRuntimeActionConfigs(context.nodes);
			const invalidTriggerConfigs = getInvalidTriggerConfigs(context.nodes);
			const invalidAssets = validateEditorAssets(context.assets).errors;
			const invalidAssetReferences = getInvalidAssetReferences(context.nodes, context.assets);
			const invalidSerialConfig = getInvalidSerialConfig(context.nodes);
			const ready =
				context.scriptName.trim() &&
				context.nodes.length > 0 &&
				invalidEdges.length === 0 &&
				manualTriggerCount <= 1 &&
				invalidVariableWrites.length === 0 &&
				invalidCalculateConfigs.length === 0 &&
				invalidFileActionConfigs.length === 0 &&
				invalidPixelActionConfigs.length === 0 &&
				invalidFlowConfigs.length === 0 &&
				invalidRuntimeActionConfigs.length === 0 &&
				invalidTriggerConfigs.length === 0 &&
				invalidAssets.length === 0 &&
				invalidAssetReferences.length === 0 &&
				invalidSerialConfig.length === 0;

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
				outcome: editorMetadata && nodes && invalidNodes.length === 0 ? "passed" : "failed",
				message:
					editorMetadata && nodes && invalidNodes.length === 0
						? `${nodes.length} editor node position${nodes.length === 1 ? "" : "s"} validated.`
						: "Editor metadata must define a nodes array with finite x/y positions.",
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
			const operation = normalizeVariableOperation(configString(node, "operation"));
			const valueType = configString(node, "valueType");
			const normalizedType = normalizeVariableType(valueType);
			const errors: string[] = [];

			if (normalizedName && readOnlyNames.has(normalizedName)) {
				errors.push(`${node.id} tries to change read-only variable "${normalizedName}".`);
			} else {
				const nameError = validateVariableName(name);
				if (nameError) {
					errors.push(`${node.id} has invalid variable name: ${nameError}`);
				}
			}

			if (!normalizedType) {
				errors.push(`${node.id} has invalid variable type "${valueType || "missing"}".`);
			} else {
				const valueError = validateVariableOperationValue(
					operation,
					normalizedType,
					configString(node, "value"),
					configString(node, "fieldPath"),
				);
				if (valueError) {
					errors.push(`${node.id} has invalid ${operation} operation for ${normalizedType}: ${valueError}`);
				}
			}

			return errors;
		});
}

function getInvalidTriggerConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes.flatMap((node) => {
		switch (node.data.actionType) {
			case "trigger.websocket": {
				const socketName = configString(node, "socketName").trim();
				const path = configString(node, "path").trim();
				const errors = [
					socketName ? "" : `${node.id} must define a WebSocket socket name.`,
					path ? "" : `${node.id} must define a WebSocket path.`,
					path && !path.startsWith("/") ? `${node.id} WebSocket path must start with "/".` : "",
				];

				return errors.filter(Boolean);
			}
			case "trigger.process_started":
				return [validateRequiredConfig(node, "target", "process start target")].filter(Boolean);
			default:
				return [];
		}
	});
}

function getInvalidCalculateConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes
		.filter((node) => node.data.actionType === "action.calculate")
		.flatMap((node) => {
			const expression = configString(node, "expression");
			const error = validateCalculationExpression(expression);
			return error ? [`${node.id} has invalid calculation expression: ${error}`] : [];
		});
}

function getInvalidControlFlowConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes.flatMap((node) => {
		if (node.data.actionType !== "control.for_each") {
			return [];
		}

		const errors = [
			validateRequiredConfig(node, "items", "for-each items"),
			validateVariableConfig(node, "itemVariable", "item variable"),
			validateVariableConfig(node, "indexVariable", "index variable"),
		];

		return errors.filter(Boolean);
	});
}

function getInvalidRuntimeActionConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes.flatMap((node) => {
		switch (node.data.actionType) {
			case "action.text.format":
				return [validateRequiredConfig(node, "template", "format template")].filter(Boolean);
			case "action.process.status":
				return [validateRequiredConfig(node, "target", "process target")].filter(Boolean);
			case "action.process.kill":
				return [
					validateRequiredConfig(node, "target", "process target"),
					configString(node, "matchMode") === "pid"
						? validateStaticNonNegativeIntegerConfig(node, "target", "process ID")
						: "",
				].filter(Boolean);
			case "action.script.run":
				return [validateRequiredConfig(node, "script", "sub-script name or path")].filter(Boolean);
			case "action.window.focus":
				return [validateRequiredConfig(node, "target", "window focus target")].filter(Boolean);
			case "action.keyboard.type_text":
				return [validateRequiredConfig(node, "text", "text to type")].filter(Boolean);
			case "action.mouse.move": {
				const relative = configString(node, "relative") === "true";
				return [
					relative
						? validateStaticNumberConfig(node, "x", "mouse X offset")
						: validateStaticNonNegativeNumberConfig(node, "x", "mouse X coordinate"),
					relative
						? validateStaticNumberConfig(node, "y", "mouse Y offset")
						: validateStaticNonNegativeNumberConfig(node, "y", "mouse Y coordinate"),
				].filter(Boolean);
			}
			case "action.beep":
				return [
					validateStaticPositiveNumberConfig(node, "frequencyHz", "beep frequency"),
					validateStaticPositiveNumberConfig(node, "durationMs", "beep duration"),
				].filter(Boolean);
			default:
				return [];
		}
	});
}

function getInvalidFileActionConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes.flatMap((node) => {
		switch (node.data.actionType) {
			case "action.file.download":
				return [
					validateRequiredConfig(node, "url", "download URL"),
					validateStaticHttpUrlConfig(node, "url", "download URL"),
					validateRequiredConfig(node, "destinationPath", "destination file path"),
				].filter(Boolean);
			case "action.file.write":
			case "action.file.delete":
				return [validateRequiredConfig(node, "path", "file path")].filter(Boolean);
			case "action.file.copy":
			case "action.file.move":
				return [
					validateRequiredConfig(node, "sourcePath", "source file path"),
					validateRequiredConfig(node, "destinationPath", "destination file path"),
				].filter(Boolean);
			default:
				return [];
		}
	});
}

function getInvalidPixelActionConfigs(nodes: Node<ScriptNodeData>[]) {
	return nodes
		.filter((node) => node.data.actionType === "action.pixel.get")
		.flatMap((node) => [
			validateStaticNonNegativeNumberConfig(node, "x", "screen X coordinate"),
			validateStaticNonNegativeNumberConfig(node, "y", "screen Y coordinate"),
		])
		.filter(Boolean);
}

function validateRequiredConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	return configString(node, key).trim() ? "" : `${node.id} must define ${label}.`;
}

function validateStaticNonNegativeNumberConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value) {
		return `${node.id} must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) && numberValue >= 0 ? "" : `${node.id} ${label} must be a non-negative number.`;
}

function validateStaticNonNegativeIntegerConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value) {
		return `${node.id} must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isInteger(numberValue) && numberValue >= 0 ? "" : `${node.id} ${label} must be a non-negative integer.`;
}

function validateStaticPositiveNumberConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value) {
		return `${node.id} must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) && numberValue > 0 ? "" : `${node.id} ${label} must be greater than zero.`;
}

function validateStaticNumberConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value) {
		return `${node.id} must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) ? "" : `${node.id} ${label} must be a number.`;
}

function validateVariableConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value) {
		return `${node.id} must define ${label}.`;
	}

	const error = validateVariableName(value);
	return error ? `${node.id} has invalid ${label}: ${error}` : "";
}

function validateStaticHttpUrlConfig(node: Node<ScriptNodeData>, key: string, label: string) {
	const value = configString(node, key).trim();
	if (!value || hasTemplateReference(value)) {
		return "";
	}

	try {
		const url = new URL(value);
		return url.protocol === "http:" || url.protocol === "https:"
			? ""
			: `${node.id} ${label} must use http:// or https://.`;
	} catch {
		return `${node.id} ${label} must be a valid URL.`;
	}
}

function getInvalidAssetReferences(nodes: Node<ScriptNodeData>[], assets: EditorAsset[]) {
	const audioAssetPaths = new Set(
		assets.filter((asset) => asset.kind === "audio").map((asset) => asset.packagePath.toLowerCase()),
	);

	return nodes
		.filter((node) => node.data.actionType === "action.sound.play")
		.flatMap((node) => {
			const source = configString(node, "source") === "file_path" ? "file_path" : "asset";
			const errors: string[] = [];

			if (source === "asset") {
				const assetPath = configString(node, "assetPath").trim();
				if (!assetPath) {
					errors.push(`${node.id} must select an audio asset.`);
				} else if (!audioAssetPaths.has(assetPath.toLowerCase())) {
					errors.push(`${node.id} references missing or non-audio asset "${assetPath}".`);
				}
				return errors;
			}

			if (!configString(node, "filePath").trim()) {
				errors.push(`${node.id} must define an audio file path.`);
			}

			return errors;
		});
}

function getInvalidSerialConfig(nodes: Node<ScriptNodeData>[]) {
	const errors: string[] = [];
	const triggerDeviceIds = new Set<string>();
	const duplicateDeviceIds = new Set<string>();

	for (const node of nodes) {
		if (node.data.actionType !== "trigger.serial_input") {
			continue;
		}

		const deviceId = configString(node, "deviceId").trim();
		const normalizedDeviceId = normalizeSerialDeviceId(deviceId);
		if (!deviceId) {
			errors.push(`${node.id} must define a serial device id.`);
			continue;
		}

		if (deviceId !== normalizedDeviceId) {
			errors.push(`${node.id} serial device id must use lowercase letters, numbers, underscores, or hyphens.`);
			continue;
		}

		if (!configString(node, "port").trim()) {
			errors.push(`${node.id} must define a runner serial port such as COM3 or /dev/ttyUSB0.`);
		}

		if (triggerDeviceIds.has(deviceId)) {
			duplicateDeviceIds.add(deviceId);
		}
		triggerDeviceIds.add(deviceId);
	}

	for (const deviceId of duplicateDeviceIds) {
		errors.push(`Serial device id "${deviceId}" is used by more than one Serial Input Trigger.`);
	}

	for (const node of nodes) {
		if (node.data.actionType !== "action.serial.write") {
			continue;
		}

		const deviceId = configString(node, "deviceId").trim();
		if (!deviceId) {
			errors.push(`${node.id} must select a serial device.`);
			continue;
		}

		if (!triggerDeviceIds.has(deviceId)) {
			errors.push(`${node.id} writes to unknown serial device "${deviceId}". Add a Serial Input Trigger for it.`);
		}
	}

	return errors;
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

function normalizeVariableType(value: string): VariableType | null {
	return variableTypes.includes(value as VariableType) ? (value as VariableType) : null;
}

function hasTemplateReference(value: string) {
	return /\{\{\s*[^{}]+\s*\}\}/.test(value);
}
