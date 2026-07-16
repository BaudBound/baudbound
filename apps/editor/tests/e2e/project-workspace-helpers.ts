import { readFileSync } from "node:fs";
import { expect, type Locator, type Page } from "@playwright/test";
import JSZip from "jszip";

export async function createProject(page: Page, name: string) {
	await page.goto("/");
	await createProjectFromHome(page, name);
}

export async function trackBeforeUnloadListeners(page: Page) {
	await page.addInitScript(() => {
		const listeners = new Set<EventListenerOrEventListenerObject>();
		const state = window as typeof window & { __baudboundBeforeUnloadListenerCount?: () => number };
		state.__baudboundBeforeUnloadListenerCount = () => listeners.size;
		const originalAddEventListener = window.addEventListener.bind(window);
		const originalRemoveEventListener = window.removeEventListener.bind(window);
		window.addEventListener = ((
			type: string,
			listener: EventListenerOrEventListenerObject | null,
			options?: boolean | AddEventListenerOptions,
		) => {
			if (!listener) return;
			if (type === "beforeunload") listeners.add(listener);
			originalAddEventListener(type, listener, options);
		}) as typeof window.addEventListener;
		window.removeEventListener = ((
			type: string,
			listener: EventListenerOrEventListenerObject | null,
			options?: boolean | EventListenerOptions,
		) => {
			if (!listener) return;
			if (type === "beforeunload") listeners.delete(listener);
			originalRemoveEventListener(type, listener, options);
		}) as typeof window.removeEventListener;
	});
}

export async function readBeforeUnloadListenerCount(page: Page) {
	return page.evaluate(
		() =>
			(
				window as typeof window & { __baudboundBeforeUnloadListenerCount?: () => number }
			).__baudboundBeforeUnloadListenerCount?.() ?? 0,
	);
}

export async function createProjectFromHome(page: Page, name: string) {
	await page.getByRole("button", { name: "New project" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill(name);
	await page.getByRole("button", { name: "Create project" }).click();
	await expect(page.getByRole("button", { name: "Open asset editor" })).toBeVisible();
}

export async function downloadPackage(page: Page, destination: string) {
	await page.getByRole("button", { name: "Export package" }).click();
	await page.getByRole("button", { name: "Next" }).click();
	await page.getByRole("button", { name: "Next" }).click();
	await expect(page.getByText("Verification passed. The download button is now available.")).toBeVisible();
	const downloadPromise = page.waitForEvent("download");
	await page.getByRole("button", { name: "Download .bbs" }).click();
	const download = await downloadPromise;
	await download.saveAs(destination);
	await page.getByRole("button", { name: "Cancel export" }).click();
}

export async function readPackageManifest(path: string) {
	const zip = await JSZip.loadAsync(readFileSync(path));
	const entry = zip.file("manifest.json");
	if (!entry) throw new Error("Exported package is missing manifest.json.");
	return JSON.parse(await entry.async("text")) as { created_at: string; id: string; updated_at: string };
}

export async function failNextProjectPut(page: Page, errorName: string) {
	await page.evaluate((name) => {
		const originalPut = IDBObjectStore.prototype.put;
		IDBObjectStore.prototype.put = function (value: unknown, key?: IDBValidKey) {
			if (this.name === "projects") {
				IDBObjectStore.prototype.put = originalPut;
				throw new DOMException("Injected project write failure.", name);
			}
			const args = key === undefined ? [value] : [value, key];
			return Reflect.apply(originalPut, this, args) as IDBRequest<IDBValidKey>;
		};
	}, errorName);
}

export async function abortNextProjectTransaction(page: Page) {
	await page.evaluate(() => {
		const originalTransaction = IDBDatabase.prototype.transaction;
		IDBDatabase.prototype.transaction = function (
			storeNames: string | string[],
			mode?: IDBTransactionMode,
			options?: IDBTransactionOptions,
		) {
			const args = options === undefined ? [storeNames, mode] : [storeNames, mode, options];
			const transaction = Reflect.apply(originalTransaction, this, args) as IDBTransaction;
			const names = typeof storeNames === "string" ? [storeNames] : storeNames;
			if (mode === "readwrite" && names.includes("projects")) {
				IDBDatabase.prototype.transaction = originalTransaction;
				queueMicrotask(() => transaction.abort());
			}
			return transaction;
		};
	});
}

export async function readStoredAssetCounts(page: Page) {
	return page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const transaction = database.transaction(["projects", "project-assets"], "readonly");
		const id = location.pathname.split("/").at(-1);
		if (!id) throw new Error("Project ID is missing from the route.");
		const projectRequest = transaction.objectStore("projects").get(id);
		const assetRequest = transaction.objectStore("project-assets").index("projectId").count(id);
		const [project, assetBlobs] = await Promise.all([
			new Promise<{ assets: unknown[]; revision: number }>((resolve, reject) => {
				projectRequest.onsuccess = () => resolve(projectRequest.result);
				projectRequest.onerror = () => reject(projectRequest.error);
			}),
			new Promise<number>((resolve, reject) => {
				assetRequest.onsuccess = () => resolve(assetRequest.result);
				assetRequest.onerror = () => reject(assetRequest.error);
			}),
		]);
		database.close();
		return { assetBlobs, assetMetadata: project.assets.length, revision: project.revision };
	});
}

