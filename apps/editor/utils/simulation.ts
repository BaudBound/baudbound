import type { Edge, Node } from "@xyflow/react";
import { isConditionRow, isSwitchCaseRow } from "@/data/nodes/definitions/rows";
import type { NodeSimulationApi } from "@/data/nodes/node-definition";
import { fallibleActionTypes, getNodeDefinition } from "@/data/nodes/registry";
import { evaluateCalculationExpression } from "@/data/project/calculation";
import {
	getClearedVariableValue,
	normalizeVariableOperation,
	type VariableType,
	variableTypes,
} from "@/data/project/variables";
import type {
	JsonValue,
	LogEntry,
	ScriptNodeData,
	SimulationOverride,
	SimulationTraceEntry,
	SimulationVariableSnapshot,
} from "@/lib/types";
import type {
	NodeExecutionResult,
	SimulationContext,
	SimulationRun,
	SimulationRunOptions,
	SimulationSideEffect,
	SimulationSideEffectResult,
	SimulationStep,
} from "./simulation-types";

export type {
	SimulationRun,
	SimulationRunOptions,
	SimulationSideEffect,
	SimulationSideEffectResult,
	SimulationStep,
} from "./simulation-types";

const MAX_SIMULATION_STEPS = 180;
const MAX_EDGE_VISITS = 12;
const MAX_SIMULATION_MESSAGE_LENGTH = 4000;
const MAX_VARIABLE_SNAPSHOT_ENTRIES = 600;
const MAX_VARIABLE_SNAPSHOT_STRING_LENGTH = 4000;
const MAX_SIMULATED_DELAY_MS = 30_000;

const nodeSimulationApi: NodeSimulationApi = {
	clampNumber,
	createError,
	createPixelColorOutput: createSimulatedPixelColorOutput,
	executeHttpRequest: executeHttpRequestNode,
	formatValue,
	getConfigString,
	parseJsonValue,
	resolveJsonCompatibleInput,
	resolveTemplate,
	validatePlaySound: validatePlaySoundNode,
};

