import assert from "node:assert/strict";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const appRoot = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = join(appRoot, "..", "..");
const schemasRoot = join(repoRoot, "schemas");
const nodeSchemasRoot = join(schemasRoot, "nodes");
const runnerCapabilityContractPath = join(
	repoRoot,
	"crates",
	"baudbound-security",
	"contracts",
	"node-capabilities.json",
);
const publicSchemaRoot = "https://schemas.baudbound.app";
const programSchemaUrl = `${publicSchemaRoot}/program.schema.json`;
const jsonValueRef = `${programSchemaUrl}#/$defs/jsonValue`;
const runtimeOutputRef = `${programSchemaUrl}#/$defs/runtimeOutput`;
const checkMode = process.argv.includes("--check");

const optionValues = createOptionValueMap();
const sharedCapabilityValues = createSharedCapabilityValueMap();
const definitions = readNodeDefinitions().sort((a, b) => a.actionType.localeCompare(b.actionType));
const generatedNodeSchemas = Object.fromEntries(
	definitions.map((definition) => [getNodeSchemaFileName(definition.actionType), createNodeSchema(definition)]),
);
const generatedProgramSchema = createProgramSchema(
	JSON.parse(readFileSync(join(schemasRoot, "program.schema.json"), "utf8")),
);
const generatedRunnerCapabilityContract = createRunnerCapabilityContract();

if (checkMode) {
	assertGeneratedFile(join(schemasRoot, "program.schema.json"), generatedProgramSchema);
	assertGeneratedFile(runnerCapabilityContractPath, generatedRunnerCapabilityContract);
	for (const [fileName, schema] of Object.entries(generatedNodeSchemas)) {
		assertGeneratedFile(join(nodeSchemasRoot, fileName), schema);
	}
	const expectedFiles = new Set(Object.keys(generatedNodeSchemas));
	const actualFiles = existsSync(nodeSchemasRoot)
		? readdirSync(nodeSchemasRoot).filter((fileName) => fileName.endsWith(".schema.json"))
		: [];
	for (const fileName of actualFiles) {
		assert.ok(expectedFiles.has(fileName), `Unexpected generated node schema: ${fileName}`);
	}
	process.exit(0);
}

mkdirSync(nodeSchemasRoot, { recursive: true });
for (const fileName of readdirSync(nodeSchemasRoot)) {
	if (fileName.endsWith(".schema.json")) {
		rmSync(join(nodeSchemasRoot, fileName));
	}
}
writeJson(join(schemasRoot, "program.schema.json"), generatedProgramSchema);
mkdirSync(dirname(runnerCapabilityContractPath), { recursive: true });
writeJson(runnerCapabilityContractPath, generatedRunnerCapabilityContract);
for (const [fileName, schema] of Object.entries(generatedNodeSchemas)) {
	writeJson(join(nodeSchemasRoot, fileName), schema);
}

function createRunnerCapabilityContract() {
	return {
		version: 1,
		nodes: Object.fromEntries(definitions.map((definition) => [definition.actionType, definition.capabilities])),
	};
}

function createProgramSchema(programSchema) {
	const triggerDefinitions = definitions.filter((definition) => definition.kind === "trigger");
	const controlDefinitions = definitions.filter((definition) => definition.kind === "control");
	const executableActionDefinitions = definitions.filter(
		(definition) => definition.kind === "action" && definition.actionType !== "runtime.set_variable",
	);
	const triggerRefs = triggerDefinitions.map((definition) => schemaRef(definition.actionType));
	const controlRefs = controlDefinitions.map((definition) => schemaRef(definition.actionType));
	const variableOperationRefs = definitions
		.filter((definition) => definition.actionType === "runtime.set_variable")
		.map((definition) => schemaRef(definition.actionType));
	const actionRefs = executableActionDefinitions.map((definition) => schemaRef(definition.actionType));

	return {
		...programSchema,
		$defs: {
			...programSchema.$defs,
			actionType: stringEnum(definitions.map((definition) => definition.actionType)),
			actionRunnerType: stringEnum(executableActionDefinitions.map((definition) => definition.runnerType)),
			controlActionType: stringEnum(controlDefinitions.map((definition) => definition.actionType)),
			controlStepType: stringEnum(controlDefinitions.map((definition) => definition.controlType)),
			executableActionType: stringEnum(executableActionDefinitions.map((definition) => definition.actionType)),
			triggerActionType: stringEnum(triggerDefinitions.map((definition) => definition.actionType)),
			triggerType: stringEnum(triggerDefinitions.map((definition) => definition.runnerType)),
			trigger: { oneOf: triggerRefs },
			controlStep: { oneOf: controlRefs },
			variableOperationStep: { oneOf: variableOperationRefs },
			actionStep: { oneOf: actionRefs },
			step: {
				oneOf: [
					{ $ref: "#/$defs/controlStep" },
					{ $ref: "#/$defs/variableOperationStep" },
					{ $ref: "#/$defs/actionStep" },
				],
			},
		},
	};
}

