import type { Edge, Node } from "@xyflow/react";
import { isConditionRow, isSwitchCaseRow } from "@/data/nodes/definitions/rows";
import type { NodeSimulationApi } from "@/data/nodes/node-definition";
import { fallibleActionTypes, getNodeDefinition } from "@/data/nodes/registry";
import { createSimulationBuiltInVariableValues } from "@/data/project/built-in-variables";
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

const SIMULATION_YIELD_INTERVAL = 100;
const MAX_SIMULATION_MESSAGE_LENGTH = 4000;
const MAX_VARIABLE_SNAPSHOT_ENTRIES = 600;
const MAX_VARIABLE_SNAPSHOT_STRING_LENGTH = 4000;
const MAX_REGEX_PATTERN_LENGTH = 256;
const UNSAFE_REGEX_PATTERN =
	/(\([^)]*[+*][^)]*\)[+*?])|(\[[^\]]+\][+*?].*\[[^\]]+\][+*?])|((?:\.\*){2,})|((?:\w|\)|\]|\.|\+|\*)\{\d+,?\d*\}[+*?])/;

type SimulationFrame =
	| {
			kind: "edge";
			handle: string;
			sourceNodeId: string;
			stopAtNodeId?: string;
			targetNodeId: string;
	  }
	| {
			kind: "follow";
			handle: string;
			sourceNodeId: string;
			stopAtNodeId?: string;
	  }
	| {
			kind: "for_each";
			index: number;
			items: JsonValue[];
			nodeId: string;
	  }
	| {
			kind: "loop";
			count: number;
			index: number;
			nodeId: string;
	  }
	| {
			kind: "node";
			nodeId: string;
			stopAtNodeId?: string;
	  };

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
	projectSettings,
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
		runtimeVariables: createSimulationBuiltInVariableValues(projectSettings),
		signal,
		stepDelayMs,
		streamedSteps: 0,
		triggerPayload,
		webhookResponse: null,
	};

	if (!trigger) {
		await pushStep(context, {
			level: "error",
			message: "[Simulation] No trigger node exists, so the script cannot be simulated.",
		});
		return {
			finalVariables: createVariableSnapshot(context),
			status: "failed",
		};
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Starting from ${trigger.data.label} (${trigger.id}).`,
	});
	await processSimulationFrames(context, [{ kind: "node", nodeId: trigger.id }]);
	if (context.signal?.aborted) {
		await pushStep(context, {
			level: "warn",
			message: "[Simulation] Simulation stopped by user.",
		});
		return { finalVariables: createVariableSnapshot(context), status: "failed" };
	}
	await pushStep(context, {
		level: context.failed ? "error" : "info",
		message: context.failed ? "[Simulation] Simulation finished with errors." : "[Simulation] Simulation completed.",
	});

	return { finalVariables: createVariableSnapshot(context), status: context.failed ? "failed" : "completed" };
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

async function processSimulationFrames(context: SimulationContext, initialFrames: SimulationFrame[]) {
	const frames = [...initialFrames];

	while (frames.length > 0) {
		if (context.halted || context.signal?.aborted) {
			return;
		}

		const frame = frames.pop();
		if (!frame) {
			continue;
		}

		await processSimulationFrame(context, frame, frames);
	}
}

async function processSimulationFrame(context: SimulationContext, frame: SimulationFrame, frames: SimulationFrame[]) {
	if (frame.kind === "node") {
		await executeNodeFrame(frame.nodeId, context, frames, frame.stopAtNodeId);
		return;
	}

	if (frame.kind === "follow") {
		await enqueueFollowFrames(context, frames, frame.sourceNodeId, frame.handle, frame.stopAtNodeId);
		return;
	}

	if (frame.kind === "edge") {
		await pushStep(context, {
			level: "debug",
			message: `[Simulation] Following "${frame.handle}" from ${frame.sourceNodeId} to ${frame.targetNodeId}.`,
		});
		frames.push({ kind: "node", nodeId: frame.targetNodeId, stopAtNodeId: frame.stopAtNodeId });
		return;
	}

	if (frame.kind === "loop") {
		await processLoopFrame(context, frames, frame);
		return;
	}

	await processForEachFrame(context, frames, frame);
}

async function executeNodeFrame(
	nodeId: string,
	context: SimulationContext,
	frames: SimulationFrame[],
	stopAtNodeId?: string,
) {
	if (context.halted || context.signal?.aborted) {
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

	const afterExecuteTraces =
		(await getNodeDefinition(node.data.actionType)?.simulation?.afterExecute?.({
			api: nodeSimulationApi,
			context,
			failed,
			node,
			sideEffectResults,
		})) ?? [];

	for (const trace of afterExecuteTraces) {
		await pushStep(context, trace);
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

	if (node.data.actionType === "control.loop") {
		const count = normalizeIterationCount(resolveTemplate(getConfigString(node, "count"), context));
		frames.push({ kind: "loop", nodeId: node.id, index: 0, count });
		return;
	}

	if (node.data.actionType === "control.for_each") {
		const items = getForEachItems(node, context);
		frames.push({ kind: "for_each", nodeId: node.id, index: 0, items });
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

	frames.push({ kind: "follow", sourceNodeId: node.id, handle });
}

async function processLoopFrame(
	context: SimulationContext,
	frames: SimulationFrame[],
	frame: Extract<SimulationFrame, { kind: "loop" }>,
) {
	const node = context.nodesById.get(frame.nodeId);
	if (!node) {
		await pushStep(context, {
			level: "error",
			message: `[Simulation] Missing loop node ${frame.nodeId}; branch stopped.`,
		});
		return;
	}

	if (frame.index >= frame.count) {
		frames.push({ kind: "follow", sourceNodeId: node.id, handle: "done" });
		return;
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Loop ${node.id} iteration ${frame.index + 1} of ${frame.count}.`,
	});
	frames.push({ kind: "loop", nodeId: node.id, index: frame.index + 1, count: frame.count });
	frames.push({ kind: "follow", sourceNodeId: node.id, handle: "loop", stopAtNodeId: node.id });
}

