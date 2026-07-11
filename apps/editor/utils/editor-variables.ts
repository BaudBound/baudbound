import type { Node } from "@xyflow/react";
import { getBuiltInVariableRuntimeEntries } from "@/data/project/built-in-variables";
import {
	createConfiguredVariableDefinitions,
	createDerivedVariableMetadataDefinitions,
	createNodeOutputVariables,
	type EditorVariable,
} from "@/data/project/variables";
import type { ProjectSettings, ScriptNodeData, SecretDeclaration, SimulationVariableSnapshot } from "@/lib/types";

export function createVariablePanelEntries(
	projectSettings: ProjectSettings,
	nodes: Node<ScriptNodeData>[],
	snapshots: SimulationVariableSnapshot[],
	secretDeclarations: SecretDeclaration[] = [],
): EditorVariable[] {
	return createEditorVariableRegistry(projectSettings, nodes, snapshots, secretDeclarations);
}

export function createEditorVariableRegistry(
	projectSettings: ProjectSettings,
	nodes: Node<ScriptNodeData>[],
	snapshots: SimulationVariableSnapshot[] = [],
	secretDeclarations: SecretDeclaration[] = [],
): EditorVariable[] {
	const variables = new Map<string, EditorVariable>();

	for (const variable of [
		...getBuiltInVariableRuntimeEntries(projectSettings),
		...createConfiguredVariableDefinitions(nodes),
		...createNodeOutputVariables(nodes),
	]) {
		variables.set(variable.name, variable);
	}
	for (const secret of secretDeclarations) {
		variables.set(secret.name, {
			description: secret.description,
			name: secret.name,
			read_only: true,
			scope: "secret",
			source: "secret",
			token: `{{${secret.name}}}`,
			type: secret.type,
		});
	}

	for (const snapshot of snapshots) {
		const existing = variables.get(snapshot.name);
		if (existing) {
			variables.set(snapshot.name, { ...existing, value: snapshot.value });
			continue;
		}

		variables.set(snapshot.name, {
			name: snapshot.name,
			read_only: snapshot.source !== "runtime",
			scope: snapshot.source === "node_output" ? "node_output" : snapshot.source === "secret" ? "secret" : "runtime",
			source: snapshot.source === "node_output" ? "node_output" : snapshot.source === "secret" ? "secret" : "user",
			token: `{{${snapshot.name}}}`,
			type: inferVariableType(snapshot.value),
			value: snapshot.value,
		});
	}

	const baseVariables = [...variables.values()];
	for (const variable of createDerivedVariableMetadataDefinitions(
		baseVariables.filter((variable) => variable.source !== "secret"),
	)) {
		variables.set(variable.name, variable);
	}

	return [...variables.values()].sort((a, b) => {
		const sourceOrder = getVariableSourceOrder(a) - getVariableSourceOrder(b);
		return sourceOrder || a.name.localeCompare(b.name);
	});
}

function getVariableSourceOrder(variable: EditorVariable) {
	if (variable.source === "built_in") {
		return variable.scope === "manifest" ? 0 : 1;
	}

	if (variable.source === "user") {
		return 2;
	}
	if (variable.source === "secret") {
		return 3;
	}

	return 4;
}

function inferVariableType(value: SimulationVariableSnapshot["value"]): EditorVariable["type"] {
	if (typeof value === "number") {
		return "number";
	}

	if (typeof value === "boolean") {
		return "boolean";
	}

	if (Array.isArray(value)) {
		return "list";
	}

	if (value && typeof value === "object") {
		return "object";
	}

	return "string";
}
