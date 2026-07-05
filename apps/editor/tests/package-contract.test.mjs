import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const appRoot = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = join(appRoot, "..", "..");
const schemasRoot = join(repoRoot, "schemas");

test("schemas are valid JSON", () => {
	for (const fileName of readdirSync(schemasRoot)) {
		if (!fileName.endsWith(".json")) {
			continue;
		}

		assert.doesNotThrow(() => JSON.parse(read(join(schemasRoot, fileName))), fileName);
	}
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

test("program schema restricts config keys to editor-owned node config fields", () => {
	const definitionsSource = readDefinitions();
	const programSchema = JSON.parse(read(join(schemasRoot, "program.schema.json")));
	const allowedConfigKeys = new Set(programSchema.$defs.config.propertyNames.enum);
	const definitionConfigKeys = [
		...definitionsSource.matchAll(/\{\s*key:\s*"([^"]+)"\s*,\s*label:/g),
		...definitionsSource.matchAll(/defaultConfig:\s*\(\)\s*=>\s*\(\{([\s\S]*?)\}\)/g),
	]
		.flatMap((match) => (match[1].includes(":") ? [...match[1].matchAll(/^\s*([A-Za-z][A-Za-z0-9]*)\s*:/gm)] : [match]))
		.map((match) => match[1])
		.filter((key) => key && !["label", "value"].includes(key));

	assert.ok(
		Array.isArray(programSchema.$defs.config.propertyNames.enum),
		"config schema must define propertyNames enum",
	);
	assert.equal(programSchema.$defs.config.additionalProperties.$ref, "#/$defs/jsonValue");
	for (const key of new Set(["customName", ...definitionConfigKeys])) {
		assert.ok(allowedConfigKeys.has(key), `${key} is missing from program.schema.json config propertyNames`);
	}
});

test("package contract validates graph structure and import rejects malformed edges", () => {
	const contractSource = read(join(appRoot, "utils", "package-contract.ts"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));

	assert.match(contractSource, /validateNodeConfig/);
	assert.match(contractSource, /validateProgramGraphContract/);
	assert.match(contractSource, /entry\.triggers must contain at least one trigger/);
	assert.match(contractSource, /references missing source node/);
	assert.match(contractSource, /unknown source_handle/);
	assert.match(packageSource, /Program edge .* references an unknown source or target node/);
	assert.equal(/return \[\];\s*\n\s*}\);\s*\n}/.test(packageSource), false, "import must not silently drop edges");
});

test("asset validation has no fixed package size or count cap", () => {
	const assetsSource = read(join(appRoot, "data", "project", "assets.ts"));
	const assetEditorSource = read(join(appRoot, "components", "modals", "asset-editor-modal.tsx"));
	const packageSource = read(join(appRoot, "utils", "bbs-package.ts"));

	assert.equal(/MAX_ASSET_(COUNT|SIZE|TOTAL)/.test(assetsSource), false);
	assert.equal(/MAX_ASSET_(COUNT|SIZE|TOTAL)/.test(packageSource), false);
	assert.match(assetEditorSource, /No fixed editor cap/);
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

test("example package fixtures contain required package files", () => {
	const examplesRoot = join(repoRoot, "examples");
	const requiredFiles = [
		"manifest.json",
		"program.json",
		"permissions.json",
		"capabilities.json",
		"editor.json",
		"README.md",
	];
	const exampleDirectories = readdirSync(examplesRoot, { withFileTypes: true })
		.filter((entry) => entry.isDirectory())
		.map((entry) => join(examplesRoot, entry.name));

	assert.ok(exampleDirectories.length > 0, "examples directory should contain package fixtures");
	for (const directory of exampleDirectories) {
		const files = new Set(readdirSync(directory));
		for (const requiredFile of requiredFiles) {
			assert.ok(files.has(requiredFile), `${directory} is missing ${requiredFile}`);
		}
		for (const file of [...files].filter((entry) => entry.endsWith(".json"))) {
			assert.doesNotThrow(() => JSON.parse(read(join(directory, file))), `${directory}/${file}`);
		}
	}
});

test("example package declarations match their program action types", () => {
	const examplesRoot = join(repoRoot, "examples");
	const permissionByActionType = new Map([
		["action.log", "log"],
		["action.http", "http_request"],
	]);
	const capabilityByActionType = new Map([
		["trigger.manual", "trigger.manual"],
		["trigger.schedule", "trigger.schedule"],
		["action.log", "action.log"],
		["action.http", "action.http"],
	]);

	for (const entry of readdirSync(examplesRoot, { withFileTypes: true })) {
		if (!entry.isDirectory()) {
			continue;
		}

		const directory = join(examplesRoot, entry.name);
		const program = JSON.parse(read(join(directory, "program.json")));
		const permissions = JSON.parse(read(join(directory, "permissions.json")));
		const capabilities = JSON.parse(read(join(directory, "capabilities.json")));
		const actionTypes = getProgramActionTypes(program);
		const expectedPermissions = [
			...new Set(actionTypes.map((type) => permissionByActionType.get(type)).filter(Boolean)),
		].sort();
		const expectedCapabilities = [
			...new Set(actionTypes.map((type) => capabilityByActionType.get(type)).filter(Boolean)),
		].sort();

		assert.deepEqual([...permissions.declared_permissions].sort(), expectedPermissions, `${entry.name} permissions`);
		assert.deepEqual(
			[...capabilities.required_capabilities].sort(),
			expectedCapabilities,
			`${entry.name} capabilities`,
		);
	}
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

function getProgramActionTypes(program) {
	const entry = program.entry ?? {};
	const triggers = Array.isArray(entry.triggers) ? entry.triggers : [];
	const steps = Array.isArray(entry.program?.steps) ? entry.program.steps : [];
	return [...triggers, ...steps].map((node) => node.action_type).filter(Boolean);
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

function escapeRegExp(value) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
