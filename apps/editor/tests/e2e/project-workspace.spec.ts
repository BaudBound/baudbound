import { readFileSync, writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

import {
	abortNextProjectTransaction,
	createProject,
	createProjectFromHome,
	delayNextProjectSaveCompletion,
	downloadPackage,
	failNextProjectPut,
	readBeforeUnloadListenerCount,
	readLegacyPanelValues,
	readPackageManifest,
	readProjectAssetPutCount,
	readStoredAssetCounts,
	readStoredNodeCount,
	readStoredProjectCount,
	readStoredProjectIdentities,
	startProjectAssetPutCounter,
	trackBeforeUnloadListeners,
} from "./project-workspace-helpers";

test("project creation requires valid settings before writing storage", async ({ page }) => {
	await page.goto("/");
	await page.getByRole("button", { name: "New project" }).click();
	const name = page.getByRole("textbox", { name: "Name" });
	const website = page.getByRole("textbox", { name: "Website" });
	const create = page.getByRole("button", { name: "Create project" });

	await name.fill("");
	await expect(page.getByText("Project name is required.")).toBeVisible();
	await expect(create).toBeDisabled();
	await name.fill("Validated project");
	await website.fill("not a URL");
	await expect(page.getByText("Use a valid URL.")).toBeVisible();
	await expect(create).toBeDisabled();
	await website.fill("https://baudbound.app/projects/validated");
	await create.click();

	await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+$/);
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	expect(await readStoredProjectCount(page)).toBe(1);
});

test("project home disables the native browser context menu", async ({ page }) => {
	await page.goto("/");
	const defaultAllowed = await page.evaluate(() =>
		document.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })),
	);
	expect(defaultAllowed).toBe(false);
});

test("initial IndexedDB upgrade creates the complete versioned schema", async ({ page }) => {
	await page.goto("/missing-schema-probe");
	await page.evaluate(
		() =>
			new Promise<void>((resolve, reject) => {
				const request = indexedDB.deleteDatabase("baudbound-editor");
				request.onsuccess = () => resolve();
				request.onerror = () => reject(request.error);
				request.onblocked = () => reject(new Error("The test database is still open."));
			}),
	);

	await page.goto("/");
	await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
	const schema = await page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const transaction = database.transaction(["projects", "project-assets", "preferences"], "readonly");
		const result = {
			assetIndexes: Array.from(transaction.objectStore("project-assets").indexNames),
			preferenceIndexes: Array.from(transaction.objectStore("preferences").indexNames),
			projectIndexes: Array.from(transaction.objectStore("projects").indexNames),
			stores: Array.from(database.objectStoreNames),
			version: database.version,
		};
		database.close();
		return result;
	});

	expect(schema).toEqual({
		assetIndexes: ["projectId"],
		preferenceIndexes: [],
		projectIndexes: ["updatedAt"],
		stores: ["preferences", "project-assets", "projects"],
		version: 1,
	});
});

test("project creation, save, reload, and home listing use durable storage", async ({ page }) => {
	await createProject(page, "Durable project");
	await page.getByRole("button", { name: "Manual" }).click();
	await expect(page.getByText("unsaved changes", { exact: true })).toBeVisible();

	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	const projectUrl = page.url();

	await page.reload();
	await expect(page).toHaveURL(projectUrl);
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
	await page.getByRole("button", { name: "Return to projects" }).click();

	await expect(page.getByText("Durable project", { exact: true })).toBeVisible();
	await expect(page.getByText("1 node, 0 connections", { exact: true })).toBeVisible();
});

test("rapid save requests commit only one project revision", async ({ page }) => {
	await createProject(page, "Single save transaction");
	await page.getByRole("button", { name: "Manual" }).click();
	await delayNextProjectSaveCompletion(page, 300);

	await page.evaluate(() => {
		window.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, ctrlKey: true, key: "s" }));
		window.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, ctrlKey: true, key: "s" }));
	});
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	expect((await readStoredAssetCounts(page)).revision).toBe(2);
});

test("returning home protects unsaved changes", async ({ page }) => {
	await createProject(page, "Unsaved project");
	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("button", { name: "Return to projects" }).click();

	await expect(page.getByRole("heading", { name: "Save changes?" })).toBeVisible();
	await page.getByRole("button", { name: "Cancel" }).click();
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);

	await page.getByRole("button", { name: "Return to projects" }).click();
	await page.getByRole("button", { name: "Discard" }).click();
	await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
});

test("Save and return commits changes before opening the project home", async ({ page }) => {
	await createProject(page, "Saved on exit");
	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("button", { name: "Return to projects" }).click();
	await page.getByRole("button", { name: "Save and return" }).click();

	await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
	await page.getByText("Saved on exit", { exact: true }).click();
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
});