export async function createSimulationRun({
	assets,
	edges,
	nodes,
	onStep,
	overrides,
	signal,
	stepDelayMs = 0,
	triggerNodeId,
	triggerPayload = {},
}: SimulationRunOptions): Promise<SimulationRun> {
	const trigger = getSimulationTrigger(nodes, triggerNodeId);
	const context: SimulationContext = {
		assetsByPackagePath: new Map(assets.map((asset) => [asset.packagePath.toLowerCase(), asset])),
		edgesBySource: groupEdgesBySource(edges),
		failed: false,
		halted: false,
		nodeOutputs: {},
		nodesById: new Map(nodes.map((node) => [node.id, node])),
		onStep,
		overridesByNodeId: new Map(overrides.map((override) => [override.nodeId, override.outcome])),
		runtimeVariables: {},
		signal,
		stepDelayMs,
		streamedSteps: 0,
		steps: [],
		triggerPayload,
		visitedEdges: new Map(),
	};

	if (!trigger) {
		await pushStep(context, {
			level: "error",
			message: "[Simulation] No trigger node exists, so the script cannot be simulated.",
		});
		return {
			status: "failed",
			steps: context.steps,
		};
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Starting from ${trigger.data.label} (${trigger.id}).`,
	});
	await executeNode(trigger.id, context, 0);
	if (context.signal?.aborted) {
		await pushStep(context, {
			level: "warn",
			message: "[Simulation] Simulation stopped by user.",
		});
		return { status: "failed", steps: context.steps };
	}
	await pushStep(context, {
		level: context.failed ? "error" : "info",
		message: context.failed ? "[Simulation] Simulation finished with errors." : "[Simulation] Simulation completed.",
	});

	return { status: context.failed ? "failed" : "completed", steps: context.steps };
}

function getSimulationTrigger(nodes: Node<ScriptNodeData>[], triggerNodeId: string | undefined) {
	const triggers = sortNodesForExecution(nodes).filter((node) => node.data.kind === "trigger");
	if (triggerNodeId) {
		const selectedTrigger = triggers.find((node) => node.id === triggerNodeId);
		if (selectedTrigger) {
			return selectedTrigger;
		}
	}

	return triggers[0] ?? null;
}

async function executeNode(nodeId: string, context: SimulationContext, depth: number, stopAtNodeId?: string) {
	if (context.halted || context.signal?.aborted) {
		return;
	}

	if (context.steps.length >= MAX_SIMULATION_STEPS) {
		context.halted = true;
		await pushStep(context, {
			level: "error",
			message: `[Simulation] Stopped after ${MAX_SIMULATION_STEPS} steps to prevent an infinite loop.`,
		});
		return;
	}

	if (nodeId === stopAtNodeId) {
		return;
	}

	const node = context.nodesById.get(nodeId);
	if (!node) {
		await pushStep(context, {
			level: "error",
			message: `[Simulation] Missing target node ${nodeId}; branch stopped.`,
		});
		return;
	}

	const override = context.overridesByNodeId.get(node.id);
	const forcedFailed = override === "failed";
	const result = await createNodeOutputData(node, context, forcedFailed);
	if (context.signal?.aborted) {
		return;
	}

	const failed = result.failed;
	const outcome = failed ? "failed" : "success";
	if (Object.keys(result.outputData).length > 0) {
		context.nodeOutputs[node.id] = result.outputData;
	}

	const sideEffects = failed ? [] : createNodeSideEffects(node, context);
	let sideEffectResults: SimulationSideEffectResult[] = [];

	if (node.data.actionType === "action.delay" && !failed) {
		await simulateDelayNode(node, context);
	} else {
		for (const [index, trace] of describeNodeExecution(node, context, failed, override).entries()) {
			const results = await pushStep(context, trace, index === 0 ? sideEffects : []);
			if (index === 0 && results.length > 0) {
				sideEffectResults = results;
			}
		}
	}

	if (node.data.actionType === "action.message_box" && !failed) {
		const selectedButton =
			sideEffectResults.find(
				(sideEffectResult) => sideEffectResult.type === "message_box" && sideEffectResult.nodeId === node.id,
			)?.button ??
			getMessageBoxButtons(node)[0] ??
			"ok";
		context.nodeOutputs[node.id] = {
			...(context.nodeOutputs[node.id] ?? {}),
			button: selectedButton,
		};
		await pushStep(context, {
			level: "info",
			message: `[Simulation] MessageBox (${node.id}) returned "${selectedButton}".`,
		});
	}

	const outputLogs =
		getNodeDefinition(node.data.actionType)?.simulation?.outputLogs?.({
			api: nodeSimulationApi,
			context,
			failed,
			node,
		}) ?? [];

	for (const log of outputLogs) {
		await pushOutputLog(context, log);
	}

	if (node.data.actionType === "runtime.set_variable" && !failed) {
		const name = getConfigString(node, "name").trim();
		if (name) {
			const result = applyVariableOperation(node, context);
			context.runtimeVariables[name] = result.value;
			await pushStep(context, {
				level: "info",
				message: `[Simulation] ${result.message}`,
			});
		}
	}

	if (node.data.actionType === "control.loop") {
		const count = Math.max(0, Math.min(50, Number(resolveTemplate(getConfigString(node, "count"), context)) || 0));
		for (let index = 0; index < count; index += 1) {
			await pushStep(context, {
				level: "info",
				message: `[Simulation] Loop ${node.id} iteration ${index + 1} of ${count}.`,
			});
			await followRepeatedBody(node, "loop", context, depth + 1, node.id);
		}
		await followHandle(node, "done", context, depth + 1);
		return;
	}

	if (node.data.actionType === "control.for_each") {
		const items = getForEachItems(node, context).slice(0, 200);
		const itemVariable = getConfigString(node, "itemVariable").trim();
		const indexVariable = getConfigString(node, "indexVariable").trim();

		for (const [index, item] of items.entries()) {
			context.nodeOutputs[node.id] = { item, index };
			if (itemVariable) {
				context.runtimeVariables[itemVariable] = item;
			}
			if (indexVariable) {
				context.runtimeVariables[indexVariable] = index;
			}

			await pushStep(context, {
				level: "info",
				message: `[Simulation] For Each ${node.id} item ${index + 1} of ${items.length}.`,
			});
			await followRepeatedBody(node, "loop", context, depth + 1, node.id);
		}
		await followHandle(node, "done", context, depth + 1);
		return;
	}

	const handle = await determineOutputHandle(node, context, outcome);
	if (!handle) {
		await pushStep(context, {
			level: "warn",
			message: `[Simulation] ${node.data.label} (${node.id}) did not select an output branch.`,
		});
		return;
	}

	await followHandle(node, handle, context, depth + 1);
}

async function followRepeatedBody(
	node: Node<ScriptNodeData>,
	handle: string,
	context: SimulationContext,
	depth: number,
	stopAtNodeId: string,
) {
	const outerVisitedEdges = context.visitedEdges;
	context.visitedEdges = new Map();

	try {
		await followHandle(node, handle, context, depth, stopAtNodeId);
	} finally {
		context.visitedEdges = outerVisitedEdges;
	}
}

async function followHandle(
	node: Node<ScriptNodeData>,
	handle: string,
	context: SimulationContext,
	depth: number,
	stopAtNodeId?: string,
) {
	if (context.halted || context.signal?.aborted) {
		return;
	}

	const outgoingEdges = (context.edgesBySource.get(node.id) ?? [])
		.filter((edge) => edge.sourceHandle === handle)
		.sort(
			(a, b) => getNodeSortValue(context.nodesById.get(a.target)) - getNodeSortValue(context.nodesById.get(b.target)),
		);

	if (outgoingEdges.length === 0) {
		await pushStep(context, {
			level: "info",
			message: `[Simulation] No connection from ${node.data.label} (${node.id}) output "${handle}". Branch ended.`,
		});
		return;
	}

	for (const edge of outgoingEdges) {
		if (context.halted || context.signal?.aborted) {
			return;
		}

		const visitKey = `${edge.source}:${edge.sourceHandle}->${edge.target}:${edge.targetHandle}`;
		const visitCount = context.visitedEdges.get(visitKey) ?? 0;
		if (visitCount >= MAX_EDGE_VISITS) {
			await pushStep(context, {
				level: "error",
				message: `[Simulation] Connection ${visitKey} was visited ${MAX_EDGE_VISITS} times; branch stopped to prevent a cycle.`,
			});
			continue;
		}

		context.visitedEdges.set(visitKey, visitCount + 1);
		await pushStep(context, {
			level: "debug",
			message: `[Simulation] Following "${handle}" from ${node.id} to ${edge.target}.`,
		});
		await executeNode(edge.target, context, depth + 1, stopAtNodeId);
	}
}

async function determineOutputHandle(
	node: Node<ScriptNodeData>,
	context: SimulationContext,
	outcome: "success" | "failed",
) {
	if (outcome === "failed" && !node.data.outputs.some((output) => output.id === "failed")) {
		await pushStep(context, {
			level: "warn",
			message: `[Simulation] ${node.data.label} (${node.id}) was forced to fail, but it has no failed output. Branch stopped.`,
		});
		return "";
	}

	if (node.data.kind === "trigger") {
		return "out";
	}

	if (node.data.actionType === "control.if") {
		const result = evaluateIfNode(node, context);
		await pushStep(context, {
			level: "info",
			message: `[Simulation] If / Else ${node.id} evaluated to ${result ? "true" : "false"}.`,
		});
		return result ? "true" : "false";
	}

	if (node.data.actionType === "control.switch") {
		return await evaluateSwitchNode(node, context);
	}

	if (fallibleActionTypes.has(node.data.actionType)) {
		return outcome;
	}

	return "out";
}

function evaluateIfNode(node: Node<ScriptNodeData>, context: SimulationContext) {
	const rows = Array.isArray(node.data.config.conditions) ? node.data.config.conditions.filter(isConditionRow) : [];

	if (rows.length === 0) {
		return false;
	}

	return rows.reduce((result, row, index) => {
		const rowResult = compareValues(
			resolveTemplate(row.left, context),
			row.operator,
			resolveTemplate(row.right, context),
		);

		if (index === 0) {
			return rowResult;
		}

		return row.combinator === "or" ? result || rowResult : result && rowResult;
	}, false);
}

async function evaluateSwitchNode(node: Node<ScriptNodeData>, context: SimulationContext) {
	const switchValue = String(resolveTemplate(getConfigString(node, "value"), context));
	const cases = Array.isArray(node.data.config.cases) ? node.data.config.cases.filter(isSwitchCaseRow) : [];
	const matchedCase = cases.find((switchCase) => String(resolveTemplate(switchCase.value, context)) === switchValue);

	if (!matchedCase) {
		await pushStep(context, {
			level: "warn",
			message: `[Simulation] Switch ${node.id} found no case for value "${switchValue}".`,
		});
		return "";
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Switch ${node.id} matched case "${matchedCase.name}" for value "${switchValue}".`,
	});
	return `case-${matchedCase.id}`;
}

function compareValues(left: JsonValue, operator: string, right: JsonValue) {
	const leftText = String(left);
	const rightText = String(right);
	const leftNumber = Number(leftText);
	const rightNumber = Number(rightText);

	switch (operator) {
		case "==":
			return leftText === rightText;
		case "!=":
			return leftText !== rightText;
		case ">":
			return leftNumber > rightNumber;
		case ">=":
			return leftNumber >= rightNumber;
		case "<":
			return leftNumber < rightNumber;
		case "<=":
			return leftNumber <= rightNumber;
		case "contains":
			return leftText.includes(rightText);
		case "starts_with":
			return leftText.startsWith(rightText);
		case "ends_with":
			return leftText.endsWith(rightText);
		case "regex_match":
			return safeRegexMatch(leftText, rightText);
		case "is_empty":
			return leftText.length === 0;
		case "is_null":
			return left === null || leftText === "null";
		default:
			return false;
	}
}

function safeRegexMatch(value: string, pattern: string) {
	try {
		return new RegExp(pattern).test(value);
	} catch {
		return false;
	}
}

