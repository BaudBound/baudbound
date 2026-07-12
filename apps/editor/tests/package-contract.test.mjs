import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const appRoot = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = join(appRoot, "..", "..");
const schemasRoot = join(repoRoot, "schemas");
const runnerCapabilityContractPath = join(
	repoRoot,
	"crates",
	"baudbound-security",
	"contracts",
	"node-capabilities.json",
);
const runnerPermissionContractPath = join(
	repoRoot,
	"crates",
	"baudbound-security",
	"contracts",
	"node-permissions.json",
);
const runnerPortContractPath = join(repoRoot, "crates", "baudbound-script", "contracts", "node-ports.json");

test("schemas are valid JSON", () => {
	for (const filePath of readJsonFiles(schemasRoot)) {
		assert.doesNotThrow(() => JSON.parse(read(filePath)), filePath);
	}
});

test("generated node schemas are current", () => {
	assert.doesNotThrow(() => {
		execFileSync("node", ["scripts/generate-node-schemas.mjs", "--check"], {
			cwd: appRoot,
			stdio: "pipe",
		});
	});
});

test("generated runner capability contract covers every editor node", () => {
	const contract = JSON.parse(read(runnerCapabilityContractPath));
	const definitionsSource = readDefinitions();
	const actionTypes = extractDefinitionActionTypes(definitionsSource).sort();

	assert.equal(contract.version, 1);
	assert.deepEqual(Object.keys(contract.nodes).sort(), actionTypes);
	for (const [actionType, capabilities] of Object.entries(contract.nodes)) {
		assert.ok(capabilities.length > 0, `${actionType} must require at least one capability`);
		assert.equal(new Set(capabilities).size, capabilities.length, `${actionType} capabilities must be unique`);
	}
});

test("generated runner permission contract covers every editor node", () => {
	const contract = JSON.parse(read(runnerPermissionContractPath));
	const actionTypes = extractDefinitionActionTypes(readDefinitions()).sort();

	assert.equal(contract.version, 1);
	assert.deepEqual(Object.keys(contract.nodes).sort(), actionTypes);
	assert.deepEqual(contract.nodes["action.file.read"].path_rules, [{ access: "read", config_key: "path" }]);
	assert.deepEqual(contract.nodes["action.file.copy"].path_rules, [
		{ access: "read", config_key: "sourcePath" },
		{ access: "write", config_key: "destinationPath" },
	]);
	assert.equal(contract.nodes["trigger.manual"].permission, null);
});

test("generated runner port contract covers every editor node", () => {
	const contract = JSON.parse(read(runnerPortContractPath));
	const actionTypes = extractDefinitionActionTypes(readDefinitions()).sort();

	assert.equal(contract.version, 1);
	assert.deepEqual(Object.keys(contract.nodes).sort(), actionTypes);
	assert.deepEqual(contract.nodes["trigger.manual"], { kind: "fixed", inputs: [], outputs: ["out"] });
	assert.deepEqual(contract.nodes["action.log"], { kind: "fixed", inputs: ["input"], outputs: ["out"] });
	assert.deepEqual(contract.nodes["action.http"], {
		kind: "fixed",
		inputs: ["input"],
		outputs: ["success", "failed"],
	});
	assert.deepEqual(contract.nodes["control.switch"], {
		kind: "switch_cases",
		config_key: "cases",
		input: "input",
		output_prefix: "case-",
	});
});

test("program schema uses public per-node schema references", () => {
	const programSchema = JSON.parse(read(join(schemasRoot, "program.schema.json")));
	const refs = [
		...programSchema.$defs.trigger.oneOf,
		...programSchema.$defs.controlStep.oneOf,
		...programSchema.$defs.variableOperationStep.oneOf,
		...programSchema.$defs.actionStep.oneOf,
	].map((entry) => entry.$ref);

	assert.ok(refs.length > 0, "program schema must reference generated node schemas");
	for (const ref of refs) {
		assert.match(ref, /^https:\/\/schemas\.baudbound\.app\/nodes\/.+\.schema\.json$/);
		const fileName = ref.split("/").at(-1);
		assert.ok(
			readJsonFiles(join(schemasRoot, "nodes")).some((filePath) => filePath.endsWith(fileName)),
			`${ref} file is missing`,
		);
	}
});