async function processForEachFrame(
	context: SimulationContext,
	frames: SimulationFrame[],
	frame: Extract<SimulationFrame, { kind: "for_each" }>,
) {
	const node = context.nodesById.get(frame.nodeId);
	if (!node) {
		await pushStep(context, {
			level: "error",
			message: `[Simulation] Missing for-each node ${frame.nodeId}; branch stopped.`,
		});
		return;
	}

	if (frame.index >= frame.items.length) {
		frames.push({ kind: "follow", sourceNodeId: node.id, handle: "done" });
		return;
	}

	const item = frame.items[frame.index];
	const itemVariable = getConfigString(node, "itemVariable").trim();
	const indexVariable = getConfigString(node, "indexVariable").trim();
	context.nodeOutputs[node.id] = { item, index: frame.index };
	if (itemVariable) {
		context.runtimeVariables[itemVariable] = item;
	}
	if (indexVariable) {
		context.runtimeVariables[indexVariable] = frame.index;
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] For Each ${node.id} item ${frame.index + 1} of ${frame.items.length}.`,
	});
	frames.push({ kind: "for_each", nodeId: node.id, index: frame.index + 1, items: frame.items });
	frames.push({ kind: "follow", sourceNodeId: node.id, handle: "loop", stopAtNodeId: node.id });
}

async function enqueueFollowFrames(
	context: SimulationContext,
	frames: SimulationFrame[],
	nodeId: string,
	handle: string,
	stopAtNodeId?: string,
) {
	if (context.halted || context.signal?.aborted) {
		return;
	}

	const node = context.nodesById.get(nodeId);
	if (!node) {
		await pushStep(context, {
			level: "error",
			message: `[Simulation] Missing source node ${nodeId}; branch stopped.`,
		});
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

	for (const edge of [...outgoingEdges].reverse()) {
		frames.push({
			kind: "edge",
			sourceNodeId: node.id,
			targetNodeId: edge.target,
			handle,
			stopAtNodeId,
		});
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
	if (pattern.length > MAX_REGEX_PATTERN_LENGTH || UNSAFE_REGEX_PATTERN.test(pattern)) {
		return false;
	}

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

	return { failed: false, outputData: {} };
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
		message: `[Simulation] Delay (${node.id}) waiting ${delay.label}.`,
	});

	await sleepSimulationStep(delay.waitMs, context.signal);
	if (context.signal?.aborted) {
		return;
	}

	await pushStep(context, {
		level: "info",
		message: `[Simulation] Delay (${node.id}) completed after ${delay.label}.`,
	});
}

function getDelaySimulationDuration(node: Node<ScriptNodeData>, context: SimulationContext) {
	const amountValue = Number(resolveTemplate(getConfigString(node, "amount"), context));
	const safeAmount = Number.isFinite(amountValue) && amountValue > 0 ? amountValue : 0;
	const configuredMs = Math.round(safeAmount * getDelayUnitMultiplier(getConfigString(node, "unit")));

	return {
		label: `${formatValue(resolveTemplate(getConfigString(node, "amount"), context))} ${normalizeDelayUnit(getConfigString(node, "unit"))}`,
		waitMs: configuredMs,
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

function createNodeSideEffects(node: Node<ScriptNodeData>, context: SimulationContext): SimulationSideEffect[] {
	const customSideEffects = getNodeDefinition(node.data.actionType)?.simulation?.sideEffects;
	if (customSideEffects) {
		return customSideEffects({ api: nodeSimulationApi, context, node });
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

function getExecutionDetail(node: Node<ScriptNodeData>, _context: SimulationContext, failed: boolean) {
	if (failed) {
		return "The failed output will be used when it is connected.";
	}

	return node.data.kind === "trigger" ? "Trigger fired." : "";
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

function resolveTemplate(value: string, context: SimulationContext): JsonValue {
	const exactReference = value.match(/^\{\{\s*([^}]+?)\s*\}\}$/);
	if (exactReference) {
		return getReferenceValue(exactReference[1].trim(), context);
	}

	return value.replace(/\{\{\s*([^}]+?)\s*\}\}/g, (_match, reference: string) =>
		String(getReferenceValue(reference.trim(), context)),
	);
}

function normalizeIterationCount(value: JsonValue) {
	if (typeof value === "number") {
		return Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0;
	}

	if (typeof value === "string") {
		const count = Number(value.trim());
		return Number.isFinite(count) ? Math.max(0, Math.trunc(count)) : 0;
	}

	return 0;
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
		const key = getPathPartKey(part);
		if (key.startsWith("$")) {
			return getDerivedValueMetadata(currentValue, key);
		}

		if (currentValue === undefined || currentValue === null || typeof currentValue !== "object") {
			return undefined;
		}

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

function getDerivedValueMetadata(value: JsonValue | undefined, key: string): JsonValue | undefined {
	if (key === "$length" || key === "$count") {
		return getValueLength(value);
	}

	if (key === "$type") {
		return getValueType(value);
	}

	if (key === "$is_empty") {
		return isValueEmpty(value);
	}

	return undefined;
}

function getValueLength(value: JsonValue | undefined) {
	if (typeof value === "string" || Array.isArray(value)) {
		return value.length;
	}

	if (value && typeof value === "object") {
		return Object.keys(value).length;
	}

	return 0;
}

function getValueType(value: JsonValue | undefined) {
	if (Array.isArray(value)) {
		return "list";
	}

	if (value === null) {
		return "null";
	}

	if (value === undefined) {
		return "missing";
	}

	return typeof value;
}

function isValueEmpty(value: JsonValue | undefined) {
	if (value === undefined || value === null) {
		return true;
	}

	if (typeof value === "string" || Array.isArray(value)) {
		return value.length === 0;
	}

	if (typeof value === "object") {
		return Object.keys(value).length === 0;
	}

	return false;
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
	if (context.streamedSteps % SIMULATION_YIELD_INTERVAL === 0) {
		await yieldSimulationFrame(context.signal);
	}

	if (context.signal?.aborted) {
		return [];
	}

	return (await context.onStep(step)) ?? [];
}

function yieldSimulationFrame(signal: AbortSignal | undefined) {
	if (signal?.aborted) {
		return Promise.resolve();
	}

	return new Promise<void>((resolve) => {
		window.setTimeout(resolve, 0);
	});
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

function clampNumber(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}

function formatValue(value: JsonValue) {
	return typeof value === "string" ? value : JSON.stringify(value);
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
