import type { EditorProject } from "./model";
import { projectContentSignature } from "./serialization";

export function getProjectHistoryCoalesceKey(previous: EditorProject, next: EditorProject) {
	if (sameProjectWithout(previous, next, "nodes")) {
		return getNodeChangeKey(previous, next);
	}
	if (sameProjectWithout(previous, next, "comments")) {
		return getCommentChangeKey(previous, next);
	}
	return null;
}

function getNodeChangeKey(previous: EditorProject, next: EditorProject) {
	if (previous.nodes.length !== next.nodes.length) return null;
	const previousById = new Map(previous.nodes.map((node) => [node.id, node]));
	const changed = next.nodes.filter((node) => {
		const before = previousById.get(node.id);
		return !before || JSON.stringify(before) !== JSON.stringify(node);
	});
	if (changed.length === 0 || changed.some((node) => !previousById.has(node.id))) return null;

	const configChanges = changed.filter((node) => {
		const before = previousById.get(node.id);
		return before && JSON.stringify(before.data.config) !== JSON.stringify(node.data.config);
	});
	const positionOnly = changed.every((node) => {
		const before = previousById.get(node.id);
		return before && JSON.stringify(before.data) === JSON.stringify(node.data);
	});

	if (positionOnly)
		return `node-position:${changed
			.map((node) => node.id)
			.sort()
			.join(",")}`;
	if (changed.length === 1 && configChanges.length === 1) return `node-config:${changed[0]?.id}`;
	return null;
}

function getCommentChangeKey(previous: EditorProject, next: EditorProject) {
	if (previous.comments.length !== next.comments.length) return null;
	const previousById = new Map(previous.comments.map((comment) => [comment.id, comment]));
	const changed = next.comments.filter((comment) => {
		const before = previousById.get(comment.id);
		return !before || JSON.stringify(before) !== JSON.stringify(comment);
	});
	if (changed.length !== 1 || !previousById.has(changed[0]?.id ?? "")) return null;
	return `comment:${changed[0]?.id}`;
}

function sameProjectWithout(previous: EditorProject, next: EditorProject, field: "comments" | "nodes") {
	return projectContentSignature({ ...previous, [field]: [] }) === projectContentSignature({ ...next, [field]: [] });
}