test("node schema files include every editor action type", () => {
	const typesSource = read(join(appRoot, "lib", "types.ts"));
	const actionTypeBlock = typesSource.match(/export type ActionType =([\s\S]*?);/)?.[1] ?? "";
	const actionTypes = [...actionTypeBlock.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
	const schemaFiles = new Set(
		readJsonFiles(join(schemasRoot, "nodes")).map((filePath) => filePath.split(/[\\/]/).at(-1)),
	);

	for (const actionType of actionTypes) {
		const fileName = `${actionType.replaceAll(".", "-").replaceAll("_", "-")}.schema.json`;
		assert.ok(schemaFiles.has(fileName), `${fileName} is missing for ${actionType}`);
	}
});

test("node schemas restrict each node config object", () => {
	for (const filePath of readJsonFiles(join(schemasRoot, "nodes"))) {
		const schema = JSON.parse(read(filePath));
		const config = schema.$defs?.config;
		assert.equal(config?.type, "object", `${filePath} config must be an object schema`);
		assert.equal(config?.additionalProperties, false, `${filePath} config must reject unknown fields`);
		assert.ok(config.properties?.customName, `${filePath} should allow customName`);
	}
});

test("select config fields produce enum values in generated node schemas", () => {
	const httpSchema = JSON.parse(read(join(schemasRoot, "nodes", "action-http.schema.json")));
	const variableSchema = JSON.parse(read(join(schemasRoot, "nodes", "runtime-set-variable.schema.json")));

	assert.deepEqual(httpSchema.$defs.config.properties.method.enum, [
		"GET",
		"POST",
		"PUT",
		"PATCH",
		"DELETE",
		"HEAD",
		"OPTIONS",
	]);
	assert.deepEqual(variableSchema.$defs.config.properties.operation.enum, [
		"set",
		"increment",
		"append_list",
		"set_object_field",
		"clear",
	]);
});

test("schemas are served with public canonical ids", () => {
	for (const filePath of readJsonFiles(schemasRoot)) {
		const schema = JSON.parse(read(filePath));
		if (!schema.$id) {
			continue;
		}

		assert.match(schema.$id, /^https:\/\/schemas\.baudbound\.app\//, `${filePath} has non-public $id`);
	}
});

test("editor schema and package contract support editor-only metadata", () => {
	const editorSchema = JSON.parse(read(join(schemasRoot, "editor.schema.json")));
	const programSchema = JSON.parse(read(join(schemasRoot, "program.schema.json")));
	const packageContractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));

	assert.ok(editorSchema.properties.comments, "editor.schema.json should define editor-only comments");
	assert.deepEqual(editorSchema.properties.canvas.properties.edge_style.enum, [
		"smoothstep",
		"bezier",
		"straight",
		"step",
	]);
	assert.deepEqual(editorSchema.properties.comments.items.properties.color.enum, [
		"amber",
		"blue",
		"green",
		"rose",
		"violet",
	]);
	assert.deepEqual(editorSchema.properties.comments.items.properties.font_size, {
		type: "number",
		minimum: 12,
		maximum: 72,
	});
	assert.match(packageContractSource, /editor\.json comments/);
	assert.match(packageContractSource, /canvas\.edge_style/);
	assert.match(packageContractSource, /font_size from 12 to 72/);
	assert.match(packageSource, /edge_style/);
	assert.match(packageSource, /font_size/);
	assert.match(packageSource, /comments: comments\.map/);
	assert.match(packageSource, /function toEditorComments/);
	assert.equal(
		programSchema.$defs.actionType.enum.includes("commentNode"),
		false,
		"comments must not be program nodes",
	);
});