function stringEnum(values) {
	return {
		type: "string",
		enum: [...new Set(values.filter(Boolean))],
	};
}

function createNodeSchema(definition) {
	const configSchema = createConfigSchema(definition);
	const required = ["id", "action_type", "type", "config", "runtime_outputs"];
	const properties = {
		id: { type: "string", minLength: 1 },
		action_type: { const: definition.actionType },
		type: { const: getProgramNodeType(definition) },
		config: { $ref: "#/$defs/config" },
		runtime_outputs: {
			type: "array",
			items: { $ref: runtimeOutputRef },
		},
	};

	if (definition.kind === "action" && definition.actionType !== "runtime.set_variable") {
		required.splice(3, 0, "action");
		properties.action = { const: definition.runnerType };
	}

	return {
		$schema: "https://json-schema.org/draft/2020-12/schema",
		$id: `${publicSchemaRoot}/nodes/${getNodeSchemaFileName(definition.actionType)}`,
		title: `BaudBound Node - ${definition.label}`,
		type: "object",
		additionalProperties: false,
		required,
		properties,
		$defs: {
			config: configSchema,
		},
	};
}

function createConfigSchema(definition) {
	const fieldsByKey = new Map(definition.configFields.map((field) => [field.key, field]));
	const keys = new Set(["customName", ...definition.defaultConfigKeys, ...fieldsByKey.keys()]);
	const properties = {};
	const required = [];

	for (const key of [...keys].sort()) {
		if (key === "customName") {
			properties[key] = { type: "string" };
			continue;
		}

		const field = fieldsByKey.get(key);
		properties[key] = field ? createConfigFieldSchema(field) : { $ref: jsonValueRef };
		if (field && field.required !== false) {
			required.push(key);
		}
	}

	return {
		type: "object",
		additionalProperties: false,
		...(required.length > 0 ? { required: required.sort() } : {}),
		properties,
	};
}

function createConfigFieldSchema(field) {
	if (field.type === "text" || field.type === "textarea") {
		return { type: "string" };
	}

	if (field.type === "switch") {
		return { type: "boolean" };
	}

	if (field.type === "select") {
		const schema = { type: "string" };
		if (field.options.length > 0) {
			schema.enum = field.options;
		}
		return schema;
	}

	const numberString = field.usesVariables
		? {
				type: "string",
				minLength: 1,
			}
		: {
				type: "string",
				pattern: "^-?(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?$",
			};

	return {
		anyOf: [{ type: "number" }, numberString],
	};
}

function getProgramNodeType(definition) {
	if (definition.kind === "trigger" || definition.kind === "control") {
		return definition.runnerType ?? definition.controlType;
	}

	return definition.actionType === "runtime.set_variable" ? "set_variable" : "action";
}

function readNodeDefinitions() {
	const definitionsRoot = join(appRoot, "data", "nodes", "definitions");
	const files = collectFiles(definitionsRoot).filter((filePath) => filePath.endsWith(".ts"));
	const definitions = [];

	for (const filePath of files) {
		const sourceFile = ts.createSourceFile(filePath, readFileSync(filePath, "utf8"), ts.ScriptTarget.Latest, true);
		visit(sourceFile, (node) => {
			if (!isDefineNodeCall(node)) {
				return;
			}

			const object = node.arguments[0];
			if (!ts.isObjectLiteralExpression(object)) {
				return;
			}

			definitions.push(readDefinitionObject(object, filePath));
		});
	}

	return definitions;
}