async function createNodeOutputData(
	node: Node<ScriptNodeData>,
	context: SimulationContext,
	failed: boolean,
): Promise<NodeExecutionResult> {
	if (failed) {
		return {
			failed: true,
			outputData: {
				error: createError(`Simulated failure for ${node.data.label}.`, "SIMULATED_FAILURE", "simulation", {
					node_id: node.id,
					action_type: node.data.actionType,
				}),
			},
		};
	}

	const customOutput = getNodeDefinition(node.data.actionType)?.simulation?.createOutput;
	if (customOutput) {
		return await customOutput({ api: nodeSimulationApi, context, forcedFailed: failed, node });
	}

	switch (node.data.actionType) {
		case "trigger.file_watch":
			return {
				failed: false,
				outputData: {
					path: context.triggerPayload.path || resolveTemplate(getConfigString(node, "path"), context),
					event: context.triggerPayload.event || "modified",
				},
			};
		case "trigger.webhook": {
			const body = context.triggerPayload.body || '{"event":"simulation"}';
			const json = parseJsonValue(body);
			const headers = normalizePayloadRecord(context.triggerPayload.headers, { "content-type": "application/json" });
			const query = normalizePayloadRecord(context.triggerPayload.query);
			return {
				failed: false,
				outputData: {
					method: context.triggerPayload.method || getConfigString(node, "method") || "POST",
					path: context.triggerPayload.path || `/events/${getConfigString(node, "hookName") || "name"}`,
					headers,
					query,
					body,
					json: json ?? {},
				},
			};
		}
		case "trigger.websocket": {
			const message = context.triggerPayload.message || '{"event":"simulation"}';
			const json = parseJsonValue(message);
			const headers = normalizePayloadRecord(context.triggerPayload.headers);
			const query = normalizePayloadRecord(context.triggerPayload.query);
			return {
				failed: false,
				outputData: {
					path: context.triggerPayload.path || getConfigString(node, "path") || "/events/socketname",
					connection_id: context.triggerPayload.connectionId || "simulated-connection",
					headers,
					query,
					message,
					json: json ?? {},
					remote_address: context.triggerPayload.remoteAddress || "127.0.0.1",
				},
			};
		}
		case "trigger.hotkey":
			return { failed: false, outputData: { key: context.triggerPayload.key || getConfigString(node, "key") } };
		case "trigger.serial_input": {
			const data = context.triggerPayload.data || "simulation serial input";
			return {
				failed: false,
				outputData: {
					device_id: getConfigString(node, "deviceId") || node.id,
					data,
					bytes: new TextEncoder().encode(data).length,
					timestamp: new Date().toISOString(),
				},
			};
		}
		case "trigger.startup":
			return {
				failed: false,
				outputData: {
					timestamp: new Date().toISOString(),
					reason: context.triggerPayload.reason || "runner_startup",
				},
			};
		case "trigger.process_started":
			return {
				failed: false,
				outputData: {
					process_name: context.triggerPayload.processName || getConfigString(node, "target") || "app.exe",
					process_id: Number(context.triggerPayload.processId) || 4244,
					executable_path: context.triggerPayload.executablePath || "",
					window_title: context.triggerPayload.windowTitle || "",
					timestamp: new Date().toISOString(),
				},
			};
		case "action.calculate":
			return executeCalculateNode(node, context);
		case "action.text.format":
			return {
				failed: false,
				outputData: {
					text: String(resolveTemplate(getConfigString(node, "template"), context)),
				},
			};
		case "action.http":
			return executeHttpRequestNode(node, context);
		case "action.message_box":
			return { failed: false, outputData: {} };
		case "action.pixel.get":
			return {
				failed: false,
				outputData: createSimulatedPixelColorOutput(
					Number(resolveTemplate(getConfigString(node, "x"), context)) || 0,
					Number(resolveTemplate(getConfigString(node, "y"), context)) || 0,
				),
			};
		case "action.file.download":
			return {
				failed: false,
				outputData: {
					url: String(resolveTemplate(getConfigString(node, "url"), context)),
					path: String(resolveTemplate(getConfigString(node, "destinationPath"), context)),
				},
			};
		case "action.file.delete":
			return {
				failed: false,
				outputData: {
					path: String(resolveTemplate(getConfigString(node, "path"), context)),
				},
			};
		case "action.file.copy":
			return {
				failed: false,
				outputData: {
					source_path: String(resolveTemplate(getConfigString(node, "sourcePath"), context)),
					destination_path: String(resolveTemplate(getConfigString(node, "destinationPath"), context)),
				},
			};
		case "action.file.move":
			return {
				failed: false,
				outputData: {
					source_path: String(resolveTemplate(getConfigString(node, "sourcePath"), context)),
					destination_path: String(resolveTemplate(getConfigString(node, "destinationPath"), context)),
				},
			};
		case "action.process.run":
			return { failed: false, outputData: { process_id: 4242 } };
		case "action.process.status":
			return {
				failed: false,
				outputData: {
					running: true,
					state: "running",
					process_id: 4242,
					process_name: String(resolveTemplate(getConfigString(node, "target"), context)) || "app.exe",
				},
			};
		case "action.process.kill":
			return {
				failed: false,
				outputData: {
					process_id: getSimulatedProcessId(node, context),
					process_name:
						getConfigString(node, "matchMode") === "pid"
							? `pid:${getSimulatedProcessId(node, context)}`
							: String(resolveTemplate(getConfigString(node, "target"), context)) || "app.exe",
				},
			};
		case "action.script.run":
			return {
				failed: false,
				outputData: {
					status: "completed",
					exit_code: 0,
				},
			};
		case "action.application.open":
			return {
				failed: false,
				outputData: { application_id: getConfigString(node, "application") || "application", process_id: 4243 },
			};
		case "action.window.active":
			return {
				failed: false,
				outputData: {
					title: "Simulated Active Window",
					process_name: "app.exe",
					process_id: 4245,
					executable_path: "C:\\Program Files\\App\\app.exe",
				},
			};
		case "action.sound.play":
			return validatePlaySoundNode(node, context);
		case "action.serial.write":
			return { failed: false, outputData: {} };
		case "action.shell":
			return { failed: false, outputData: { exit_code: 0, stdout: "Simulated shell output", stderr: "" } };
		default:
			return { failed: false, outputData: {} };
	}
}

function executeCalculateNode(node: Node<ScriptNodeData>, context: SimulationContext): NodeExecutionResult {
	const expression = String(resolveTemplate(getConfigString(node, "expression"), context));
	const result = evaluateCalculationExpression(expression);

	if (!result.ok) {
		return {
			failed: true,
			outputData: {
				error: {
					message: result.message,
					code: "CALCULATION_FAILED",
					type: "validation",
					retryable: false,
					details: {
						expression,
					},
				},
			},
		};
	}

	return { failed: false, outputData: { result: result.value } };
}

function getForEachItems(node: Node<ScriptNodeData>, context: SimulationContext): JsonValue[] {
	const value = resolveJsonCompatibleInput(getConfigString(node, "items"), context);
	if (Array.isArray(value)) {
		return value;
	}

	if (value && typeof value === "object") {
		return Object.values(value);
	}

	if (typeof value === "string" && value.trim()) {
		return value
			.split(/\r?\n|,/)
			.map((item) => item.trim())
			.filter(Boolean);
	}

	return [];
}

