const databaseName = "baudbound-editor";
const databaseVersion = 1;

export const projectStoreName = "projects";
export const projectAssetStoreName = "project-assets";
export const preferenceStoreName = "preferences";

let databasePromise: Promise<IDBDatabase> | null = null;

export function openEditorDatabase() {
	if (typeof indexedDB === "undefined") {
		return Promise.reject(new EditorStorageError("unavailable", "IndexedDB is unavailable in this browser context."));
	}

	databasePromise ??= openDatabase().catch((error) => {
		databasePromise = null;
		throw error;
	});
	return databasePromise;
}

export function requestResult<T>(request: IDBRequest<T>) {
	return new Promise<T>((resolve, reject) => {
		request.addEventListener("success", () => resolve(request.result), { once: true });
		request.addEventListener("error", () => reject(toEditorStorageError(request.error, "IndexedDB request failed.")), {
			once: true,
		});
	});
}

export function transactionComplete(transaction: IDBTransaction) {
	return new Promise<void>((resolve, reject) => {
		transaction.addEventListener("complete", () => resolve(), { once: true });
		transaction.addEventListener(
			"abort",
			() => reject(toEditorStorageError(transaction.error, "IndexedDB transaction was aborted.")),
			{ once: true },
		);
		transaction.addEventListener(
			"error",
			() => reject(toEditorStorageError(transaction.error, "IndexedDB transaction failed.")),
			{ once: true },
		);
	});
}

export type EditorStorageErrorKind = "blocked" | "corrupt" | "quota" | "transaction" | "unavailable";

export class EditorStorageError extends Error {
	constructor(
		readonly kind: EditorStorageErrorKind,
		message: string,
		options?: ErrorOptions,
	) {
		super(message, options);
		this.name = "EditorStorageError";
	}
}

function openDatabase() {
	return new Promise<IDBDatabase>((resolve, reject) => {
		const request = indexedDB.open(databaseName, databaseVersion);
		request.addEventListener("upgradeneeded", () => upgradeDatabase(request.result, request.transaction));
		request.addEventListener(
			"blocked",
			() => reject(new EditorStorageError("blocked", "Close other BaudBound Editor tabs to upgrade local storage.")),
			{ once: true },
		);
		request.addEventListener(
			"error",
			() => reject(toEditorStorageError(request.error, "Unable to open BaudBound Editor storage.")),
			{ once: true },
		);
		request.addEventListener(
			"success",
			() => {
				const database = request.result;
				database.addEventListener("versionchange", () => {
					database.close();
					databasePromise = null;
				});
				resolve(database);
			},
			{ once: true },
		);
	});
}

function upgradeDatabase(database: IDBDatabase, transaction: IDBTransaction | null) {
	if (!transaction) {
		throw new EditorStorageError("transaction", "IndexedDB upgrade transaction is unavailable.");
	}

	if (!database.objectStoreNames.contains(projectStoreName)) {
		const projects = database.createObjectStore(projectStoreName, { keyPath: "id" });
		projects.createIndex("updatedAt", "updatedAt", { unique: false });
	}
	if (!database.objectStoreNames.contains(projectAssetStoreName)) {
		const assets = database.createObjectStore(projectAssetStoreName, { keyPath: "key" });
		assets.createIndex("projectId", "projectId", { unique: false });
	}
	if (!database.objectStoreNames.contains(preferenceStoreName)) {
		database.createObjectStore(preferenceStoreName, { keyPath: "key" });
	}
}

export function toEditorStorageError(error: unknown, fallbackMessage: string) {
	if (error instanceof EditorStorageError) return error;
	const errorName = error instanceof DOMException ? error.name : null;
	const errorMessage = error instanceof Error ? error.message : null;
	if (errorName === "QuotaExceededError") {
		return new EditorStorageError("quota", "Browser storage quota was exceeded.", { cause: error });
	}
	if (errorName === "InvalidStateError" || errorName === "NotAllowedError") {
		return new EditorStorageError("unavailable", fallbackMessage, { cause: error });
	}
	return new EditorStorageError("transaction", errorMessage || fallbackMessage, { cause: error });
}