test("browser reload warning is registered only while project changes are unsaved", async ({ page }) => {
	await trackBeforeUnloadListeners(page);
	await createProject(page, "Unload warning");
	expect(await readBeforeUnloadListenerCount(page)).toBe(0);
	await page.getByRole("button", { name: "Manual" }).click();
	expect(await readBeforeUnloadListenerCount(page)).toBe(1);

	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	expect(await readBeforeUnloadListenerCount(page)).toBe(0);
});

test("edits made while saving remain unsaved after the committed snapshot completes", async ({ page }) => {
	await createProject(page, "Concurrent save edits");
	await page.getByRole("button", { name: "Manual" }).click();
	await delayNextProjectSaveCompletion(page, 500);
	await page.keyboard.press("Control+s");
	await expect(page.getByText("saving...", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Output & Timing" }).click();
	await page.getByRole("button", { name: /^Log/ }).click();

	await expect(page.getByText("unsaved changes", { exact: true })).toBeVisible();
	await expect(page.locator(".react-flow__node")).toHaveCount(2);
	expect(await readStoredNodeCount(page)).toBe(1);
});

test("legacy panel preferences migrate once into IndexedDB", async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() => {
		localStorage.setItem(
			"baudbound.editor.panel-collapsed.v1",
			JSON.stringify({ left: true, right: false, bottom: true }),
		);
		localStorage.setItem("baudbound.editor.panel-sizes.v1", JSON.stringify({ left: 280, right: 360, bottom: 240 }));
	});
	await createProjectFromHome(page, "Migrated preferences");

	await expect(page.getByRole("button", { name: "Expand block library" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand bottom panel" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Collapse inspector" })).toBeVisible();
	const legacyValues = await page.evaluate(() => ({
		collapsed: localStorage.getItem("baudbound.editor.panel-collapsed.v1"),
		sizes: localStorage.getItem("baudbound.editor.panel-sizes.v1"),
	}));
	expect(legacyValues).toEqual({ collapsed: null, sizes: null });
});