function createSimulatedPixelColorOutput(x: number, y: number): Record<string, JsonValue> {
	const normalizedX = Math.trunc(Math.max(0, x));
	const normalizedY = Math.trunc(Math.max(0, y));
	const red = (normalizedX * 37 + normalizedY * 17) % 256;
	const green = (normalizedX * 13 + normalizedY * 57) % 256;
	const blue = (normalizedX * 91 + normalizedY * 23) % 256;
	const alpha = 255;
	const integer = red * 65536 + green * 256 + blue;
	const hex = `#${toHexChannel(red)}${toHexChannel(green)}${toHexChannel(blue)}`;

	return {
		hex,
		rgb: { r: red, g: green, b: blue },
		rgba: { r: red, g: green, b: blue, a: alpha },
		red,
		green,
		blue,
		alpha,
		integer,
	};
}

function toHexChannel(value: number) {
	return value.toString(16).padStart(2, "0");
}

function validatePlaySoundNode(node: Node<ScriptNodeData>, context: SimulationContext): NodeExecutionResult {
	const source = getConfigString(node, "source") === "file_path" ? "file_path" : "asset";
	if (source === "file_path") {
		return { failed: false, outputData: {} };
	}

	const assetPath = String(resolveTemplate(getConfigString(node, "assetPath"), context)).trim();
	const asset = context.assetsByPackagePath.get(assetPath.toLowerCase());

	if (!asset || asset.kind !== "audio") {
		return {
			failed: true,
			outputData: {
				error: createPlaySoundErrorObject("Audio asset was not found in the current asset library.", {
					source,
					asset_path: assetPath,
				}),
			},
		};
	}

	return { failed: false, outputData: {} };
}

