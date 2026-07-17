import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

import { createProject, expectCommentBoxNear, expectNodePositionNear } from "./project-workspace-helpers";

test("undo and redo restore document changes", async ({ page }) => {
	await createProject(page, "History project");
	const logNodes = page.locator(".react-flow__node").filter({ hasText: "Log" });

	await page.getByRole("textbox", { name: "Search blocks" }).fill("Log");
	await page.getByRole("button", { name: /^Log/ }).click();
	await page.getByRole("button", { name: /^Log/ }).click();
	await expect(logNodes).toHaveCount(2);
	await page.keyboard.press("Control+z");
	await expect(logNodes).toHaveCount(1);
	await page.keyboard.press("Control+z");
	await expect(logNodes).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(logNodes).toHaveCount(1);
	await page.keyboard.press("Control+y");
	await expect(logNodes).toHaveCount(2);
});

test("history tracks saved boundaries, survives save, and resets after reload", async ({ page }) => {
	await createProject(page, "Save history");
	await page.getByRole("button", { name: "Manual" }).click();
	await page.keyboard.press("Control+s");
	await expect(page.getByText("saved", { exact: true })).toBeVisible();

	await page.keyboard.press("Control+z");
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(0);
	await expect(page.getByText("unsaved changes", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+Shift+z");
	await expect(page.locator(".react-flow__node").filter({ hasText: "Manual Trigger" })).toHaveCount(1);
	await expect(page.getByText("saved", { exact: true })).toBeVisible();

	await page.reload();
	await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();
	await expect(page.getByRole("button", { name: "Redo" })).toBeDisabled();
});

test("new edits clear redo and field changes coalesce into one history entry", async ({ page }) => {
	await createProject(page, "Coalesced history");
	await page.getByRole("button", { name: "Output & Timing" }).click();
	await page.getByRole("button", { name: /^Log/ }).click();
	const customName = page.getByRole("textbox", { name: "Custom name" });
	await customName.pressSequentially("Coalesced name");
	await customName.evaluate((element) => element.blur());
	await page.waitForTimeout(350);

	await page.keyboard.press("Control+z");
	const logNode = page.locator(".react-flow__node").filter({ hasText: "Log" });
	await expect(logNode).toHaveCount(1);
	await logNode.click();
	await expect(page.getByRole("textbox", { name: "Custom name" })).toHaveValue("");
	await expect(page.getByRole("button", { name: "Redo" })).toBeEnabled();

	await page.getByRole("button", { name: /^Beep/ }).click();
	await expect(page.getByRole("button", { name: "Redo" })).toBeDisabled();
});

test("focused text fields keep native undo instead of undoing the graph", async ({ page }) => {
	await createProject(page, "Native text undo");
	await page.getByRole("button", { name: "Output & Timing" }).click();
	await page.getByRole("button", { name: /^Log/ }).click();
	const customName = page.getByRole("textbox", { name: "Custom name" });
	await customName.click();
	await page.keyboard.type("Native text");
	await page.keyboard.press("Control+z");

	await expect(customName).not.toHaveValue("Native text");
	await expect(page.locator(".react-flow__node").filter({ hasText: "Log" })).toHaveCount(1);
});

test("one complete node drag creates one history entry", async ({ page }) => {
	await createProject(page, "Drag history");
	await page.getByRole("button", { name: "Output & Timing" }).click();
	await page.getByRole("button", { name: /^Log/ }).click();
	const logNode = page.locator(".react-flow__node").filter({ hasText: "Log" });
	const originalBox = await logNode.boundingBox();
	if (!originalBox) throw new Error("Log node is not visible.");

	await page.mouse.move(originalBox.x + originalBox.width / 2, originalBox.y + 24);
	await page.mouse.down();
	await page.mouse.move(originalBox.x + originalBox.width / 2 + 140, originalBox.y + 104, { steps: 12 });
	await page.mouse.up();
	const movedBox = await logNode.boundingBox();
	if (!movedBox) throw new Error("Moved Log node is not visible.");
	expect(Math.abs(movedBox.x - originalBox.x)).toBeGreaterThan(100);

	await page.keyboard.press("Control+z");
	const restoredBox = await logNode.boundingBox();
	if (!restoredBox) throw new Error("Restored Log node is not visible.");
	expect(restoredBox.x).toBeCloseTo(originalBox.x, 0);
	expect(restoredBox.y).toBeCloseTo(originalBox.y, 0);
	await page.keyboard.press("Control+z");
	await expect(logNode).toHaveCount(0);
});

test("history restores project settings, variables, secrets, comments, and edge style", async ({ page }) => {
	await createProject(page, "Durable history fields");

	await page.getByRole("button", { name: "Open project settings" }).click();
	await page.getByRole("button", { name: "Target runtime" }).click();
	await page.getByRole("option", { name: "Windows Desktop" }).click();
	await page.getByRole("button", { name: "Save Settings" }).click();
	await expect(page.getByText("Windows Desktop", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("Generic Desktop", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+y");
	await expect(page.getByText("Windows Desktop", { exact: true })).toBeVisible();

	await page.getByRole("button", { name: "Variables", exact: true }).click();
	await page.getByRole("button", { name: "Add variable" }).click();
	const variableDialog = page.getByRole("dialog");
	await variableDialog.getByRole("textbox", { name: "Name" }).fill("history_value");
	await variableDialog.getByRole("textbox", { name: "Default value" }).fill("restored");
	await variableDialog.getByRole("button", { name: "Save", exact: true }).click();
	await expect(page.getByText("history_value", { exact: true }).first()).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history_value", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history_value", { exact: true }).first()).toBeVisible();
	await page.getByRole("button", { name: "Edit history_value" }).click();
	await variableDialog.getByRole("textbox", { name: "Description" }).fill("Variable history description");
	await variableDialog.getByRole("button", { name: "Save", exact: true }).click();
	await expect(page.getByText("Variable history description", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("Variable history description", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(page.getByText("Variable history description", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Delete history_value" }).click();
	await expect(page.getByText("history_value", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history_value", { exact: true }).first()).toBeVisible();
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history_value", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+z");

	await page.getByRole("button", { name: "Add secret" }).click();
	const secretDialog = page.getByRole("dialog");
	await secretDialog.getByRole("textbox", { name: "Name" }).fill("history_secret");
	await secretDialog.getByRole("button", { name: "Save", exact: true }).click();
	await expect(page.getByText("history_secret", { exact: true }).first()).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history_secret", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history_secret", { exact: true }).first()).toBeVisible();
	await page.getByRole("button", { name: "Edit history_secret" }).click();
	await secretDialog.getByRole("textbox", { name: "Description" }).fill("Secret history description");
	await secretDialog.getByRole("button", { name: "Save", exact: true }).click();
	await expect(page.getByText("Secret history description", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("Secret history description", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(page.getByText("Secret history description", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Delete history_secret" }).click();
	await expect(page.getByText("history_secret", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history_secret", { exact: true }).first()).toBeVisible();
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history_secret", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+z");

	await page.getByTitle("Add comment").click();
	const comment = page.getByPlaceholder("Write a note...");
	await comment.fill("History comment");
	await comment.evaluate((element) => element.blur());
	await page.waitForTimeout(350);
	await page.keyboard.press("Control+z");
	await expect(comment).toHaveValue("");
	await page.keyboard.press("Control+y");
	await expect(comment).toHaveValue("History comment");

	await page.getByRole("button", { name: "Edge style" }).click();
	await page.getByRole("option", { name: "Bezier" }).click();
	await expect(page.getByRole("button", { name: "Edge style" })).toContainText("Bezier");
	await page.keyboard.press("Control+z");
	await expect(page.getByRole("button", { name: "Edge style" })).toContainText("Smooth step");
	await page.keyboard.press("Control+y");
	await expect(page.getByRole("button", { name: "Edge style" })).toContainText("Bezier");
});

test("history restores added and removed binary asset references", async ({ page }, testInfo) => {
	const assetPath = testInfo.outputPath("history-asset.txt");
	writeFileSync(assetPath, "asset retained by document history");
	await createProject(page, "Asset history");

	await page.getByRole("button", { name: "Open asset editor" }).click();
	await page.locator('input[type="file"][multiple]').setInputFiles(assetPath);
	await expect(page.getByText("history-asset.txt", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history-asset.txt", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history-asset.txt", { exact: true })).toBeVisible();

	await page.getByRole("button", { name: "Remove history-asset.txt" }).click();
	await expect(page.getByText("history-asset.txt", { exact: true })).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(page.getByText("history-asset.txt", { exact: true })).toBeVisible();
	await page.keyboard.press("Control+y");
	await expect(page.getByText("history-asset.txt", { exact: true })).toHaveCount(0);
});

test("history restores edge connections, node duplication, and incident-edge deletion", async ({ page }) => {
	await createProject(page, "Graph transaction history");
	const search = page.getByRole("textbox", { name: "Search blocks" });
	await search.fill("Log");
	await page.getByRole("button", { name: /^Log/ }).click();
	await search.fill("HTTP Request");
	await page.getByRole("button", { name: /HTTP Request/ }).click();

	const logNodes = page.locator(".react-flow__node").filter({ hasText: "Log" });
	const httpNodes = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" });
	const edges = page.locator(".react-flow__edge");
	await logNodes.first().locator(".react-flow__handle.source").first().dispatchEvent("click", { bubbles: true });
	await httpNodes.first().locator(".react-flow__handle.target").first().dispatchEvent("click", { bubbles: true });
	await expect(edges).toHaveCount(1);

	await page.keyboard.press("Control+z");
	await expect(edges).toHaveCount(0);
	await page.keyboard.press("Control+y");
	await expect(edges).toHaveCount(1);

	await edges.first().dispatchEvent("click", { bubbles: true });
	await page.getByRole("button", { name: "Disconnect selected edge" }).click();
	await expect(edges).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(edges).toHaveCount(1);
	await page.keyboard.press("Control+y");
	await expect(edges).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(edges).toHaveCount(1);

	await logNodes.first().dispatchEvent("contextmenu", { bubbles: true, button: 2, cancelable: true });
	await page.getByRole("menuitem", { name: "Duplicate" }).click();
	await expect(logNodes).toHaveCount(2);
	await page.keyboard.press("Control+z");
	await expect(logNodes).toHaveCount(1);
	await page.keyboard.press("Control+y");
	await expect(logNodes).toHaveCount(2);
	await page.keyboard.press("Control+z");
	await expect(logNodes).toHaveCount(1);

	await logNodes.first().dispatchEvent("click", { bubbles: true });
	await page.keyboard.press("Delete");
	await expect(logNodes).toHaveCount(0);
	await expect(edges).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(logNodes).toHaveCount(1);
	await expect(edges).toHaveCount(1);
	await page.keyboard.press("Control+y");
	await expect(logNodes).toHaveCount(0);
	await expect(edges).toHaveCount(0);
});

test("history restores dynamic switch rows and their edited values", async ({ page }) => {
	await createProject(page, "Dynamic row history");
	await page.getByRole("textbox", { name: "Search blocks" }).fill("Switch");
	await page.getByRole("button", { name: /^Switch/ }).click();
	const switchNode = page.locator(".react-flow__node").filter({ hasText: "Switch" }).first();
	const cases = page.getByRole("list", { name: "Switch cases" }).locator("li");
	await expect(cases).toHaveCount(2);

	await page.getByRole("button", { name: "Add switch case" }).click();
	await expect(cases).toHaveCount(3);
	await page.keyboard.press("Control+z");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases).toHaveCount(2);
	await page.keyboard.press("Control+y");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases).toHaveCount(3);

	const thirdName = cases.nth(2).getByRole("textbox", { name: "Name" });
	const originalName = await thirdName.inputValue();
	await thirdName.fill("fallback");
	await thirdName.evaluate((element) => element.blur());
	await page.waitForTimeout(350);
	await page.keyboard.press("Control+z");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases.nth(2).getByRole("textbox", { name: "Name" })).toHaveValue(originalName);
	await page.keyboard.press("Control+y");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases.nth(2).getByRole("textbox", { name: "Name" })).toHaveValue("fallback");

	await cases.nth(2).getByRole("button", { name: "Remove switch case" }).click();
	await expect(cases).toHaveCount(2);
	await page.keyboard.press("Control+z");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases).toHaveCount(3);
	await expect(cases.nth(2).getByRole("textbox", { name: "Name" })).toHaveValue("fallback");
	await page.keyboard.press("Control+y");
	await switchNode.dispatchEvent("click", { bubbles: true });
	await expect(cases).toHaveCount(2);
});

test("history restores comment position, size, color, and deletion", async ({ page }) => {
	await createProject(page, "Comment transaction history");
	await page.getByTitle("Add comment").click();
	const commentNode = page.locator(".baud-comment-flow-node");
	const dragHandle = page.locator(".baud-comment-drag-handle");
	const initialBox = await commentNode.boundingBox();
	const dragBox = await dragHandle.boundingBox();
	if (!initialBox || !dragBox) throw new Error("Comment node controls are not visible.");

	await page.mouse.move(dragBox.x + dragBox.width / 2, dragBox.y + dragBox.height / 2);
	await page.mouse.down();
	await page.mouse.move(dragBox.x + dragBox.width / 2 + 90, dragBox.y + dragBox.height / 2 + 55, { steps: 8 });
	await page.mouse.up();
	await page.waitForTimeout(350);
	const movedBox = await commentNode.boundingBox();
	if (!movedBox) throw new Error("Moved comment node is not visible.");
	expect(movedBox.x - initialBox.x).toBeGreaterThan(70);
	expect(movedBox.y - initialBox.y).toBeGreaterThan(35);
	await page.keyboard.press("Control+z");
	await expectCommentBoxNear(commentNode, initialBox);
	await page.keyboard.press("Control+y");
	await expectCommentBoxNear(commentNode, movedBox);

	const resizeControl = page.getByRole("button", { name: "Resize comment" });
	const resizeBox = await resizeControl.boundingBox();
	if (!resizeBox) throw new Error("Comment resize control is not visible.");
	const resizeStart = { x: resizeBox.x + resizeBox.width / 2, y: resizeBox.y + resizeBox.height / 2 };
	await resizeControl.dispatchEvent("pointerdown", {
		bubbles: true,
		button: 0,
		buttons: 1,
		clientX: resizeStart.x,
		clientY: resizeStart.y,
		pointerId: 1,
		pointerType: "mouse",
	});
	await page.evaluate(({ x, y }) => {
		window.dispatchEvent(
			new PointerEvent("pointermove", {
				bubbles: true,
				buttons: 1,
				clientX: x + 80,
				clientY: y + 60,
				pointerId: 1,
				pointerType: "mouse",
			}),
		);
		window.dispatchEvent(
			new PointerEvent("pointerup", {
				bubbles: true,
				clientX: x + 80,
				clientY: y + 60,
				pointerId: 1,
				pointerType: "mouse",
			}),
		);
	}, resizeStart);
	await page.waitForTimeout(350);
	const resizedBox = await commentNode.boundingBox();
	if (!resizedBox) throw new Error("Resized comment node is not visible.");
	expect(resizedBox.width - movedBox.width).toBeGreaterThan(60);
	expect(resizedBox.height - movedBox.height).toBeGreaterThan(40);
	await page.keyboard.press("Control+z");
	await expectCommentBoxNear(commentNode, movedBox);
	await page.keyboard.press("Control+y");
	await expectCommentBoxNear(commentNode, resizedBox);

	const amber = page.getByRole("button", { name: "Set comment color to Amber" });
	const blue = page.getByRole("button", { name: "Set comment color to Blue" });
	await blue.dispatchEvent("click", { bubbles: true });
	await page.waitForTimeout(350);
	await expect(blue).toHaveAttribute("aria-pressed", "true");
	await page.keyboard.press("Control+z");
	await expect(amber).toHaveAttribute("aria-pressed", "true");
	await page.keyboard.press("Control+y");
	await expect(blue).toHaveAttribute("aria-pressed", "true");
	const commentEditor = page.getByPlaceholder("Write a note...");
	await expect(commentEditor).toHaveCSS("font-size", "14px");
	await page.getByRole("button", { name: "Increase comment font size" }).dispatchEvent("click", { bubbles: true });
	await page.waitForTimeout(350);
	await expect(commentEditor).toHaveCSS("font-size", "15px");
	await page.keyboard.press("Control+z");
	await expect(commentEditor).toHaveCSS("font-size", "14px");
	await page.keyboard.press("Control+y");
	await expect(commentEditor).toHaveCSS("font-size", "15px");

	await page.getByRole("button", { name: "Delete comment" }).dispatchEvent("click", { bubbles: true });
	await expect(commentNode).toHaveCount(0);
	await page.keyboard.press("Control+z");
	await expect(commentNode).toHaveCount(1);
	await page.keyboard.press("Control+y");
	await expect(commentNode).toHaveCount(0);
});

test("one multi-node drag is restored as one history transaction", async ({ page }) => {
	await createProject(page, "Multi-node drag history");
	const pane = page.locator(".react-flow__pane");
	const paneBox = await pane.boundingBox();
	if (!paneBox) throw new Error("React Flow pane is not visible.");

	const addNode = async (name: string, x: number, y: number) => {
		await page.mouse.click(x, y, { button: "right" });
		const browser = page.getByRole("dialog", { name: "Add node" });
		await browser.getByRole("textbox", { name: "Search nodes" }).fill(name);
		await browser.getByRole("button", { name: new RegExp(name) }).click();
	};
	await addNode("Log", paneBox.x + paneBox.width * 0.3, paneBox.y + paneBox.height / 2);
	await addNode("HTTP Request", paneBox.x + paneBox.width * 0.65, paneBox.y + paneBox.height / 2);

	const logNode = page.locator(".react-flow__node").filter({ hasText: "Log" }).first();
	const httpNode = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" }).first();
	const initialLogBox = await logNode.boundingBox();
	const initialHttpBox = await httpNode.boundingBox();
	if (!initialLogBox || !initialHttpBox) throw new Error("Graph nodes are not visible.");
	const selectionStart = {
		x: Math.min(initialLogBox.x, initialHttpBox.x) - 12,
		y: Math.min(initialLogBox.y, initialHttpBox.y) - 12,
	};
	const selectionEnd = {
		x: Math.max(initialLogBox.x + initialLogBox.width, initialHttpBox.x + initialHttpBox.width) + 12,
		y: Math.max(initialLogBox.y + initialLogBox.height, initialHttpBox.y + initialHttpBox.height) + 12,
	};
	await page.keyboard.down("Control");
	await page.mouse.move(selectionStart.x, selectionStart.y);
	await page.mouse.down();
	await page.mouse.move(selectionEnd.x, selectionEnd.y, { steps: 10 });
	await page.mouse.up();
	await page.keyboard.up("Control");
	await expect(page.locator(".react-flow__node.selected")).toHaveCount(2);
	const groupSelection = page.locator(".react-flow__nodesselection-rect");
	await expect(groupSelection).toBeVisible();
	await expect(groupSelection).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
	await expect(groupSelection).toHaveCSS("border-top-width", "0px");

	const dragStart = { x: initialLogBox.x + initialLogBox.width / 2, y: initialLogBox.y + 24 };
	await page.mouse.move(dragStart.x, dragStart.y);
	await page.mouse.down();
	await page.mouse.move(dragStart.x + 110, dragStart.y + 75, { steps: 10 });
	await page.mouse.up();
	await page.waitForTimeout(350);
	const movedLogBox = await logNode.boundingBox();
	const movedHttpBox = await httpNode.boundingBox();
	if (!movedLogBox || !movedHttpBox) throw new Error("Moved graph nodes are not visible.");
	expect(movedLogBox.x - initialLogBox.x).toBeGreaterThan(90);
	expect(movedHttpBox.x - initialHttpBox.x).toBeGreaterThan(90);
	expect(movedLogBox.y - initialLogBox.y).toBeGreaterThan(55);
	expect(movedHttpBox.y - initialHttpBox.y).toBeGreaterThan(55);

	await page.keyboard.press("Control+z");
	await expectNodePositionNear(logNode, initialLogBox);
	await expectNodePositionNear(httpNode, initialHttpBox);
	await page.keyboard.press("Control+y");
	await expectNodePositionNear(logNode, movedLogBox);
	await expectNodePositionNear(httpNode, movedHttpBox);
});

test("document history stays bounded to the most recent 100 transactions", async ({ page }) => {
	test.setTimeout(60_000);
	await createProject(page, "Bounded history");
	await page.getByRole("textbox", { name: "Search blocks" }).fill("Log");
	const addLog = page.getByRole("button", { name: /^Log/ });
	for (let index = 0; index < 105; index += 1) {
		await addLog.click();
	}
	const logNodes = page.locator(".react-flow__node").filter({ hasText: "Log" });
	await expect(logNodes).toHaveCount(105);

	for (let index = 0; index < 100; index += 1) {
		await page.keyboard.press("Control+z");
	}
	await expect(logNodes).toHaveCount(5);
	await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();
});
