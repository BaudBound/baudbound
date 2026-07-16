import type { EditorProject, ProjectSummary } from "@/data/projects/model";
import { ProjectNotFoundError, ProjectRevisionConflictError } from "@/data/projects/model";
import {
	assetStorageFingerprint,
	hydrateProject,
	projectAssetKey,
	type StoredProjectAsset,
	type StoredProjectRecord,
	toProjectSummary,
	toStoredProject,
	toStoredProjectAsset,
} from "@/data/projects/serialization";
import {
	EditorStorageError,
	openEditorDatabase,
	projectAssetStoreName,
	projectStoreName,
	requestResult,
	toEditorStorageError,
	transactionComplete,
} from "./database";

export async function listProjects(): Promise<ProjectSummary[]> {
	const database = await openEditorDatabase();
	const transaction = database.transaction(projectStoreName, "readonly");
	const records = await requestResult(transaction.objectStore(projectStoreName).index("updatedAt").getAll());
	await transactionComplete(transaction);
	try {
		return records.reverse().map(toProjectSummary);
	} catch (error) {
		throw corruptProjectStorage(error);
	}
}

export async function getProject(projectId: string): Promise<EditorProject> {
	const database = await openEditorDatabase();
	const transaction = database.transaction([projectStoreName, projectAssetStoreName], "readonly");
	const record = await requestResult(transaction.objectStore(projectStoreName).get(projectId));
	if (!record) {
		transaction.abort();
		throw new ProjectNotFoundError(projectId);
	}
	const assets = await requestResult(
		transaction.objectStore(projectAssetStoreName).index("projectId").getAll(projectId),
	);
	await transactionComplete(transaction);
	try {
		return hydrateProject(record, assets);
	} catch (error) {
		throw corruptProjectStorage(error);
	}
}

export async function projectExists(projectId: string) {
	const database = await openEditorDatabase();
	const transaction = database.transaction(projectStoreName, "readonly");
	const key = await requestResult(transaction.objectStore(projectStoreName).getKey(projectId));
	await transactionComplete(transaction);
	return key !== undefined;
}

export async function createProject(project: EditorProject): Promise<EditorProject> {
	return writeProject(project, null);
}

export async function replaceProject(project: EditorProject): Promise<EditorProject> {
	const existing = await getProject(project.identity.id);
	return writeProject({ ...project, revision: existing.revision }, existing.revision);
}

export async function saveProject(project: EditorProject, expectedRevision: number): Promise<EditorProject> {
	return writeProject(project, expectedRevision);
}

export async function deleteProject(projectId: string) {
	const database = await openEditorDatabase();
	const transaction = database.transaction([projectStoreName, projectAssetStoreName], "readwrite");
	const projects = transaction.objectStore(projectStoreName);
	if ((await requestResult(projects.getKey(projectId))) === undefined) {
		transaction.abort();
		throw new ProjectNotFoundError(projectId);
	}
	projects.delete(projectId);
	const assetStore = transaction.objectStore(projectAssetStoreName);
	const cursorRequest = assetStore.index("projectId").openKeyCursor(IDBKeyRange.only(projectId));
	await iterateKeyCursor(cursorRequest, (cursor) => assetStore.delete(cursor.primaryKey));
	await transactionComplete(transaction);
}

export async function requestPersistentEditorStorage() {
	if (!navigator.storage?.persist || !navigator.storage.persisted) {
		return false;
	}
	try {
		if (await navigator.storage.persisted()) return true;
		return await navigator.storage.persist();
	} catch {
		return false;
	}
}

async function writeProject(project: EditorProject, expectedRevision: number | null): Promise<EditorProject> {
	const database = await openEditorDatabase();
	let transaction: IDBTransaction | null = null;
	try {
		transaction = database.transaction([projectStoreName, projectAssetStoreName], "readwrite");
		const projects = transaction.objectStore(projectStoreName);
		const existing = (await requestResult(projects.get(project.identity.id))) as StoredProjectRecord | undefined;

		if (expectedRevision === null && existing) {
			transaction.abort();
			throw new ProjectRevisionConflictError(project.identity.id, 0, existing.revision);
		}
		if (expectedRevision !== null && !existing) {
			transaction.abort();
			throw new ProjectNotFoundError(project.identity.id);
		}
		if (expectedRevision !== null && existing?.revision !== expectedRevision) {
			transaction.abort();
			throw new ProjectRevisionConflictError(project.identity.id, expectedRevision, existing?.revision ?? 0);
		}

		const committed: EditorProject = {
			...project,
			revision: expectedRevision === null ? 1 : expectedRevision + 1,
			updatedAt: new Date().toISOString(),
		};
		projects.put(toStoredProject(committed));

		const assetStore = transaction.objectStore(projectAssetStoreName);
		const retainedAssetIds = new Set(committed.assets.map((asset) => asset.id));
		const existingAssets = (await requestResult(
			assetStore.index("projectId").getAll(project.identity.id),
		)) as StoredProjectAsset[];
		const existingAssetsById = new Map(existingAssets.map((asset) => [asset.assetId, asset]));
		for (const asset of committed.assets) {
			if (existingAssetsById.get(asset.id)?.fingerprint !== assetStorageFingerprint(asset)) {
				assetStore.put(toStoredProjectAsset(project.identity.id, asset));
			}
		}
		for (const asset of existingAssets) {
			if (!retainedAssetIds.has(asset.assetId)) {
				assetStore.delete(projectAssetKey(project.identity.id, asset.assetId));
			}
		}

		await transactionComplete(transaction);
		return committed;
	} catch (error) {
		try {
			transaction?.abort();
		} catch {
			// The transaction may already have completed or aborted with the original error.
		}
		if (error instanceof ProjectRevisionConflictError || error instanceof ProjectNotFoundError) throw error;
		throw toEditorStorageError(error, "The project transaction failed.");
	}
}

function iterateKeyCursor(request: IDBRequest<IDBCursor | null>, visit: (cursor: IDBCursor) => void) {
	return iterateCursor(request, visit);
}

function corruptProjectStorage(error: unknown) {
	return new EditorStorageError(
		"corrupt",
		"A stored project record is damaged or incompatible. Open a valid .bbs backup or remove the affected local project.",
		{ cause: error },
	);
}

function iterateCursor<Cursor extends IDBCursor>(request: IDBRequest<Cursor | null>, visit: (cursor: Cursor) => void) {
	return new Promise<void>((resolve, reject) => {
		request.addEventListener("success", () => {
			const cursor = request.result;
			if (!cursor) {
				resolve();
				return;
			}
			visit(cursor);
			cursor.continue();
		});
		request.addEventListener("error", () => reject(request.error), { once: true });
	});
}