async function simulateDelayNode(node: Node<ScriptNodeData>, context: SimulationContext) {
	const delay = getDelaySimulationDuration(node, context);
	await pushStep(context, {
		level: "info",
		message: `[Simulation] Delay (${node.id}) waiting ${delay.label}${delay.capped ? `; simulator wait capped to ${formatDurationMs(delay.waitMs)}` : ""}.`,
	});

	await sleepSimulationStep(delay.waitMs, context.signal);
	if (context.signal?.aborted) {
		return;
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Delay (${node.id}) completed after ${delay.capped ? formatDurationMs(delay.waitMs) : delay.label}.`,
	});
}

function getDelaySimulationDuration(node: Node<ScriptNodeData>, context: SimulationContext) {
	const amountValue = Number(resolveTemplate(getConfigString(node, "amount"), context));
	const safeAmount = Number.isFinite(amountValue) && amountValue > 0 ? amountValue : 0;
	const configuredMs = Math.round(safeAmount * getDelayUnitMultiplier(getConfigString(node, "unit")));
	const waitMs = Math.min(configuredMs, MAX_SIMULATED_DELAY_MS);

	return {
		capped: configuredMs > waitMs,
		label: `${formatValue(resolveTemplate(getConfigString(node, "amount"), context))} ${normalizeDelayUnit(getConfigString(node, "unit"))}`,
		waitMs,
	};
}

function getDelayUnitMultiplier(unit: string) {
	if (unit === "days") {
		return 24 * 60 * 60 * 1000;
	}

	if (unit === "hours") {
		return 60 * 60 * 1000;
	}

	if (unit === "minutes") {
		return 60 * 1000;
	}

	return 1000;
}

function normalizeDelayUnit(unit: string) {
	return unit === "days" || unit === "hours" || unit === "minutes" ? unit : "seconds";
}

function formatDurationMs(milliseconds: number) {
	if (milliseconds < 1000) {
		return `${milliseconds}ms`;
	}

	const totalSeconds = Math.round(milliseconds / 1000);
	const hours = Math.floor(totalSeconds / 3600);
	const minutes = Math.floor((totalSeconds % 3600) / 60);
	const seconds = totalSeconds % 60;

	if (hours > 0) {
		return `${hours}h ${minutes}m ${seconds}s`;
	}

	if (minutes > 0) {
		return `${minutes}m ${seconds}s`;
	}

	return `${seconds}s`;
}

function createPlaySoundErrorObject(message: string, details: Record<string, JsonValue>): Record<string, JsonValue> {
	return createError(message, "AUDIO_PLAYBACK_FAILED", "audio", details);
}

function createError(
	message: string,
	code: string,
	type: string,
	details: Record<string, JsonValue> = {},
	retryable = true,
): Record<string, JsonValue> {
	return {
		message,
		code,
		type,
		retryable,
		details,
	};
}

async function executeHttpRequestNode(
	node: Node<ScriptNodeData>,
	context: SimulationContext,
): Promise<NodeExecutionResult> {
	const method = normalizeHttpMethod(getConfigString(node, "method"));
	const url = String(resolveTemplate(getConfigString(node, "url"), context)).trim();
	const body = String(resolveTemplate(getConfigString(node, "body"), context));
	const timeoutSeconds = clampNumber(Number(getConfigString(node, "timeoutSeconds")) || 30, 1, 300);
	const headers = createHttpHeaders(node, context);
	const startedAt = performance.now();
	const abortController = new AbortController();
	const forwardAbort = () => abortController.abort(context.signal?.reason);
	const timeoutId = window.setTimeout(() => abortController.abort("timeout"), timeoutSeconds * 1000);

	if (context.signal?.aborted) {
		window.clearTimeout(timeoutId);
		return { failed: false, outputData: {} };
	}

	context.signal?.addEventListener("abort", forwardAbort, { once: true });

	try {
		const response = await window.fetch(url, {
			method,
			headers,
			body: method === "GET" || method === "HEAD" || body.length === 0 ? undefined : body,
			signal: abortController.signal,
		});
		const responseBody = await response.text();
		const responseHeaders = getResponseHeaders(response.headers);
		const json = parseJsonValue(responseBody);
		const outputData: Record<string, JsonValue> = {
			status_code: response.status,
			status_text: response.statusText,
			headers: responseHeaders,
			body: responseBody,
			duration_ms: Math.round(performance.now() - startedAt),
		};

		if (json !== undefined) {
			outputData.json = json;
		}

		return { failed: false, outputData };
	} catch (error) {
		if (context.signal?.aborted) {
			return { failed: false, outputData: {} };
		}

		return {
			failed: true,
			outputData: {
				error: createHttpErrorObject(error, url, method, Math.round(performance.now() - startedAt)),
			},
		};
	} finally {
		window.clearTimeout(timeoutId);
		context.signal?.removeEventListener("abort", forwardAbort);
	}
}

function createHttpHeaders(node: Node<ScriptNodeData>, context: SimulationContext) {
	const headers = new Headers();
	const configHeaders = node.data.config.headers;

	if (Array.isArray(configHeaders)) {
		for (const header of configHeaders) {
			if (!isHeaderLike(header)) {
				continue;
			}

			const name = String(resolveTemplate(header.name, context)).trim();
			const value = String(resolveTemplate(header.value, context));
			if (name && name.toLowerCase() !== "user-agent") {
				try {
					headers.set(name, value);
				} catch {
					// Browsers reject invalid or forbidden request headers. The runner can validate these more strictly.
				}
			}
		}
	}

	return headers;
}

function getResponseHeaders(headers: Headers): Record<string, JsonValue> {
	const responseHeaders: Record<string, JsonValue> = {};
	headers.forEach((value, key) => {
		responseHeaders[key] = value;
	});

	return responseHeaders;
}

function createHttpErrorObject(
	error: unknown,
	url: string,
	method: string,
	durationMs: number,
): Record<string, JsonValue> {
	const isTimeout = error instanceof DOMException && error.name === "AbortError";
	const isBrowserFetchFailure = error instanceof TypeError;
	const message = isTimeout
		? `HTTP request timed out after ${durationMs}ms.`
		: isBrowserFetchFailure
			? "Browser fetch failed. The target may be blocking cross-origin browser requests with CORS, the page may be blocking mixed HTTP content, or the network request failed."
			: error instanceof Error
				? error.message
				: "HTTP request failed.";

	return {
		message,
		code: isTimeout ? "HTTP_TIMEOUT" : isBrowserFetchFailure ? "BROWSER_FETCH_BLOCKED" : "HTTP_REQUEST_FAILED",
		type: "http",
		retryable: true,
		details: {
			method,
			url,
			duration_ms: durationMs,
			client_side: true,
			possible_causes: ["cors", "mixed_content", "network"],
		},
	};
}

function getHttpExecutionDetail(node: Node<ScriptNodeData>, context: SimulationContext) {
	const output = context.nodeOutputs[node.id];
	const method = getConfigString(node, "method");
	const url = formatValue(resolveTemplate(getConfigString(node, "url"), context));

	if (output?.error && typeof output.error === "object" && !Array.isArray(output.error)) {
		return `${method} ${url} failed: ${String(output.error.message ?? "request failed")}.`;
	}

	if (typeof output?.status_code === "number") {
		return `${method} ${url} returned ${output.status_code} ${String(output.status_text ?? "")} in ${String(output.duration_ms ?? "?")}ms.`;
	}

	return `${method} ${url} was skipped because the simulation stopped.`;
}

function getPixelColorExecutionDetail(node: Node<ScriptNodeData>, context: SimulationContext) {
	const output = context.nodeOutputs[node.id];
	const x = formatValue(resolveTemplate(getConfigString(node, "x"), context));
	const y = formatValue(resolveTemplate(getConfigString(node, "y"), context));

	return `Captured simulated screen pixel at x=${x}, y=${y} as ${String(output?.hex ?? "unknown")}.`;
}

function createNodeSideEffects(node: Node<ScriptNodeData>, context: SimulationContext): SimulationSideEffect[] {
	const customSideEffects = getNodeDefinition(node.data.actionType)?.simulation?.sideEffects;
	if (customSideEffects) {
		return customSideEffects({ api: nodeSimulationApi, context, node });
	}

	if (node.data.actionType === "action.notification") {
		return [
			{
				type: "notification_toast",
				nodeId: node.id,
				title: String(resolveTemplate(getConfigString(node, "title"), context)),
				message: String(resolveTemplate(getConfigString(node, "message"), context)),
			},
		];
	}

	if (node.data.actionType === "action.message_box") {
		return [
			{
				type: "message_box",
				nodeId: node.id,
				title: String(resolveTemplate(getConfigString(node, "title"), context)),
				message: String(resolveTemplate(getConfigString(node, "message"), context)),
				variant: normalizeMessageBoxVariant(getConfigString(node, "type")),
				buttons: getMessageBoxButtons(node),
			},
		];
	}

	if (node.data.actionType === "action.sound.play") {
		if (getConfigString(node, "source") === "file_path") {
			return [];
		}

		const assetPath = String(resolveTemplate(getConfigString(node, "assetPath"), context)).trim();
		return assetPath ? [{ type: "play_audio_asset", nodeId: node.id, assetPath }] : [];
	}

	if (node.data.actionType === "action.beep") {
		return [
			{
				type: "system_beep",
				nodeId: node.id,
				frequencyHz: clampNumber(
					Number(resolveTemplate(getConfigString(node, "frequencyHz"), context)) || 800,
					20,
					20000,
				),
				durationMs: clampNumber(Number(resolveTemplate(getConfigString(node, "durationMs"), context)) || 200, 10, 5000),
			},
		];
	}

	return [];
}

function describeNodeExecution(
	node: Node<ScriptNodeData>,
	context: SimulationContext,
	failed: boolean,
	override: SimulationOverride["outcome"] | undefined,
): SimulationTraceEntry[] {
	const customDescribe = getNodeDefinition(node.data.actionType)?.simulation?.describe;
	if (customDescribe) {
		return customDescribe({ api: nodeSimulationApi, context, failed, node, override });
	}

	const prefix = `[Simulation] ${node.data.label} (${node.id})`;
	const overrideText = override ? ` Override forced ${override}.` : "";
	const resultText = failed ? "failed" : "succeeded";
	const detail = getExecutionDetail(node, context, failed);

	return [
		{
			level: failed ? "error" : "info",
			message: `${prefix} ${resultText}.${overrideText}${detail ? ` ${detail}` : ""}`,
		},
	];
}

function getExecutionDetail(node: Node<ScriptNodeData>, context: SimulationContext, failed: boolean) {
	if (failed) {
		if (node.data.actionType === "action.http") {
			return getHttpExecutionDetail(node, context);
		}

		return "The failed output will be used when it is connected.";
	}

	switch (node.data.actionType) {
		case "trigger.manual":
			return "Manual trigger fired.";
		case "trigger.schedule":
			return `Schedule fired every ${getConfigString(node, "every")} ${getConfigString(node, "unit")}.`;
		case "trigger.file_watch":
			return getFileWatchExecutionDetail(node, context);
		case "trigger.webhook":
			return `Webhook ${getConfigString(node, "method")} /events/${getConfigString(node, "hookName")} received simulated JSON.`;
		case "trigger.websocket":
			return `WebSocket ${getConfigString(node, "path")} received a simulated message.`;
		case "trigger.hotkey":
			return `Hotkey ${getConfigString(node, "key")} was pressed.`;
		case "trigger.serial_input":
			return `Serial device ${getConfigString(node, "deviceId")} on ${getConfigString(node, "port")} received simulated data.`;
		case "trigger.startup":
			return "Startup event fired.";
		case "trigger.process_started":
			return `Process start detected for ${getConfigString(node, "target")}.`;
		case "runtime.set_variable":
			return `Preparing to ${normalizeVariableOperation(getConfigString(node, "operation")).replaceAll("_", " ")} ${getConfigString(node, "name")}.`;
		case "action.calculate": {
			const output = context.nodeOutputs[node.id];
			return typeof output?.result === "number"
				? `Calculated ${formatValue(resolveTemplate(getConfigString(node, "expression"), context))} = ${output.result}.`
				: `Would calculate ${formatValue(resolveTemplate(getConfigString(node, "expression"), context))}.`;
		}
		case "action.text.format":
			return `Formatted text as "${String(context.nodeOutputs[node.id]?.text ?? "")}".`;
		case "action.log":
			return `Emitted a ${normalizeLogLevel(getConfigString(node, "level"))} runner log to the Output tab.`;
		case "action.delay":
			return `Waited ${formatValue(resolveTemplate(getConfigString(node, "amount"), context))} ${getConfigString(node, "unit")}.`;
		case "action.http":
			return getHttpExecutionDetail(node, context);
		case "action.notification":
			return `Would show notification "${resolveTemplate(getConfigString(node, "title"), context)}" with message "${resolveTemplate(getConfigString(node, "message"), context)}".`;
		case "action.message_box":
			return `Would show ${getConfigString(node, "type")} message box "${resolveTemplate(getConfigString(node, "title"), context)}".`;
		case "action.pixel.get":
			return getPixelColorExecutionDetail(node, context);
		case "action.file.download":
			return `Would download ${formatValue(resolveTemplate(getConfigString(node, "url"), context))} to ${formatValue(resolveTemplate(getConfigString(node, "destinationPath"), context))}${getOverwriteDetail(node)}.`;
		case "action.file.write":
			return `Would ${getConfigString(node, "mode") === "append" ? "append to" : "overwrite"} ${formatValue(resolveTemplate(getConfigString(node, "path"), context))}.`;
		case "action.file.delete":
			return `Would delete ${formatValue(resolveTemplate(getConfigString(node, "path"), context))}.`;
		case "action.file.copy":
			return `Would copy ${formatValue(resolveTemplate(getConfigString(node, "sourcePath"), context))} to ${formatValue(resolveTemplate(getConfigString(node, "destinationPath"), context))}${getOverwriteDetail(node)}.`;
		case "action.file.move":
			return `Would move ${formatValue(resolveTemplate(getConfigString(node, "sourcePath"), context))} to ${formatValue(resolveTemplate(getConfigString(node, "destinationPath"), context))}${getOverwriteDetail(node)}.`;
		case "action.process.run":
			return `Would run ${formatValue(resolveTemplate(getConfigString(node, "executable"), context))} ${formatValue(resolveTemplate(getConfigString(node, "arguments"), context))}.`;
		case "action.process.status":
			return `Would check ${getConfigString(node, "matchMode")} ${formatValue(resolveTemplate(getConfigString(node, "target"), context))}; simulated state is running.`;
		case "action.process.kill":
			return `Would terminate ${getConfigString(node, "matchMode")} ${formatValue(resolveTemplate(getConfigString(node, "target"), context))}.`;
		case "action.script.run":
			return `Would run the manual trigger in sub-script ${formatValue(resolveTemplate(getConfigString(node, "script"), context))}.`;
		case "action.application.open":
			return `Would open application ${formatValue(resolveTemplate(getConfigString(node, "application"), context))}.`;
		case "action.window.active":
			return `Captured active window "${String(context.nodeOutputs[node.id]?.title ?? "unknown")}".`;
		case "action.window.focus":
			return `Would focus window using ${getConfigString(node, "matchMode")} ${formatValue(resolveTemplate(getConfigString(node, "target"), context))}.`;
		case "action.sound.play":
			return getPlaySoundExecutionDetail(node, context);
		case "action.serial.write":
			return `Would write ${formatValue(resolveTemplate(getConfigString(node, "data"), context))} to ${getSerialWriteTarget(node, context)} ${getSerialLineEndingDetail(node)}.`;
		case "action.keyboard":
			return `Would press keys ${getConfigString(node, "key")}.`;
		case "action.keyboard.type_text":
			return `Would type text ${formatValue(resolveTemplate(getConfigString(node, "text"), context))}.`;
		case "action.mouse":
			return `Would ${getConfigString(node, "clickType")} ${getConfigString(node, "button")} mouse button.`;
		case "action.mouse.move":
			return `Would move mouse ${getConfigString(node, "relative") === "true" ? "relatively by" : "to"} x=${formatValue(resolveTemplate(getConfigString(node, "x"), context))}, y=${formatValue(resolveTemplate(getConfigString(node, "y"), context))}.`;
		case "action.beep":
			return `Played simulated beep at ${formatValue(resolveTemplate(getConfigString(node, "frequencyHz"), context))}Hz for ${formatValue(resolveTemplate(getConfigString(node, "durationMs"), context))}ms.`;
		case "action.clipboard":
			return `Would write clipboard value ${formatValue(resolveTemplate(getConfigString(node, "value"), context))}.`;
		case "action.shell":
			return `Would run shell command ${formatValue(resolveTemplate(getConfigString(node, "command"), context))}.`;
		default:
			return "";
	}
}

function getPlaySoundExecutionDetail(node: Node<ScriptNodeData>, context: SimulationContext) {
	const source = getConfigString(node, "source") === "file_path" ? "file_path" : "asset";
	if (source === "file_path") {
		return `Would play audio file ${formatValue(resolveTemplate(getConfigString(node, "filePath"), context))}.`;
	}

	return `Would play packaged audio asset ${formatValue(resolveTemplate(getConfigString(node, "assetPath"), context))}.`;
}

function getFileWatchExecutionDetail(node: Node<ScriptNodeData>, context: SimulationContext) {
	const output = context.nodeOutputs[node.id];
	const event = typeof output?.event === "string" ? output.event : "modified";
	const path =
		typeof output?.path === "string"
			? output.path
			: formatValue(resolveTemplate(getConfigString(node, "path"), context));

	return `File watcher received ${event} event for ${path}.`;
}

function getSerialWriteTarget(node: Node<ScriptNodeData>, context: SimulationContext) {
	const deviceId = getConfigString(node, "deviceId");
	const trigger = [...context.nodesById.values()].find(
		(candidate) =>
			candidate.data.actionType === "trigger.serial_input" && getConfigString(candidate, "deviceId") === deviceId,
	);

	if (!trigger) {
		return `serial device ${deviceId}`;
	}

	const port = getConfigString(trigger, "port");
	return port ? `serial device ${deviceId} on ${port}` : `serial device ${deviceId}`;
}

function getSerialLineEndingDetail(node: Node<ScriptNodeData>) {
	const lineEnding = getConfigString(node, "lineEnding");
	if (lineEnding === "crlf") {
		return "with CRLF line ending";
	}

	if (lineEnding === "lf") {
		return "with LF line ending";
	}

	return "without an added line ending";
}

function getOverwriteDetail(node: Node<ScriptNodeData>) {
	return getConfigString(node, "overwrite") === "true" ? " and overwrite an existing destination" : "";
}

function getMessageBoxButtons(node: Node<ScriptNodeData>) {
	switch (getConfigString(node, "buttons")) {
		case "ok_cancel":
			return ["ok", "cancel"];
		case "yes_no":
			return ["yes", "no"];
		case "yes_no_cancel":
			return ["yes", "no", "cancel"];
		case "retry_cancel":
			return ["retry", "cancel"];
		default:
			return ["ok"];
	}
}

function normalizeMessageBoxVariant(value: string): Extract<SimulationSideEffect, { type: "message_box" }>["variant"] {
	if (value === "warning" || value === "error" || value === "question") {
		return value;
	}

	return "info";
}

function applyVariableOperation(node: Node<ScriptNodeData>, context: SimulationContext) {
	const name = getConfigString(node, "name").trim();
	const type = normalizeVariableType(getConfigString(node, "valueType"));
	const operation = normalizeVariableOperation(getConfigString(node, "operation"));
	const currentValue = context.runtimeVariables[name];

	if (operation === "increment") {
		const amount = Number(resolveTemplate(getConfigString(node, "value"), context));
		const currentNumber = typeof currentValue === "number" ? currentValue : Number(currentValue);
		const value = (Number.isFinite(currentNumber) ? currentNumber : 0) + (Number.isFinite(amount) ? amount : 0);

		return {
			value,
			message: `Incremented runtime variable "${name}" by ${formatValue(amount)} to ${formatValue(value)}.`,
		};
	}

	if (operation === "append_list") {
		const item = resolveJsonCompatibleInput(getConfigString(node, "value"), context);
		const value = [...(Array.isArray(currentValue) ? currentValue : []), item];

		return {
			value,
			message: `Appended ${formatValue(item)} to list variable "${name}".`,
		};
	}

	if (operation === "set_object_field") {
		const fieldPath = getConfigString(node, "fieldPath").trim();
		const fieldValue = resolveJsonCompatibleInput(getConfigString(node, "value"), context);
		const value = setObjectPathValue(currentValue, fieldPath, fieldValue);

		return {
			value,
			message: `Set object field "${name}.${fieldPath}" to ${formatValue(fieldValue)}.`,
		};
	}

	if (operation === "clear") {
		const value = resolveVariableInput(getClearedVariableValue(type), type, context);

		return {
			value,
			message: `Cleared runtime variable "${name}" to ${formatValue(value)}.`,
		};
	}

	const value = resolveVariableInput(getConfigString(node, "value"), type, context);
	return {
		value,
		message: `Set runtime variable "${name}" to ${formatValue(value)}.`,
	};
}

function normalizeVariableType(value: string): VariableType {
	return variableTypes.includes(value as VariableType) ? (value as VariableType) : "string";
}

function resolveVariableInput(value: string, type: VariableType, context: SimulationContext): JsonValue {
	const resolved = resolveTemplate(value, context);
	if (typeof resolved !== "string") {
		return resolved;
	}

	if (type === "number") {
		const numberValue = Number(resolved);
		return Number.isFinite(numberValue) ? numberValue : resolved;
	}

	if (type === "boolean") {
		return resolved.trim() === "true" ? true : resolved.trim() === "false" ? false : resolved;
	}

	if (type === "list" || type === "object" || type === "duration" || type === "datetime" || type === "http_response") {
		return parseJsonValue(resolved) ?? resolved;
	}

	return resolved;
}

function resolveJsonCompatibleInput(value: string, context: SimulationContext): JsonValue {
	const resolved = resolveTemplate(value, context);
	if (typeof resolved !== "string") {
		return resolved;
	}

	const trimmed = resolved.trim();
	if (trimmed === "true") {
		return true;
	}

	if (trimmed === "false") {
		return false;
	}

	if (trimmed === "null") {
		return null;
	}

	if (trimmed && Number.isFinite(Number(trimmed))) {
		return Number(trimmed);
	}

	return parseJsonValue(trimmed) ?? resolved;
}

function setObjectPathValue(currentValue: JsonValue | undefined, path: string, value: JsonValue): JsonValue {
	const root =
		currentValue && typeof currentValue === "object" && !Array.isArray(currentValue) ? cloneJson(currentValue) : {};
	const parts = parseObjectPath(path);
	let cursor: Record<string, JsonValue> | JsonValue[] = root;

	for (let index = 0; index < parts.length; index += 1) {
		const part = parts[index];
		const isLast = index === parts.length - 1;

		if (isLast) {
			setPathContainerValue(cursor, part, value);
			break;
		}

		const nextPart = parts[index + 1];
		const existing = getPathContainerValue(cursor, part);
		const nextValue =
			existing && typeof existing === "object" ? cloneJson(existing) : typeof nextPart === "number" ? [] : {};

		setPathContainerValue(cursor, part, nextValue);
		cursor = nextValue as Record<string, JsonValue> | JsonValue[];
	}

	return root;
}

function getPathContainerValue(container: Record<string, JsonValue> | JsonValue[], key: string | number) {
	return Array.isArray(container) ? container[Number(key)] : container[String(key)];
}

function setPathContainerValue(
	container: Record<string, JsonValue> | JsonValue[],
	key: string | number,
	value: JsonValue,
) {
	if (Array.isArray(container)) {
		container[Number(key)] = value;
		return;
	}

	container[String(key)] = value;
}

function parseObjectPath(path: string): Array<string | number> {
	return [...path.matchAll(/[A-Za-z_][A-Za-z0-9_]*|\[(0|[1-9][0-9]*)\]/g)].map((match) =>
		match[1] === undefined ? match[0] : Number(match[1]),
	);
}

function cloneJson<T extends JsonValue>(value: T): T {
	return JSON.parse(JSON.stringify(value)) as T;
}

function resolveTemplate(value: string, context: SimulationContext): JsonValue {
	const exactReference = value.match(/^\{\{\s*([^}]+?)\s*\}\}$/);
	if (exactReference) {
		return getReferenceValue(exactReference[1].trim(), context);
	}

	return value.replace(/\{\{\s*([^}]+?)\s*\}\}/g, (_match, reference: string) =>
		String(getReferenceValue(reference.trim(), context)),
	);
}

function getReferenceValue(reference: string, context: SimulationContext): JsonValue {
	if (reference in context.runtimeVariables) {
		return context.runtimeVariables[reference];
	}

	const variableReference = getRuntimeVariableReference(reference, context.runtimeVariables);
	if (variableReference) {
		return getPathValue(variableReference.value, variableReference.path) ?? `{{${reference}}}`;
	}

	const nodeId = [...context.nodesById.keys()].find((id) => reference === id || reference.startsWith(`${id}.`));
	if (!nodeId) {
		return `{{${reference}}}`;
	}

	const path = reference.slice(nodeId.length + 1);
	return getPathValue(context.nodeOutputs[nodeId] ?? {}, path) ?? `{{${reference}}}`;
}

function getRuntimeVariableReference(reference: string, variables: Record<string, JsonValue>) {
	const variableName = Object.keys(variables)
		.filter((name) => reference.startsWith(`${name}.`) || reference.startsWith(`${name}[`))
		.sort((a, b) => b.length - a.length)[0];

	if (!variableName) {
		return null;
	}

	const path = reference.startsWith(`${variableName}.`)
		? reference.slice(variableName.length + 1)
		: reference.slice(variableName.length);

	return {
		path,
		value: variables[variableName],
	};
}

function getPathValue(value: JsonValue, path: string): JsonValue | undefined {
	if (!path) {
		return value;
	}

	const parts = path.match(/[^.[\]]+|\[(?:0|[1-9][0-9]*)\]|\["([^"]+)"\]|\['([^']+)'\]/g) ?? [];
	return parts.reduce<JsonValue | undefined>((currentValue, part) => {
		if (currentValue === undefined || currentValue === null || typeof currentValue !== "object") {
			return undefined;
		}

		const key = getPathPartKey(part);
		if (Array.isArray(currentValue)) {
			const index = Number(key);
			return Number.isInteger(index) ? currentValue[index] : undefined;
		}

		return currentValue[key];
	}, value);
}

function getPathPartKey(part: string) {
	if (!part.startsWith("[")) {
		return part;
	}

	if (/^\[(?:0|[1-9][0-9]*)\]$/.test(part)) {
		return part.slice(1, -1);
	}

	return part.slice(2, -2);
}

async function pushStep(
	context: SimulationContext,
	trace: SimulationTraceEntry,
	sideEffects: SimulationSideEffect[] = [],
) {
	if (trace.level === "error") {
		context.failed = true;
	}

	return emitStep(context, createTraceStep(context, truncateTrace(trace), sideEffects));
}

async function pushOutputLog(context: SimulationContext, log: LogEntry) {
	return emitStep(context, {
		outputLogs: [truncateLog(log)],
		sideEffects: [],
		traces: [],
		variables: createVariableSnapshot(context),
	});
}

async function emitStep(context: SimulationContext, step: SimulationStep) {
	context.steps.push(step);
	if (!context.onStep) {
		return [];
	}

	if (context.streamedSteps > 0 && context.stepDelayMs > 0) {
		await sleepSimulationStep(context.stepDelayMs, context.signal);
	}

	if (context.signal?.aborted) {
		return [];
	}

	context.streamedSteps += 1;
	return (await context.onStep(step)) ?? [];
}

function sleepSimulationStep(ms: number, signal: AbortSignal | undefined) {
	if (ms <= 0 || signal?.aborted) {
		return Promise.resolve();
	}

	return new Promise<void>((resolve) => {
		let completed = false;
		const finish = () => {
			if (completed) {
				return;
			}

			completed = true;
			window.clearTimeout(timeoutId);
			signal?.removeEventListener("abort", handleAbort);
			resolve();
		};
		const timeoutId = window.setTimeout(finish, ms);
		const handleAbort = () => finish();
		signal?.addEventListener("abort", handleAbort, { once: true });
	});
}

function createTraceStep(
	context: SimulationContext,
	trace: SimulationTraceEntry,
	sideEffects: SimulationSideEffect[] = [],
): SimulationStep {
	return {
		outputLogs: [],
		sideEffects,
		traces: [trace],
		variables: createVariableSnapshot(context),
	};
}

function createVariableSnapshot(context: SimulationContext): SimulationVariableSnapshot[] {
	const runtimeVariables = Object.entries(context.runtimeVariables).map(([name, value]) => ({
		name,
		source: "runtime" as const,
		value: createSnapshotValue(value),
	}));

	const nodeOutputVariables = Object.entries(context.nodeOutputs).flatMap(([nodeId, outputs]) =>
		flattenObject(outputs).map(([name, value]) => ({
			name: `${nodeId}.${name}`,
			source: "node_output" as const,
			value: createSnapshotValue(value),
		})),
	);

	return [...runtimeVariables, ...nodeOutputVariables]
		.sort((a, b) => a.name.localeCompare(b.name))
		.slice(0, MAX_VARIABLE_SNAPSHOT_ENTRIES);
}

function flattenObject(value: Record<string, JsonValue>, prefix = ""): Array<[string, JsonValue]> {
	return Object.entries(value).flatMap(([key, entry]) => {
		const name = prefix ? `${prefix}.${key}` : key;
		if (entry && typeof entry === "object" && !Array.isArray(entry)) {
			return [[name, entry] as [string, JsonValue], ...flattenObject(entry, name)];
		}

		return [[name, entry]];
	});
}

function createSnapshotValue(value: JsonValue): JsonValue {
	if (typeof value === "string") {
		return truncateText(value, MAX_VARIABLE_SNAPSHOT_STRING_LENGTH);
	}

	if (Array.isArray(value)) {
		return value.slice(0, 50).map(createSnapshotValue);
	}

	if (value && typeof value === "object") {
		return Object.fromEntries(
			Object.entries(value)
				.slice(0, 50)
				.map(([key, entry]) => [key, createSnapshotValue(entry)]),
		);
	}

	return value;
}

function groupEdgesBySource(edges: Edge[]) {
	return edges.reduce((edgesBySource, edge) => {
		const currentEdges = edgesBySource.get(edge.source) ?? [];
		currentEdges.push(edge);
		edgesBySource.set(edge.source, currentEdges);
		return edgesBySource;
	}, new Map<string, Edge[]>());
}

function sortNodesForExecution(nodes: Node<ScriptNodeData>[]) {
	return [...nodes].sort((a, b) => getNodeSortValue(a) - getNodeSortValue(b));
}

function getNodeSortValue(node: Node<ScriptNodeData> | undefined) {
	if (!node) {
		return Number.MAX_SAFE_INTEGER;
	}

	return node.position.y * 100000 + node.position.x;
}

function getConfigString(node: Node<ScriptNodeData>, key: string) {
	const value = node.data.config[key];
	if (typeof value === "string") {
		return value;
	}

	if (typeof value === "number" || typeof value === "boolean") {
		return String(value);
	}

	return "";
}

function getSimulatedProcessId(node: Node<ScriptNodeData>, context: SimulationContext) {
	if (getConfigString(node, "matchMode") !== "pid") {
		return 4242;
	}

	const target = resolveTemplate(getConfigString(node, "target"), context);
	const processId = Number(target);

	return Number.isFinite(processId) && processId >= 0 ? Math.trunc(processId) : 4242;
}

function isHeaderLike(value: JsonValue): value is { name: string; value: string } {
	return (
		typeof value === "object" &&
		value !== null &&
		!Array.isArray(value) &&
		typeof value.name === "string" &&
		typeof value.value === "string"
	);
}

function normalizeHttpMethod(value: string) {
	const method = value.trim().toUpperCase();
	const allowedMethods = new Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]);

	return allowedMethods.has(method) ? method : "GET";
}

function parseJsonValue(value: string): JsonValue | undefined {
	if (!value.trim()) {
		return undefined;
	}

	try {
		const parsed = JSON.parse(value) as unknown;
		return isJsonValue(parsed) ? parsed : undefined;
	} catch {
		return undefined;
	}
}

function isJsonValue(value: unknown): value is JsonValue {
	if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
		return true;
	}

	if (Array.isArray(value)) {
		return value.every(isJsonValue);
	}

	if (typeof value === "object") {
		return Object.values(value).every(isJsonValue);
	}

	return false;
}

function normalizePayloadRecord(
	value: Record<string, string> | undefined,
	fallback: Record<string, JsonValue> = {},
): Record<string, JsonValue> {
	if (!value) {
		return fallback;
	}

	return Object.fromEntries(
		Object.entries(value)
			.map(([key, entry]) => [key.trim(), entry] as const)
			.filter(([key]) => key.length > 0),
	);
}

function clampNumber(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}

function formatValue(value: JsonValue) {
	return typeof value === "string" ? value : JSON.stringify(value);
}

function normalizeLogLevel(value: string): LogEntry["level"] {
	if (value === "debug" || value === "warn" || value === "error") {
		return value;
	}

	return "info";
}

function truncateTrace(trace: SimulationTraceEntry): SimulationTraceEntry {
	return { ...trace, message: truncateText(trace.message) };
}

function truncateLog(log: LogEntry): LogEntry {
	return { ...log, message: truncateText(log.message) };
}

function truncateText(value: string, maxLength = MAX_SIMULATION_MESSAGE_LENGTH) {
	return value.length > maxLength ? `${value.slice(0, maxLength)}... [truncated]` : value;
}