function readDefinitionObject(object, filePath) {
	const actionType = getRequiredStringProperty(object, "actionType", filePath);
	const kind = getRequiredStringProperty(object, "kind", filePath);
	const label = getRequiredStringProperty(object, "label", filePath);
	const runnerType = getOptionalStringProperty(object, "runnerType");
	const controlType = getOptionalStringProperty(object, "controlType");
	const capabilities = readCapabilities(getPropertyInitializer(object, "capabilities"), actionType);
	const configFields = readConfigFields(getPropertyInitializer(object, "configFields"), actionType);
	const defaultConfigKeys = readDefaultConfigKeys(getPropertyInitializer(object, "defaultConfig"));

	return {
		actionType,
		capabilities,
		configFields,
		controlType,
		defaultConfigKeys,
		kind,
		label,
		runnerType,
		source: relative(appRoot, filePath).split(sep).join("/"),
	};
}

function readCapabilities(initializer, actionType) {
	if (ts.isArrayLiteralExpression(initializer)) {
		const capabilities = readStringArrayValues(initializer);
		if (capabilities.length > 0) {
			return [...new Set(capabilities)].sort();
		}
	}

	if (ts.isIdentifier(initializer)) {
		const capabilities = sharedCapabilityValues.get(initializer.text);
		if (capabilities) {
			return capabilities;
		}
	}

	throw new Error(`${actionType} capabilities must be a non-empty string array or a shared capability constant.`);
}

function createSharedCapabilityValueMap() {
	const source = ts.createSourceFile(
		"shared.ts",
		readFileSync(join(appRoot, "data", "nodes", "definitions", "shared.ts"), "utf8"),
		ts.ScriptTarget.Latest,
		true,
	);
	const values = new Map();

	for (const statement of source.statements) {
		if (!ts.isVariableStatement(statement)) {
			continue;
		}
		for (const declaration of statement.declarationList.declarations) {
			if (!ts.isIdentifier(declaration.name) || !declaration.initializer) {
				continue;
			}
			const capabilities = readStringArrayValues(unwrapAsConst(declaration.initializer));
			if (capabilities.length > 0) {
				values.set(declaration.name.text, [...new Set(capabilities)].sort());
			}
		}
	}

	return values;
}

function readConfigFields(initializer, actionType) {
	if (!initializer) {
		return [];
	}

	if (!ts.isArrayLiteralExpression(initializer)) {
		throw new Error(`${actionType} configFields must be an array literal for schema generation.`);
	}

	return initializer.elements.map((element) => {
		if (!ts.isObjectLiteralExpression(element)) {
			throw new Error(`${actionType} configFields entries must be object literals.`);
		}

		const key = getRequiredStringProperty(element, "key", actionType);
		const type = getRequiredStringProperty(element, "type", actionType);
		return {
			key,
			options: readFieldOptions(getPropertyInitializer(element, "options")),
			required: getOptionalBooleanProperty(element, "required"),
			type,
			usesVariables: getOptionalBooleanProperty(element, "usesVariables") === true,
		};
	});
}

function readFieldOptions(initializer) {
	if (!initializer) {
		return [];
	}

	if (ts.isIdentifier(initializer)) {
		return optionValues.get(initializer.text) ?? [];
	}

	if (ts.isArrayLiteralExpression(initializer)) {
		return readSelectOptionValues(initializer);
	}

	return [];
}

function readDefaultConfigKeys(initializer) {
	if (!initializer || !ts.isArrowFunction(initializer)) {
		return [];
	}

	const body = unwrapParenthesized(initializer.body);
	if (!ts.isObjectLiteralExpression(body)) {
		return [];
	}

	return body.properties
		.filter(ts.isPropertyAssignment)
		.map((property) => getPropertyName(property.name))
		.filter(Boolean);
}

function createOptionValueMap() {
	const optionsSource = ts.createSourceFile(
		"options.ts",
		readFileSync(join(appRoot, "data", "nodes", "definitions", "options.ts"), "utf8"),
		ts.ScriptTarget.Latest,
		true,
	);
	const serialSource = ts.createSourceFile(
		"serial.ts",
		readFileSync(join(appRoot, "data", "project", "serial.ts"), "utf8"),
		ts.ScriptTarget.Latest,
		true,
	);
	const variablesSource = ts.createSourceFile(
		"variables.ts",
		readFileSync(join(appRoot, "data", "project", "variables.ts"), "utf8"),
		ts.ScriptTarget.Latest,
		true,
	);
	const values = new Map();

	for (const sourceFile of [variablesSource, serialSource, optionsSource]) {
		for (const statement of sourceFile.statements) {
			if (!ts.isVariableStatement(statement)) {
				continue;
			}

			for (const declaration of statement.declarationList.declarations) {
				if (!ts.isIdentifier(declaration.name) || !declaration.initializer) {
					continue;
				}

				const name = declaration.name.text;
				const initializer = unwrapAsConst(declaration.initializer);
				const literalValues = readStringArrayValues(initializer);
				if (literalValues.length > 0) {
					values.set(name, literalValues);
					continue;
				}

				const optionList = readOptionInitializerValues(initializer, values);
				if (optionList.length > 0) {
					values.set(name, optionList);
				}
			}
		}
	}

	return values;
}