test("interrupted panel migration retries before deleting legacy values", async ({ page }) => {
	await page.addInitScript(() => {
		if (localStorage.getItem("baudbound.test.preference-abort-injected")) return;
		const originalTransaction = IDBDatabase.prototype.transaction;
		IDBDatabase.prototype.transaction = function (
			storeNames: string | string[],
			mode?: IDBTransactionMode,
			options?: IDBTransactionOptions,
		) {
			const args = options === undefined ? [storeNames, mode] : [storeNames, mode, options];
			const transaction = Reflect.apply(originalTransaction, this, args) as IDBTransaction;
			const names = typeof storeNames === "string" ? [storeNames] : storeNames;
			if (mode === "readwrite" && names.includes("preferences")) {
				localStorage.setItem("baudbound.test.preference-abort-injected", "true");
				IDBDatabase.prototype.transaction = originalTransaction;
				queueMicrotask(() => transaction.abort());
			}
			return transaction;
		};
	});
	await page.goto("/");
	await page.evaluate(() => {
		localStorage.setItem(
			"baudbound.editor.panel-collapsed.v1",
			JSON.stringify({ left: true, right: false, bottom: true }),
		);
		localStorage.setItem("baudbound.editor.panel-sizes.v1", JSON.stringify({ left: 280, right: 360, bottom: 240 }));
	});
	await createProjectFromHome(page, "Migration recovery");

	const retainedLegacyValues = await readLegacyPanelValues(page);
	expect(retainedLegacyValues.collapsed).not.toBeNull();
	expect(retainedLegacyValues.sizes).not.toBeNull();

	await page.reload();
	await expect(page.getByRole("button", { name: "Expand block library" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand bottom panel" })).toBeVisible();
	expect(await readLegacyPanelValues(page)).toEqual({ collapsed: null, sizes: null });
});

test("malformed legacy panel values fall back safely and migrate once", async ({ page }) => {
	await page.goto("/");
	await page.evaluate(() => {
		localStorage.setItem("baudbound.editor.panel-collapsed.v1", "{not-json");
		localStorage.setItem("baudbound.editor.panel-sizes.v1", JSON.stringify({ left: -100, right: "wide" }));
	});
	await createProjectFromHome(page, "Malformed migration");

	await expect(page.getByRole("button", { name: "Collapse block library" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Collapse inspector" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Collapse bottom panel" })).toBeVisible();
	expect(await readLegacyPanelValues(page)).toEqual({ collapsed: null, sizes: null });
});

test("only one tab can edit a project and takeover protects unsaved changes", async ({ context, page }) => {
	await createProject(page, "Shared project");
	const projectUrl = page.url();
	const secondPage = await context.newPage();

	await secondPage.goto(projectUrl);
	await expect(secondPage.getByRole("heading", { name: "Project already open" })).toBeVisible();
	await secondPage.getByRole("button", { name: "Take control" }).click();
	await expect(secondPage.getByRole("button", { name: "Open asset editor" })).toBeVisible();
	await expect(page.getByRole("heading", { name: "Project already open" })).toBeVisible();

	await secondPage.getByRole("button", { name: "Manual" }).click();
	await expect(secondPage.getByText("unsaved changes", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Take control" }).click();
	await expect(page.getByText("The other tab has unsaved changes.", { exact: false })).toBeVisible();
	await expect(secondPage.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
});

test("a stale editor cannot overwrite a newer stored revision", async ({ page }) => {
	await createProject(page, "Revision conflict");
	await page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const transaction = database.transaction("projects", "readwrite");
		const store = transaction.objectStore("projects");
		const id = location.pathname.split("/").at(-1);
		if (!id) throw new Error("Project ID is missing from the route.");
		const record = await new Promise<Record<string, unknown>>((resolve, reject) => {
			const getRequest = store.get(id);
			getRequest.onsuccess = () => resolve(getRequest.result);
			getRequest.onerror = () => reject(getRequest.error);
		});
		store.put({ ...record, revision: 2, updatedAt: new Date().toISOString() });
		await new Promise<void>((resolve, reject) => {
			transaction.oncomplete = () => resolve();
			transaction.onerror = () => reject(transaction.error);
		});
		database.close();
	});

	await page.getByRole("button", { name: "Manual" }).click();
	await page.keyboard.press("Control+s");
	await expect(page.getByRole("heading", { name: "Project changed in another session" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Export current project" })).toBeVisible();
	await expect(page.getByText("save failed", { exact: true })).toBeVisible();
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);

	const stored = await page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const transaction = database.transaction("projects", "readonly");
		const id = location.pathname.split("/").at(-1);
		if (!id) throw new Error("Project ID is missing from the route.");
		const record = await new Promise<{ nodes: unknown[]; revision: number }>((resolve, reject) => {
			const getRequest = transaction.objectStore("projects").get(id);
			getRequest.onsuccess = () => resolve(getRequest.result);
			getRequest.onerror = () => reject(getRequest.error);
		});
		database.close();
		return { nodeCount: record.nodes.length, revision: record.revision };
	});
	expect(stored).toEqual({ nodeCount: 0, revision: 2 });
});

test("repeated exports preserve project identity while package content changes", async ({ page }, testInfo) => {
	await createProject(page, "Stable identity");
	await page.getByRole("button", { name: "Manual" }).click();
	const firstPath = testInfo.outputPath("first.bbs");
	await downloadPackage(page, firstPath);

	await page.getByTitle("Add comment").click();
	await page.getByPlaceholder("Write a note...").fill("Second revision");
	const secondPath = testInfo.outputPath("second.bbs");
	await downloadPackage(page, secondPath);

	const firstManifest = await readPackageManifest(firstPath);
	const secondManifest = await readPackageManifest(secondPath);
	expect(secondManifest.id).toBe(firstManifest.id);
	expect(secondManifest.created_at).toBe(firstManifest.created_at);
	expect(secondManifest.updated_at).not.toBe(firstManifest.updated_at);
	expect(readFileSync(secondPath).equals(readFileSync(firstPath))).toBe(false);
});

test("saved projects can be exported directly from the project list", async ({ page }, testInfo) => {
	await createProject(page, "Home export");
	await page.getByRole("button", { name: "Manual" }).click();
	await page.keyboard.press("Control+s");
	await page.getByRole("button", { name: "Return to projects" }).click();

	const downloadPromise = page.waitForEvent("download");
	await page.getByRole("button", { name: "Export Home export" }).click();
	const download = await downloadPromise;
	expect(download.suggestedFilename()).toBe("home-export.bbs");
	const destination = testInfo.outputPath("home-export.bbs");
	await download.saveAs(destination);
	const manifest = await readPackageManifest(destination);
	expect(manifest.id).toMatch(/^[0-9a-f-]{36}$/);
	await expect(page.getByText("Exported Home export.", { exact: true })).toBeVisible();
});

test("duplicating and deleting projects keeps local records isolated", async ({ page }) => {
	await createProject(page, "Source project");
	await page.getByRole("button", { name: "Manual" }).click();
	await page.keyboard.press("Control+s");
	await page.getByRole("button", { name: "Return to projects" }).click();

	await page.getByRole("button", { name: "Duplicate Source project" }).click();
	await expect(page.getByText("Source project copy", { exact: true })).toBeVisible();
	await expect(page.getByText("1 node, 0 connections", { exact: true })).toHaveCount(2);
	const identities = await readStoredProjectIdentities(page);
	expect(identities).toHaveLength(2);
	expect(new Set(identities.map((identity) => identity.id)).size).toBe(2);
	expect(new Set(identities.map((identity) => identity.createdAt)).size).toBe(2);

	await page.getByRole("button", { name: "Delete Source project copy" }).click();
	await expect(page.getByRole("heading", { name: "Delete project" })).toBeVisible();
	await page.getByRole("button", { name: "Delete project" }).click();
	await expect(page.getByText("Source project copy", { exact: true })).toHaveCount(0);
	await expect(page.getByText("Source project", { exact: true })).toBeVisible();
});

test("damaged project records show recovery instead of opening partial state", async ({ page }) => {
	await createProject(page, "Damaged project");
	await page.evaluate(async () => {
		const request = indexedDB.open("baudbound-editor", 1);
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		const transaction = database.transaction("projects", "readwrite");
		const store = transaction.objectStore("projects");
		const id = location.pathname.split("/").at(-1);
		if (!id) throw new Error("Project ID is missing from the route.");
		const record = await new Promise<Record<string, unknown>>((resolve, reject) => {
			const getRequest = store.get(id);
			getRequest.onsuccess = () => resolve(getRequest.result);
			getRequest.onerror = () => reject(getRequest.error);
		});
		store.put({ ...record, schemaVersion: 999 });
		await new Promise<void>((resolve, reject) => {
			transaction.oncomplete = () => resolve();
			transaction.onerror = () => reject(transaction.error);
		});
		database.close();
	});

	await page.reload();
	await expect(page.getByRole("heading", { name: "Project unavailable" })).toBeVisible();
	await expect(page.getByText("stored project record is damaged", { exact: false })).toBeVisible();
	await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Projects" })).toBeVisible();
});

test("quota failures preserve the committed project and binary assets", async ({ page }, testInfo) => {
	const assetPath = testInfo.outputPath("recovery-asset.txt");
	writeFileSync(assetPath, "asset content that must survive a failed save");
	await createProject(page, "Quota recovery");
	await page.getByRole("button", { name: "Open asset editor" }).click();
	await page.locator('input[type="file"][multiple]').setInputFiles(assetPath);
	await expect(page.getByText("recovery-asset.txt", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Close", exact: true }).click();
	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();

	await page.getByRole("button", { name: "Open asset editor" }).click();
	await page.getByRole("button", { name: "Remove recovery-asset.txt" }).click();
	await page.getByRole("button", { name: "Close", exact: true }).click();
	await failNextProjectPut(page, "QuotaExceededError");
	await page.keyboard.press("Control+s");

	await expect(page.getByRole("heading", { name: "Browser storage is full" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Retry save" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Export current project" })).toBeVisible();
	await expect(page.getByText("save failed", { exact: true })).toBeVisible();
	expect(await readStoredAssetCounts(page)).toEqual({ assetBlobs: 1, assetMetadata: 1, revision: 2 });

	await page.getByRole("button", { name: "Retry save" }).click();
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	expect(await readStoredAssetCounts(page)).toEqual({ assetBlobs: 0, assetMetadata: 0, revision: 3 });
});

test("graph-only saves do not rewrite unchanged binary assets", async ({ page }, testInfo) => {
	const assetPath = testInfo.outputPath("unchanged-asset.txt");
	writeFileSync(assetPath, "asset content that should not be rewritten");
	await createProject(page, "Asset write efficiency");
	await page.getByRole("button", { name: "Open asset editor" }).click();
	await page.locator('input[type="file"][multiple]').setInputFiles(assetPath);
	await page.getByRole("button", { name: "Close", exact: true }).click();
	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();

	await startProjectAssetPutCounter(page);
	await page.getByRole("button", { name: "Manual" }).click();
	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();
	expect(await readProjectAssetPutCount(page)).toBe(0);
});

test("aborted save transactions remain recoverable and do not advance revisions", async ({ page }) => {
	await createProject(page, "Transaction recovery");
	await page.getByRole("button", { name: "Manual" }).click();
	await abortNextProjectTransaction(page);
	await page.keyboard.press("Control+s");

	await expect(page.getByRole("heading", { name: "Project was not saved" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Retry save" })).toBeVisible();
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
	await expect(page.getByText("save failed", { exact: true })).toBeVisible();
	const stored = await readStoredAssetCounts(page);
	expect(stored).toEqual({ assetBlobs: 0, assetMetadata: 0, revision: 1 });
});

test("Save and return stays in the editor when persistence fails", async ({ page }) => {
	await createProject(page, "Failed exit save");
	await page.getByRole("button", { name: "Manual" }).click();
	await abortNextProjectTransaction(page);
	await page.getByRole("button", { name: "Return to projects" }).click();
	await page.getByRole("button", { name: "Save and return" }).click();

	await expect(page.getByRole("heading", { name: "Project was not saved" })).toBeVisible();
	await expect(page).toHaveURL(/\/projects\//);
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
	await expect(page.getByText("save failed", { exact: true })).toBeVisible();
});
