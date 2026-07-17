import type { LucideIcon } from "lucide-react";

export type NodeKind = "trigger" | "control" | "action";

export type RiskLevel = "low" | "medium" | "high" | "dangerous";

export type InspectorTab = "properties" | "simulator";

export type ActionType =
	| "trigger.manual"
	| "trigger.schedule"
	| "trigger.file_watch"
	| "trigger.webhook"
	| "trigger.websocket"
	| "trigger.hotkey"
	| "trigger.serial_input"
	| "trigger.startup"
	| "trigger.process_started"
	| "control.if"
	| "control.color_match"
	| "control.switch"
	| "control.loop"
	| "control.while"
	| "control.for_each"
	| "runtime.set_variable"
	| "action.calculate"
	| "action.text.format"
	| "action.log"
	| "action.delay"
	| "action.http"
	| "action.webhook_response"
	| "action.websocket.write"
	| "action.notification"
	| "action.message_box"
	| "action.pixel.get"
	| "action.file.read"
	| "action.file.download"
	| "action.file.write"
	| "action.file.delete"
	| "action.file.copy"
	| "action.file.move"
	| "action.process.run"
	| "action.process.status"
	| "action.process.kill"
	| "action.script.run"
	| "action.application.open"
	| "action.sound.play"
	| "action.serial.write"
	| "action.keyboard"
	| "action.keyboard.type_text"
	| "action.mouse"
	| "action.mouse.move"
	| "action.window.active"
	| "action.window.focus"
	| "action.beep"
	| "action.clipboard.set"
	| "action.clipboard.get"
	| "action.shell";

export type TriggerActionType = Extract<ActionType, `trigger.${string}`>;
export type ControlActionType = Extract<ActionType, `control.${string}`>;
export type ExecutableActionType = Extract<ActionType, `action.${string}`>;

export type TargetRuntime =
	| "Generic Headless"
	| "Linux Headless"
	| "Windows Headless"
	| "Generic Desktop"
	| "Windows Desktop"
	| "Linux Desktop";

export type ProjectSettings = {
	name: string;
	description: string;
	author: string;
	website: string;
	repository: string;
	tags: string[];
	targetRuntime: TargetRuntime;
	minimumRunnerVersion: string;
};

export type NodePort = {
	id: string;
	label: string;
};

export type RuntimeDataType =
	| "string"
	| "number"
	| "boolean"
	| "object"
	| "list"
	| "file_content"
	| "file_path"
	| "http_headers"
	| "http_status_code"
	| "duration_ms"
	| "process_id"
	| "exit_code"
	| "keyboard_key";

export type RuntimeDataOutput = {
	name: string;
	type: RuntimeDataType;
	description: string;
	example?: string;
	fields?: RuntimeDataField[];
};

export type RuntimeDataField = {
	name: string;
	type: RuntimeDataType;
	description: string;
	example?: string;
};

export type JsonPrimitive = string | number | boolean | null;

export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export type ScriptNodeData = {
	label: string;
	kind: NodeKind;
	actionType: ActionType;
	risk: RiskLevel;
	config: Record<string, JsonValue>;
	inputs: NodePort[];
	outputs: NodePort[];
	runtimeOutputs: RuntimeDataOutput[];
};

export type CommentNodeData = {
	editorOnly: true;
	text: string;
	size: {
		width: number;
		height: number;
	};
	color: "amber" | "blue" | "green" | "rose" | "violet";
	fontSize: number;
};

export type EditorComment = {
	id: string;
	text: string;
	position: {
		x: number;
		y: number;
	};
	size: {
		width: number;
		height: number;
	};
	color: "amber" | "blue" | "green" | "rose" | "violet";
	fontSize: number;
};

export type PaletteItem = {
	label: string;
	actionType: ActionType;
	kind: NodeKind;
	icon: LucideIcon;
	risk: RiskLevel;
	description: string;
};

export type PaletteGroup = {
	id: string;
	label: string;
	icon: LucideIcon;
	items: PaletteItem[];
	children?: PaletteGroup[];
};

export type PermissionSummary = {
	name: string;
	risk: RiskLevel;
};

export type SecretDeclaration = {
	description: string;
	name: string;
	required: boolean;
	type: import("@/data/project/variables").VariableType;
};

export type DefaultVariable = {
	description: string;
	name: string;
	scope: "runtime" | "persistent";
	type: import("@/data/project/variables").VariableType;
	value: JsonValue;
};

export type CapabilitySummary = {
	name: string;
};

export type ExportSummary = {
	filename: string;
	formatVersion: number;
	languageVersion: number;
	minimumRunnerVersion: string;
	targetRuntime: TargetRuntime;
	contents: string[];
};

export type AssetKind = "audio" | "image" | "text";

export type AssetManifestEntry = {
	id: string;
	kind: AssetKind;
	mediaType: string;
	name: string;
	packagePath: string;
	size: number;
};

export type EditorAsset = AssetManifestEntry & {
	createdAt: string;
	file: File;
};

export type LogEntry = {
	level: "debug" | "info" | "warn" | "error";
	message: string;
};

export type SimulationOverrideOutcome = "success" | "failed";

export type SimulationOverride = {
	nodeId: string;
	outcome: SimulationOverrideOutcome;
};

export type SimulationSpeed = "slow" | "normal" | "fast" | "instant";

export type SimulationSettings = {
	speed: SimulationSpeed;
};

export type SimulationRunStatus = "idle" | "waiting" | "running" | "completed" | "failed" | "stopped";

export type SimulationTriggerPayload = {
	body?: string;
	connectionId?: string;
	data?: string;
	event?: string;
	executablePath?: string;
	headers?: Record<string, string>;
	key?: string;
	message?: string;
	method?: string;
	path?: string;
	processId?: string;
	processName?: string;
	query?: Record<string, string>;
	reason?: string;
	remoteAddress?: string;
	windowTitle?: string;
};

export type SimulationVariableSnapshot = {
	name: string;
	source: "runtime" | "node_output" | "secret";
	value: JsonValue;
};

export type SimulationTraceEntry = {
	level: LogEntry["level"];
	message: string;
};
