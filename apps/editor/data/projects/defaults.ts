import { defaultEditorEdgeStyle } from "@/data/editor/flow-canvas";
import type { ProjectSettings } from "@/lib/types";
import { DEFAULT_MINIMUM_RUNNER_VERSION } from "@/lib/version";
import type { EditorProject, ProjectIdentity } from "./model";

export const defaultProjectSettings: ProjectSettings = {
	name: "untitled-script",
	description: "",
	author: "",
	website: "",
	repository: "",
	tags: [],
	targetRuntime: "Generic Desktop",
	minimumRunnerVersion: DEFAULT_MINIMUM_RUNNER_VERSION,
};

export function createProjectIdentity(now = new Date()): ProjectIdentity {
	return {
		id: crypto.randomUUID(),
		createdAt: now.toISOString(),
	};
}

export function createEmptyEditorProject(settings: ProjectSettings, now = new Date()): EditorProject {
	const identity = createProjectIdentity(now);

	return {
		assets: [],
		comments: [],
		defaultVariables: [],
		edgeStyle: defaultEditorEdgeStyle,
		edges: [],
		identity,
		nodes: [],
		revision: 1,
		schemaVersion: 1,
		secretDeclarations: [],
		settings,
		updatedAt: identity.createdAt,
	};
}