test("editor edge style metadata is mapped to valid React Flow edge types", () => {
	const flowCanvasDataSource = read(join(appRoot, "data", "editor", "flow-canvas.ts"));
	const flowCanvasSource = read(join(appRoot, "components", "canvas", "flow-canvas.tsx"));
	const editorPageSource = read(join(appRoot, "app", "editor-page.tsx"));

	assert.match(flowCanvasDataSource, /edgeStyle === "bezier" \? "default" : edgeStyle/);
	assert.match(flowCanvasSource, /type: toReactFlowEdgeType\(edgeStyle\)/);
	assert.match(flowCanvasSource, /addEdge\(\{ \.\.\.connection, id: edgeId, type: toReactFlowEdgeType\(edgeStyle\) \}/);
	assert.match(editorPageSource, /type: toReactFlowEdgeType\(nextEdgeStyle\)/);
	assert.match(editorPageSource, /type: toReactFlowEdgeType\(importedPackage\.edgeStyle\)/);
});

test("program schema includes every editor action type", () => {
	const typesSource = read(join(appRoot, "lib", "types.ts"));
	const actionTypeBlock = typesSource.match(/export type ActionType =([\s\S]*?);/)?.[1] ?? "";
	const actionTypes = [...actionTypeBlock.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
	const programSchema = JSON.parse(read(join(schemasRoot, "program.schema.json")));
	const schemaActionTypes = new Set(programSchema.$defs.actionType.enum);

	assert.ok(actionTypes.length > 0, "ActionType union should not be empty");
	for (const actionType of actionTypes) {
		assert.ok(schemaActionTypes.has(actionType), `${actionType} is missing from program.schema.json`);
	}
});

test("serial input stores only logical device ids in script packages", () => {
	const serialInputSource = read(join(appRoot, "data", "nodes", "definitions", "triggers", "serial-input.ts"));
	const serialProjectSource = read(join(appRoot, "data", "project", "serial.ts"));
	const serialSchema = JSON.parse(read(join(schemasRoot, "nodes", "trigger-serial-input.schema.json")));
	const configKeys = new Set(Object.keys(serialSchema.$defs.config.properties));

	assert.deepEqual([...configKeys].sort(), ["customName", "deviceId"].sort());
	assert.match(serialInputSource, /key:\s*"deviceId"/);
	assert.doesNotMatch(serialInputSource, /key:\s*"port"/);
	assert.doesNotMatch(serialInputSource, /validateUsbIdentity/);
	assert.doesNotMatch(serialProjectSource, /baudRate|vendorId|productId|\bport\b/);
});

test("file watch uses a static path and explicit recursive configuration", () => {
	const fileWatchSource = read(join(appRoot, "data", "nodes", "definitions", "triggers", "file-watch.ts"));
	const fileWatchSchema = JSON.parse(read(join(schemasRoot, "nodes", "trigger-file-watch.schema.json")));
	const configSchema = fileWatchSchema.$defs.config;

	assert.deepEqual(configSchema.required, ["path"]);
	assert.deepEqual(configSchema.properties.path, { type: "string" });
	assert.deepEqual(configSchema.properties.recursive, { type: "boolean" });
	assert.match(fileWatchSource, /requiredStaticConfig\(config, "path", "file watch path"\)/);
	assert.doesNotMatch(fileWatchSource, /key:\s*"path"[^\n]*usesVariables/);
});

test("schedule intervals use the shared static duration contract", () => {
	const scheduleSource = read(join(appRoot, "data", "nodes", "definitions", "triggers", "schedule.ts"));
	const validatorsSource = read(join(appRoot, "data", "nodes", "definitions", "validators.ts"));

	assert.match(scheduleSource, /staticPositiveDurationConfig\(config, "every", "unit", "schedule interval"\)/);
	assert.match(validatorsSource, /seconds < 1e-9/);
	assert.match(validatorsSource, /cannot use runtime variable references/);
});

test("runner types used by node definitions are declared by the program schema", () => {
	const programSchema = JSON.parse(read(join(schemasRoot, "program.schema.json")));
	const allowedRunnerTypes = new Set([
		...programSchema.$defs.triggerType.enum,
		...programSchema.$defs.actionRunnerType.enum,
		...programSchema.$defs.controlStepType.enum,
		"set_variable",
	]);
	const definitionsSource = readDefinitions();
	const runnerTypes = [...definitionsSource.matchAll(/runnerType:\s*"([^"]+)"/g)].map((match) => match[1]);

	assert.ok(runnerTypes.length > 0, "node definitions should declare runner types");
	for (const runnerType of runnerTypes) {
		assert.ok(allowedRunnerTypes.has(runnerType), `${runnerType} is missing from program.schema.json`);
	}
});

test("editor node action types are covered by runner support owners", () => {
	const definitionsSource = readDefinitions();
	const actionCrateSource = read(join(repoRoot, "crates", "baudbound-actions", "src", "lib.rs"));
	const coreSource = read(join(repoRoot, "crates", "baudbound-core", "src", "lib.rs"));
	const runtimeSource = read(join(repoRoot, "crates", "baudbound-runtime", "src", "execution", "contracts.rs"));
	const triggerSource = read(join(repoRoot, "crates", "baudbound-triggers", "src", "lib.rs"));
	const editorActionTypes = extractDefinitionActionTypes(definitionsSource);
	const editorExecutableActionTypes = editorActionTypes
		.filter((actionType) => actionType.startsWith("action.") || actionType === "runtime.set_variable")
		.sort();
	const editorControlActionTypes = editorActionTypes.filter((actionType) => actionType.startsWith("control.")).sort();
	const editorTriggerActionTypes = editorActionTypes.filter((actionType) => actionType.startsWith("trigger.")).sort();
	const runnerExecutableActionTypes = uniqueSorted([
		...extractRustConstStringArray(actionCrateSource, "SUPPORTED_ACTION_TYPES"),
		...extractRustConstStringArray(coreSource, "SUPPORTED_CORE_ACTION_TYPES"),
		...extractRustConstStringArray(runtimeSource, "SUPPORTED_INTERNAL_ACTION_TYPES"),
	]);
	const runnerControlActionTypes = uniqueSorted(
		extractRustConstStringArray(runtimeSource, "SUPPORTED_CONTROL_ACTION_TYPES"),
	);
	const runnerTriggerActionTypes = uniqueSorted([
		...extractRustConstStringArray(coreSource, "SUPPORTED_CORE_TRIGGER_ACTION_TYPES"),
		...extractRustConstStringArray(triggerSource, "SUPPORTED_SERVICE_TRIGGER_ACTION_TYPES"),
	]);
	const desktopAdapterActionTypes = extractRustConstStringArray(actionCrateSource, "DESKTOP_ADAPTER_ACTION_TYPES");

	assert.deepEqual(
		editorExecutableActionTypes,
		runnerExecutableActionTypes,
		"every editor executable action must have exactly one runner support owner",
	);
	assert.deepEqual(
		editorControlActionTypes,
		runnerControlActionTypes,
		"every editor control action must be explicitly supported by baudbound-runtime",
	);
	assert.deepEqual(
		editorTriggerActionTypes,
		runnerTriggerActionTypes,
		"every editor trigger action must be explicitly supported by core or trigger services",
	);

	for (const actionType of desktopAdapterActionTypes) {
		assert.ok(
			runnerExecutableActionTypes.includes(actionType),
			`${actionType} desktop adapter support must also be declared as an executable action`,
		);
	}
});

test("old capability and permission strings are not used by node definitions", () => {
	const definitionsSource = readDefinitions();
	const staleStrings = [
		"trigger.serial",
		"trigger.process",
		"runtime.foreach",
		"runtime.calculate",
		"runtime.format_text",
		"action.audio",
		"action.screen",
		"action.script",
		"file_delete",
		"file_download",
		"active_window_read",
		"play_audio",
		"run_subscript",
		"read_clipboard",
	];

	for (const staleString of staleStrings) {
		assert.equal(
			new RegExp(`"${escapeRegExp(staleString)}"`).test(definitionsSource),
			false,
			`${staleString} should not be used`,
		);
	}
});

test("simulator does not retain streamed step history or use recursive traversal helpers", () => {
	const simulationSource = read(join(appRoot, "utils", "simulation.ts"));

	assert.equal(/context\.steps|steps:\s*\[\]|steps:\s*context\./.test(simulationSource), false);
	assert.match(simulationSource, /type SimulationFrame/);
	assert.match(simulationSource, /processSimulationFrames/);
	assert.equal(/async function followHandle|async function executeNode\(/.test(simulationSource), false);
	assert.equal(/switch\s*\(\s*node\.data\.actionType\s*\)/.test(simulationSource), false);
});

test("simulation audio side effects clean up object URLs deterministically", () => {
	const sideEffectsSource = read(join(appRoot, "utils", "simulation-side-effects.ts"));

	assert.match(sideEffectsSource, /URL\.createObjectURL\(asset\.file\)/);
	assert.match(sideEffectsSource, /URL\.revokeObjectURL\(audioUrl\)/);
	assert.match(sideEffectsSource, /audio\.addEventListener\("ended", handleEnded/);
	assert.match(sideEffectsSource, /finally\s*\{\s*cleanup\(\);/);
	assert.equal(sideEffectsSource.includes("10 * 60 * 1000"), false);
});

test("node definitions include production metadata required by package analysis", () => {
	const definitionsSource = readDefinitions();
	const actionTypes = [...definitionsSource.matchAll(/actionType:\s*"([^"]+)"/g)].map((match) => match[1]);

	assert.ok(actionTypes.length > 0, "node definitions should declare action types");
	for (const actionType of actionTypes) {
		const definitionBlock = getDefinitionBlock(definitionsSource, actionType);
		assert.match(definitionBlock, /capabilities:\s*/, `${actionType} must declare capabilities`);
		assert.match(definitionBlock, /description:\s*"[^"]+"/, `${actionType} must declare a description`);
		assert.match(definitionBlock, /group:\s*"(triggers|control|actions)"/, `${actionType} must declare a group`);
		assert.match(definitionBlock, /kind:\s*"(trigger|control|action)"/, `${actionType} must declare a kind`);
		assert.match(definitionBlock, /label:\s*"[^"]+"/, `${actionType} must declare a label`);
		assert.match(definitionBlock, /risk:\s*"(low|medium|high|dangerous)"/, `${actionType} must declare risk`);

		if (
			actionType.startsWith("trigger.") ||
			actionType.startsWith("action.") ||
			actionType === "runtime.set_variable"
		) {
			assert.match(definitionBlock, /runnerType:\s*"[^"]+"/, `${actionType} must declare a runner type`);
		}

		if (actionType.startsWith("action.") || actionType === "runtime.set_variable") {
			assert.match(definitionBlock, /permission:\s*\{/, `${actionType} must declare permission metadata`);
		}
	}
});

test("native Windows-only desktop nodes declare target runtime compatibility", () => {
	const definitionsSource = readDefinitions();

	for (const actionType of [
		"action.keyboard",
		"action.keyboard.type_text",
		"action.message_box",
		"action.mouse",
		"action.mouse.move",
		"action.pixel.get",
		"action.window.active",
		"action.window.focus",
		"trigger.hotkey",
	]) {
		const definitionBlock = getDefinitionBlock(definitionsSource, actionType);
		assert.match(
			definitionBlock,
			/supportedTargetRuntimes:\s*\[\s*"Windows Desktop"\s*\]/,
			`${actionType} must be restricted to Windows Desktop until native backends exist for other desktop targets`,
		);
	}
});

test("window-title process matching has config-sensitive Windows Desktop validation", () => {
	const definitionsSource = readDefinitions();

	for (const actionType of ["action.process.status", "action.process.kill", "trigger.process_started"]) {
		const definitionBlock = getDefinitionBlock(definitionsSource, actionType);
		assert.match(
			definitionBlock,
			/validateTargetRuntime:/,
			`${actionType} must validate its match mode for the target`,
		);
		assert.match(
			definitionBlock,
			/windowsDesktopOnlyConfigValue/,
			`${actionType} must restrict window-title matching to Windows Desktop`,
		);
	}
});

test("editor and runner target runtime compatibility policies stay aligned", () => {
	const runnerPolicySource = read(join(repoRoot, "crates", "baudbound-core", "src", "compatibility.rs"));
	const definitionsSource = readDefinitions();
	const editorWindowsOnly = extractWindowsDesktopOnlyActionTypes(definitionsSource).sort();
	const editorDesktopOnly = extractDesktopOnlyActionTypes(definitionsSource).sort();
	const runnerWindowsOnly = extractRustConstStringArray(runnerPolicySource, "WINDOWS_DESKTOP_ONLY_ACTIONS").sort();
	const runnerDesktopOnly = extractRustConstStringArray(runnerPolicySource, "DESKTOP_ONLY_ACTIONS").sort();

	assert.deepEqual(editorWindowsOnly, runnerWindowsOnly, "Windows-only target runtime policies must match");
	assert.deepEqual(editorDesktopOnly, runnerDesktopOnly, "desktop-only target runtime policies must match");

	for (const actionType of [...editorWindowsOnly, ...editorDesktopOnly]) {
		const definitionBlock = getDefinitionBlock(definitionsSource, actionType);
		assert.match(definitionBlock, /desktopOnly:\s*true/, `${actionType} must be marked desktopOnly in the editor`);
	}
});

test("removed Apple target runtimes are not exposed or accepted", () => {
	const targetRuntimeSource = read(join(appRoot, "data", "project", "runtimes.ts"));
	const typeSource = read(join(appRoot, "lib", "types.ts"));
	const capabilitiesSchema = JSON.parse(read(join(schemasRoot, "capabilities.schema.json")));
	const runnerPolicySource = read(join(repoRoot, "crates", "baudbound-core", "src", "compatibility.rs"));
	const removedTargetPrefix = ["mac", "OS"].join("");

	assert.ok(!targetRuntimeSource.includes(removedTargetPrefix));
	assert.ok(!typeSource.includes(removedTargetPrefix));
	assert.ok(!runnerPolicySource.includes(removedTargetPrefix));
	assert.ok(
		!capabilitiesSchema.properties.target_runtime.enum.some((targetRuntime) =>
			targetRuntime.includes(removedTargetPrefix),
		),
		"capabilities schema must not expose removed Apple runtimes",
	);
});

test("message box schemas expose only native dialog options", () => {
	const messageBoxSchema = JSON.parse(read(join(schemasRoot, "nodes", "action-message-box.schema.json")));
	const buttons = messageBoxSchema.$defs.config.properties.buttons.enum;
	const types = messageBoxSchema.$defs.config.properties.type.enum;

	assert.deepEqual([...buttons].sort(), ["ok", "ok_cancel", "yes_no", "yes_no_cancel"].sort());
	assert.deepEqual([...types].sort(), ["error", "info", "warning"].sort());
});

test("generated node schemas restrict config keys to editor-owned node config fields", () => {
	const definitionsSource = readDefinitions();
	const nodeSchemas = readJsonFiles(join(schemasRoot, "nodes")).map((filePath) => JSON.parse(read(filePath)));
	const allowedConfigKeys = new Set(nodeSchemas.flatMap((schema) => Object.keys(schema.$defs.config.properties ?? {})));
	const definitionConfigKeys = [
		...definitionsSource.matchAll(/\{\s*key:\s*"([^"]+)"\s*,\s*label:/g),
		...definitionsSource.matchAll(/defaultConfig:\s*\(\)\s*=>\s*\(\{([\s\S]*?)\}\)/g),
	]
		.flatMap((match) => (match[1].includes(":") ? [...match[1].matchAll(/^\s*([A-Za-z][A-Za-z0-9]*)\s*:/gm)] : [match]))
		.map((match) => match[1])
		.filter((key) => key && !["label", "value"].includes(key));

	assert.ok(
		nodeSchemas.every((schema) => schema.$defs.config.additionalProperties === false),
		"node config schemas must reject unknown config fields",
	);
	for (const key of new Set(["customName", ...definitionConfigKeys])) {
		assert.ok(allowedConfigKeys.has(key), `${key} is missing from generated node config schemas`);
	}
});

test("permission and capability schemas match canonical package contract", () => {
	const contractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const permissionsSchema = JSON.parse(read(join(schemasRoot, "permissions.schema.json")));
	const capabilitiesSchema = JSON.parse(read(join(schemasRoot, "capabilities.schema.json")));
	const canonicalPermissions = extractConstStringArray(contractSource, "canonicalPermissions").sort();
	const canonicalCapabilities = extractConstStringArray(contractSource, "canonicalCapabilities").sort();

	assert.deepEqual(
		[...permissionsSchema.properties.declared_permissions.items.enum].sort(),
		canonicalPermissions,
		"permissions.schema.json must match canonicalPermissions",
	);
	assert.deepEqual(
		[...capabilitiesSchema.properties.required_capabilities.items.enum].sort(),
		canonicalCapabilities,
		"capabilities.schema.json must match canonicalCapabilities",
	);
});

test("node definitions use only canonical permissions and capabilities", () => {
	const definitionsSource = readDefinitions();
	const sharedSource = read(join(appRoot, "data", "nodes", "definitions", "shared.ts"));
	const contractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const canonicalPermissions = new Set(extractConstStringArray(contractSource, "canonicalPermissions"));
	const canonicalCapabilities = new Set(extractConstStringArray(contractSource, "canonicalCapabilities"));
	const definitionPermissions = [
		...new Set([...definitionsSource.matchAll(/permission:\s*\{\s*name:\s*"([^"]+)"/g)].map((match) => match[1])),
	].sort();
	const definitionCapabilities = [...new Set(extractDefinitionCapabilities(definitionsSource, sharedSource))].sort();

	for (const permission of definitionPermissions) {
		assert.ok(canonicalPermissions.has(permission), `${permission} is missing from canonicalPermissions`);
	}

	for (const capability of definitionCapabilities) {
		assert.ok(canonicalCapabilities.has(capability), `${capability} is missing from canonicalCapabilities`);
	}
});

test("package contract validates graph structure and import rejects malformed edges", () => {
	const contractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));
	const registrySource = read(join(appRoot, "data", "nodes", "registry.ts"));

	assert.match(contractSource, /validateNodeConfig/);
	assert.match(contractSource, /validateProgramGraphContract/);
	assert.match(contractSource, /entry\.triggers must contain at least one trigger/);
	assert.match(contractSource, /references missing source node/);
	assert.match(contractSource, /unknown source_handle/);
	assert.match(packageSource, /Program edge .* references an unknown source or target node/);
	assert.match(registrySource, /Invalid value for \$\{field\.key\}: expected string/);
	assert.match(registrySource, /isValidNumberConfigValue/);
	assert.match(registrySource, /Invalid value for \$\{field\.key\}: expected boolean/);
	assert.match(contractSource, /getTargetRuntimeCompatibilityErrors/);
	assert.match(contractSource, /capabilities\.target_runtime/);
	assert.equal(/return \[\];\s*\n\s*}\);\s*\n}/.test(packageSource), false, "import must not silently drop edges");
});

test("file permissions are derived from node config paths", () => {
	const analysisSource = read(join(appRoot, "utils", "analysis.ts"));
	const contractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const filePolicySource = read(join(appRoot, "data", "project", "file-permissions.ts"));
	const readFileSource = read(join(appRoot, "data", "nodes", "definitions", "actions", "file-read.ts"));
	const writeFileSource = read(join(appRoot, "data", "nodes", "definitions", "actions", "file-write.ts"));
	const copyFileSource = read(join(appRoot, "data", "nodes", "definitions", "actions", "file-copy.ts"));

	assert.match(analysisSource, /getNodePermissions\(node\.data\.actionType, node\.data\.config\)/);
	assert.match(contractSource, /getNodePermissions\(actionType, config\)/);
	assert.match(filePolicySource, /read_sensitive_file/);
	assert.match(filePolicySource, /write_any_file/);
	assert.match(filePolicySource, /pathUsesRuntimeData/);
	assert.match(readFileSource, /permissionPathRules: \[\{ access: "read", configKey: "path" \}\]/);
	assert.match(writeFileSource, /permissionPathRules: \[\{ access: "write", configKey: "path" \}\]/);
	assert.match(copyFileSource, /\{ access: "read", configKey: "sourcePath" \}/);
	assert.match(copyFileSource, /\{ access: "write", configKey: "destinationPath" \}/);
});

test("editor and runner share bounded package ingestion limits", () => {
	const assetsSource = read(join(appRoot, "data", "project", "assets.ts"));
	const limitsSource = read(join(appRoot, "data", "project", "package-limits.ts"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));
	const runnerLimitsSource = read(join(repoRoot, "crates", "baudbound-script", "src", "package", "limits.rs"));
	const limits = JSON.parse(read(join(schemasRoot, "package-limits.json")));

	assert.equal(limits.version, 1);
	assert.ok(limits.max_entry_count > 0);
	assert.ok(limits.max_asset_bytes > 0);
	assert.ok(limits.max_total_uncompressed_bytes >= limits.max_asset_bytes);
	assert.match(limitsSource, /schemas\/package-limits\.json/);
	assert.match(assetsSource, /packageLimits\.max_asset_bytes/);
	assert.match(packageSource, /assertZipWithinPackageLimits/);
	assert.match(runnerLimitsSource, /schemas\/package-limits\.json/);
});

test("package asset validation requires zip assets and manifest assets to match exactly", () => {
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));

	assert.match(packageSource, /function collectPackageAssetManifest/);
	assert.match(packageSource, /validatePackageAssetEntries\(getZipAssetEntries\(zip\)\)/);
	assert.match(packageSource, /asset file is not declared in manifest\.json assets/);
	assert.match(packageSource, /is listed in manifest but missing from zip/);
	assert.match(packageSource, /duplicate manifest asset path/);
	assert.match(packageSource, /manifest asset .* must define path/);
});

test("export does not create hidden implicit triggers", () => {
	const analysisSource = read(join(appRoot, "utils", "analysis.ts"));
	const verificationSource = read(join(appRoot, "utils", "verification.ts"));

	assert.equal(analysisSource.includes("implicit-manual-trigger"), false);
	assert.match(analysisSource, /Cannot export a script without at least one trigger node/);
	assert.match(verificationSource, /No trigger node found\. Add at least one trigger before export\./);
});

test("node-specific verification is owned by node definitions", () => {
	const verificationSource = read(join(appRoot, "utils", "verification.ts"));
	const definitionsSource = readDefinitions();

	assert.equal(/switch\s*\(\s*node\.data\.actionType\s*\)/.test(verificationSource), false);
	assert.match(verificationSource, /validateNodeGraph/);
	assert.match(definitionsSource, /validateConfig:/);
	assert.match(definitionsSource, /validateGraph:/);
});

test("loop control bodies do not require return edges", () => {
	const sharedSource = read(join(appRoot, "data", "nodes", "definitions", "shared.ts"));
	const loopSource = read(join(appRoot, "data", "nodes", "definitions", "control", "loop.ts"));
	const whileSource = read(join(appRoot, "data", "nodes", "definitions", "control", "while.ts"));
	const forEachSource = read(join(appRoot, "data", "nodes", "definitions", "control", "for-each.ts"));
	const inspectorSource = read(join(appRoot, "components", "inspector", "inspector.tsx"));
	const helpSource = read(join(appRoot, "components", "modals", "help-modal.tsx"));

	for (const source of [sharedSource, loopSource, whileSource, forEachSource, inspectorSource, helpSource]) {
		assert.equal(/eventually return|flow back to the loop input|must connect its loop output back/.test(source), false);
	}

	assert.match(sharedSource, /validateLoopBodyDoesNotReturn/);
	assert.match(loopSource, /validateLoopBodyDoesNotReturn\(node\.id,\s*context\.edges,\s*"loop"\)/);
	assert.match(whileSource, /validateLoopBodyDoesNotReturn\(node\.id,\s*context\.edges,\s*"loop"\)/);
	assert.match(forEachSource, /validateLoopBodyDoesNotReturn\(node\.id,\s*context\.edges,\s*"loop"\)/);
	assert.match(inspectorSource, /do not connect it\s+back to the loop input/);
	assert.match(helpSource, /body branch should end naturally/);
});

function readDefinitions() {
	return readAll(join(appRoot, "data", "nodes", "definitions"));
}

function readAll(directory) {
	return readdirSync(directory, { withFileTypes: true })
		.map((entry) => {
			const path = join(directory, entry.name);
			return entry.isDirectory() ? readAll(path) : entry.name.endsWith(".ts") ? read(path) : "";
		})
		.join("\n");
}

function read(path) {
	return readFileSync(path, "utf8");
}

function readJsonFiles(directory) {
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) {
			return readJsonFiles(path);
		}

		return entry.name.endsWith(".json") ? [path] : [];
	});
}