export async function expectCommentBoxNear(
	commentNode: Locator,
	expected: { height: number; width: number; x: number; y: number },
) {
	const actual = await commentNode.boundingBox();
	if (!actual) throw new Error("Comment node is not visible.");
	expect(actual.x).toBeCloseTo(expected.x, 0);
	expect(actual.y).toBeCloseTo(expected.y, 0);
	expect(actual.width).toBeCloseTo(expected.width, 0);
	expect(actual.height).toBeCloseTo(expected.height, 0);
}

export async function expectNodePositionNear(node: Locator, expected: { x: number; y: number }) {
	const actual = await node.boundingBox();
	if (!actual) throw new Error("Graph node is not visible.");
	expect(actual.x).toBeCloseTo(expected.x, 0);
	expect(actual.y).toBeCloseTo(expected.y, 0);
}

export async function readStoredProjectCount(page: Page) {
	return page.evaluate(async () => {
		const database = await openTestDatabase();
		const request = database.transaction("projects", "readonly").objectStore("projects").count();
		const count = await requestValue(request);
		database.close();
		return count;

		function openTestDatabase() {
			return requestValue(indexedDB.open("baudbound-editor", 1));
		}
		function requestValue<T>(request: IDBRequest<T>) {
			return new Promise<T>((resolve, reject) => {
				request.onsuccess = () => resolve(request.result);
				request.onerror = () => reject(request.error);
			});
		}
	});
}

export async function readStoredNodeCount(page: Page) {
	return page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const projectId = location.pathname.split("/").at(-1);
		if (!projectId) throw new Error("Project ID is missing from the route.");
		const projectRequest = database.transaction("projects", "readonly").objectStore("projects").get(projectId);
		const project = await new Promise<{ nodes: unknown[] }>((resolve, reject) => {
			projectRequest.onsuccess = () => resolve(projectRequest.result);
			projectRequest.onerror = () => reject(projectRequest.error);
		});
		database.close();
		return project.nodes.length;
	});
}

export async function readStoredProjectIdentities(page: Page) {
	return page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const recordsRequest = database.transaction("projects", "readonly").objectStore("projects").getAll();
		const records = await new Promise<Array<{ createdAt: string; id: string }>>((resolve, reject) => {
			recordsRequest.onsuccess = () => resolve(recordsRequest.result);
			recordsRequest.onerror = () => reject(recordsRequest.error);
		});
		database.close();
		return records.map(({ createdAt, id }) => ({ createdAt, id }));
	});
}

export async function delayNextProjectSaveCompletion(page: Page, delayMs: number) {
	await page.evaluate((delay) => {
		const originalAddEventListener = IDBTransaction.prototype.addEventListener;
		IDBTransaction.prototype.addEventListener = function (
			type: string,
			listener: EventListenerOrEventListenerObject | null,
			options?: boolean | AddEventListenerOptions,
		) {
			const isProjectWrite = this.mode === "readwrite" && Array.from(this.objectStoreNames).includes("projects");
			if (type !== "complete" || !isProjectWrite || listener === null) {
				return Reflect.apply(originalAddEventListener, this, [type, listener, options]);
			}
			IDBTransaction.prototype.addEventListener = originalAddEventListener;
			const delayedListener: EventListener = (event) => {
				setTimeout(() => {
					if (typeof listener === "function") listener.call(this, event);
					else listener.handleEvent(event);
				}, delay);
			};
			return Reflect.apply(originalAddEventListener, this, [type, delayedListener, options]);
		};
	}, delayMs);
}

export async function startProjectAssetPutCounter(page: Page) {
	await page.evaluate(() => {
		const state = window as typeof window & { __baudboundAssetPutCount?: number };
		state.__baudboundAssetPutCount = 0;
		const originalPut = IDBObjectStore.prototype.put;
		IDBObjectStore.prototype.put = function (value: unknown, key?: IDBValidKey) {
			if (this.name === "project-assets") {
				state.__baudboundAssetPutCount = (state.__baudboundAssetPutCount ?? 0) + 1;
			}
			const args = key === undefined ? [value] : [value, key];
			return Reflect.apply(originalPut, this, args) as IDBRequest<IDBValidKey>;
		};
	});
}

export async function readProjectAssetPutCount(page: Page) {
	return page.evaluate(
		() => (window as typeof window & { __baudboundAssetPutCount?: number }).__baudboundAssetPutCount ?? 0,
	);
}

export async function readLegacyPanelValues(page: Page) {
	return page.evaluate(() => ({
		collapsed: localStorage.getItem("baudbound.editor.panel-collapsed.v1"),
		sizes: localStorage.getItem("baudbound.editor.panel-sizes.v1"),
	}));
}