function readOptionInitializerValues(initializer, values) {
	if (ts.isArrayLiteralExpression(initializer)) {
		return readSelectOptionValues(initializer, values);
	}

	if (ts.isCallExpression(initializer) && ts.isPropertyAccessExpression(initializer.expression)) {
		const expression = initializer.expression;
		if (expression.name.text !== "map") {
			return [];
		}

		if (ts.isIdentifier(expression.expression)) {
			return values.get(expression.expression.text) ?? [];
		}

		return readStringArrayValues(expression.expression);
	}

	return [];
}

function readSelectOptionValues(array, values = new Map()) {
	const output = [];

	for (const element of array.elements) {
		if (ts.isSpreadElement(element) && ts.isIdentifier(element.expression)) {
			output.push(...(values.get(element.expression.text) ?? []));
			continue;
		}

		if (ts.isObjectLiteralExpression(element)) {
			const value = getOptionalStringProperty(element, "value");
			if (value) {
				output.push(value);
			}
		}
	}

	return [...new Set(output)];
}

function readStringArrayValues(initializer) {
	if (!ts.isArrayLiteralExpression(initializer)) {
		return [];
	}

	const values = [];
	for (const element of initializer.elements) {
		if (ts.isStringLiteral(element) || ts.isNumericLiteral(element)) {
			values.push(element.text);
		}
	}
	return values;
}

function isDefineNodeCall(node) {
	return (
		ts.isCallExpression(node) &&
		ts.isIdentifier(node.expression) &&
		node.expression.text === "defineNode" &&
		node.arguments.length > 0
	);
}

function getRequiredStringProperty(object, name, label) {
	const value = getOptionalStringProperty(object, name);
	if (!value) {
		throw new Error(`${label} is missing required string property ${name}.`);
	}
	return value;
}

function getOptionalStringProperty(object, name) {
	const initializer = getPropertyInitializer(object, name);
	return initializer && ts.isStringLiteral(initializer) ? initializer.text : undefined;
}

function getOptionalBooleanProperty(object, name) {
	const initializer = getPropertyInitializer(object, name);
	if (!initializer) {
		return undefined;
	}
	if (initializer.kind === ts.SyntaxKind.TrueKeyword) {
		return true;
	}
	if (initializer.kind === ts.SyntaxKind.FalseKeyword) {
		return false;
	}
	return undefined;
}

function getPropertyInitializer(object, name) {
	const property = object.properties.find(
		(entry) => ts.isPropertyAssignment(entry) && getPropertyName(entry.name) === name,
	);
	return property && ts.isPropertyAssignment(property) ? unwrapParenthesized(property.initializer) : undefined;
}

function getPropertyName(name) {
	if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
		return name.text;
	}
	return undefined;
}

function unwrapAsConst(node) {
	if (ts.isAsExpression(node)) {
		return unwrapAsConst(node.expression);
	}
	return unwrapParenthesized(node);
}

function unwrapParenthesized(node) {
	return ts.isParenthesizedExpression(node) ? unwrapParenthesized(node.expression) : node;
}

function visit(node, callback) {
	callback(node);
	node.forEachChild((child) => visit(child, callback));
}

function collectFiles(directory) {
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		return entry.isDirectory() ? collectFiles(path) : [path];
	});
}

function getNodeSchemaFileName(actionType) {
	return `${actionType.replaceAll(".", "-").replaceAll("_", "-")}.schema.json`;
}

function schemaRef(actionType) {
	return { $ref: `${publicSchemaRoot}/nodes/${getNodeSchemaFileName(actionType)}` };
}

function assertGeneratedFile(filePath, expected) {
	assert.ok(existsSync(filePath), `${filePath} is missing. Run pnpm schemas:generate.`);
	const actual = readFileSync(filePath, "utf8");
	const generated = toJson(expected);
	assert.equal(actual, generated, `${filePath} is stale. Run pnpm schemas:generate.`);
}

function writeJson(filePath, value) {
	writeFileSync(filePath, toJson(value));
}

function toJson(value) {
	return `${JSON.stringify(value, null, "\t")}\n`;
}