function getDefinitionBlock(source, actionType) {
	const actionTypeIndex = source.indexOf(`actionType: "${actionType}"`);
	assert.notEqual(actionTypeIndex, -1, `${actionType} definition block should exist`);
	const start = source.lastIndexOf("defineNode({", actionTypeIndex);
	const next = source.indexOf("defineNode({", actionTypeIndex + actionType.length);
	const end = next === -1 ? source.length : next;

	assert.notEqual(start, -1, `${actionType} should be inside defineNode`);
	return source.slice(start, end);
}

function extractConstStringArray(source, constName) {
	const match = source.match(new RegExp(`export const ${constName} = \\[([\\s\\S]*?)\\] as const;`));
	assert.ok(match, `${constName} must be exported as a const string array`);
	return [...match[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
}

test("secret declarations are package metadata while simulation values remain session-only", () => {
	const manifestSchema = JSON.parse(read(join(schemasRoot, "manifest.schema.json")));
	const variableSchema = JSON.parse(read(join(schemasRoot, "nodes", "runtime-set-variable.schema.json")));
	const permissionsSchema = JSON.parse(read(join(schemasRoot, "permissions.schema.json")));
	const editorPage = read(join(appRoot, "app", "editor-page.tsx"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));
	const simulationSource = read(join(appRoot, "utils", "simulation.ts"));

	assert.ok(manifestSchema.properties.secrets, "manifest must declare secret references");
	assert.deepEqual(variableSchema.$defs.config.properties.scope.enum, ["runtime", "persistent", "global"]);
	assert.ok(permissionsSchema.properties.declared_permissions.items.enum.includes("read_secret"));
	assert.match(editorPage, /simulationSecretValues/);
	assert.match(packageSource, /secretDeclarations/);
	assert.doesNotMatch(packageSource, /simulationSecretValues/);
	assert.match(simulationSource, /redactSecretText/);
	assert.match(simulationSource, /redactSnapshotValue/);
	assert.match(simulationSource, /\[REDACTED\]/);
});

function extractRustConstStringArray(source, constName) {
	const match = source.match(new RegExp(`(?:pub\\s+)?const ${constName}: &\\[&str\\] = &\\[([\\s\\S]*?)\\];`));
	assert.ok(match, `${constName} must be declared as a Rust string array`);
	return [...match[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
}

function extractDefinitionCapabilities(definitionsSource, sharedSource) {
	const sharedCapabilities = new Map(
		[...sharedSource.matchAll(/export const (\w+) = \[([^\]]*)\]/g)].map((match) => [
			match[1],
			[...match[2].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]),
		]),
	);
	const capabilities = [];

	for (const match of definitionsSource.matchAll(/capabilities:\s*(\[[^\]]*\]|\w+)/g)) {
		const value = match[1];
		if (value.startsWith("[")) {
			capabilities.push(...[...value.matchAll(/"([^"]+)"/g)].map((entry) => entry[1]));
			continue;
		}

		capabilities.push(...(sharedCapabilities.get(value) ?? []));
	}

	return capabilities;
}

function extractWindowsDesktopOnlyActionTypes(definitionsSource) {
	return extractDefinitionActionTypes(definitionsSource).filter((actionType) =>
		/supportedTargetRuntimes:\s*\[\s*"Windows Desktop"\s*\]/.test(getDefinitionBlock(definitionsSource, actionType)),
	);
}

function extractDesktopOnlyActionTypes(definitionsSource) {
	return extractDefinitionActionTypes(definitionsSource).filter((actionType) => {
		const definitionBlock = getDefinitionBlock(definitionsSource, actionType);
		return (
			/desktopOnly:\s*true/.test(definitionBlock) &&
			!/supportedTargetRuntimes:\s*\[\s*"Windows Desktop"\s*\]/.test(definitionBlock)
		);
	});
}

function extractDefinitionActionTypes(definitionsSource) {
	return [...definitionsSource.matchAll(/actionType:\s*"([^"]+)"/g)].map((match) => match[1]);
}

function uniqueSorted(values) {
	return [...new Set(values)].sort();
}

function escapeRegExp(value) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
