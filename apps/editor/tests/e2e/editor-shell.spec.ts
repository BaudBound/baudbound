import { readFileSync, writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import JSZip from "jszip";

test("editor shell loads the core controls", async ({ page }) => {
	await page.goto("/");

	await expect(page.getByText("BaudBound Editor", { exact: true })).toBeVisible();
	await expect(page.getByRole("button", { name: "Open asset editor" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Open project settings" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Open help" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Verify script" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Import package" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Export package" })).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Search blocks" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Properties" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Simulator" })).toBeVisible();
	await page.getByRole("button", { name: "Simulator" }).click();
	await expect(page.getByRole("button", { name: "Stop simulation" })).toBeVisible();
});

test("panel collapse state persists across editor reloads", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Collapse block library" }).click();
	await page.getByRole("button", { name: "Collapse inspector" }).click();
	await page.getByRole("button", { name: "Collapse bottom panel" }).click();

	await expect(page.getByRole("button", { name: "Expand block library" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand inspector" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand bottom panel" })).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Search blocks" })).toBeHidden();
	await expect(page.getByRole("button", { name: "Properties" })).toBeHidden();
	await expect(page.getByRole("button", { name: "Import package" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Export package" })).toBeVisible();

	const storedState = await page.evaluate(() =>
		JSON.parse(window.localStorage.getItem("baudbound.editor.panel-collapsed.v1") ?? "null"),
	);
	expect(storedState).toEqual({ left: true, right: true, bottom: true });

	await page.reload();

	await expect(page.getByRole("button", { name: "Expand block library" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand inspector" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Expand bottom panel" })).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Search blocks" })).toBeHidden();
	await expect(page.getByRole("button", { name: "Properties" })).toBeHidden();
});

test("help modal exposes controls, references, expressions, and node docs", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Open help" }).click();
	await expect(page.getByRole("heading", { name: "Editor Help" })).toBeVisible();
	await expect(page.getByRole("heading", { name: "Hotkeys" })).toBeVisible();

	await page.getByRole("button", { name: "References" }).click();
	await expect(page.getByRole("heading", { name: "Reference Formats" })).toBeVisible();
	await expect(page.getByText("{{node-id.output_name}}")).toBeVisible();

	await page.getByRole("button", { name: "Expressions" }).click();
	await expect(page.getByRole("heading", { name: "Calculate Node" })).toBeVisible();
	await expect(page.getByText("round(value)")).toBeVisible();
	await expect(page.getByText("^", { exact: true })).toBeVisible();

	await page.getByRole("button", { name: "Nodes" }).click();
	await expect(page.getByRole("heading", { name: "Node Reference" })).toBeVisible();
	await expect(page.getByText("Send an HTTP request.")).toBeVisible();
});

test("project settings target runtime can be changed with the combobox", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Open project settings" }).click();
	await expect(page.getByRole("heading", { name: "Project Settings" })).toBeVisible();

	await page.getByRole("button", { name: "Target runtime" }).click();
	await page.getByRole("option", { name: "Windows Desktop" }).click();
	await page.getByRole("button", { name: "Save Settings" }).click();

	await expect(page.getByText("Desktop", { exact: true })).toBeVisible();
	await expect(page.getByText("Not verified", { exact: true })).toBeVisible();
});

test("verification reports graph errors when the script has no trigger", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Verify script" }).click();

	await expect(page.getByRole("heading", { name: "Verification" })).toBeVisible();
	await expect(page.getByText("No trigger node found. Add at least one trigger before export.")).toBeVisible();
	await expect(page.getByText("3 failed checks must be resolved.", { exact: true }).first()).toBeVisible();
});

test("manual trigger creation is limited to one node", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("button", { name: "Manual" }).click();

	await expect(page.getByRole("heading", { name: "Manual Trigger Already Exists" })).toBeVisible();
	await expect(page.getByText("Remove the existing Manual Trigger before adding another one.")).toBeVisible();
});

test("verification warns for medium risk nodes", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("textbox", { name: "Search blocks" }).fill("Clipboard");
	await page.getByRole("button", { name: /Clipboard medium/ }).click();
	await page.getByRole("button", { name: "Verify script" }).click();

	await expect(page.getByRole("heading", { name: "Verification" })).toBeVisible();
	await expect(page.getByText("1 medium-or-higher risk permission requires review.")).toBeVisible();
	await expect(page.getByText("1 warning should be reviewed.").first()).toBeVisible();
	await expect(page.getByText("Warning", { exact: true })).toBeVisible();
});

test("comment text editing preserves caret position", async ({ page }) => {
	await page.goto("/");

	await page.getByTitle("Add comment").click();
	const commentEditor = page.getByPlaceholder("Write a note...");
	await expect(commentEditor).toHaveCSS("font-size", "14px");
	await page.getByRole("button", { name: "Increase comment font size" }).click();
	await expect(commentEditor).toHaveCSS("font-size", "15px");
	const fontSizeInput = page.getByRole("textbox", { name: "Comment font size" });
	await expect(fontSizeInput).toHaveValue("15");
	await fontSizeInput.fill("48");
	await fontSizeInput.press("Enter");
	await expect(commentEditor).toHaveCSS("font-size", "48px");
	await expect(fontSizeInput).toHaveValue("48");
	await fontSizeInput.fill("999");
	await fontSizeInput.press("Enter");
	await expect(commentEditor).toHaveCSS("font-size", "72px");
	await expect(fontSizeInput).toHaveValue("72");
	await fontSizeInput.fill("1");
	await fontSizeInput.press("Enter");
	await expect(commentEditor).toHaveCSS("font-size", "12px");
	await expect(fontSizeInput).toHaveValue("12");
	await fontSizeInput.fill("abc");
	await fontSizeInput.press("Enter");
	await expect(commentEditor).toHaveCSS("font-size", "12px");
	await expect(fontSizeInput).toHaveValue("12");

	const colorControls = page.getByRole("button", { name: /Set comment color to/ });
	await expect(colorControls).toHaveCount(5);
	for (const control of await colorControls.all()) {
		const box = await control.boundingBox();
		if (!box) {
			throw new Error("Comment color control is not visible.");
		}
		expect(box.width).toBeGreaterThanOrEqual(24);
		expect(box.height).toBeGreaterThanOrEqual(24);
	}
	const decreaseFontControl = page.getByRole("button", { name: "Decrease comment font size" });
	const decreaseFontBox = await decreaseFontControl.boundingBox();
	const firstColorBox = await colorControls.first().boundingBox();
	const secondColorBox = await colorControls.nth(1).boundingBox();
	if (!decreaseFontBox || !firstColorBox || !secondColorBox) {
		throw new Error("Comment controls are not visible.");
	}
	expect(decreaseFontBox.x).toBeLessThan(firstColorBox.x);

	const commentNode = page.locator(".baud-comment-flow-node");
	const commentNodeBeforeGapDrag = await commentNode.boundingBox();
	if (!commentNodeBeforeGapDrag) {
		throw new Error("Comment node is not visible.");
	}
	const colorGap = {
		x: (firstColorBox.x + firstColorBox.width + secondColorBox.x) / 2,
		y: firstColorBox.y + firstColorBox.height / 2,
	};
	await page.mouse.move(colorGap.x, colorGap.y);
	await page.mouse.down();
	await page.mouse.move(colorGap.x + 48, colorGap.y + 32);
	await page.mouse.up();
	const commentNodeAfterGapDrag = await commentNode.boundingBox();
	if (!commentNodeAfterGapDrag) {
		throw new Error("Comment node is not visible after using the color controls.");
	}
	expect(commentNodeAfterGapDrag.x).toBeCloseTo(commentNodeBeforeGapDrag.x, 5);
	expect(commentNodeAfterGapDrag.y).toBeCloseTo(commentNodeBeforeGapDrag.y, 5);

	const blueControl = page.getByRole("button", { name: "Set comment color to Blue" });
	await blueControl.click();
	await expect(blueControl).toHaveAttribute("aria-pressed", "true");
	await expect(page.locator(".baud-comment-drag-handle").getByText("Comment", { exact: true })).toHaveCSS(
		"font-size",
		"18px",
	);

	for (const controlName of ["Decrease comment font size", "Increase comment font size", "Delete comment"]) {
		const box = await page.getByRole("button", { name: controlName }).boundingBox();
		if (!box) {
			throw new Error(`${controlName} control is not visible.`);
		}
		const minimumSize = controlName === "Delete comment" ? 36 : 32;
		expect(box.width).toBeGreaterThanOrEqual(minimumSize);
		expect(box.height).toBeGreaterThanOrEqual(minimumSize);
	}
	const deleteIconBox = await page.getByRole("button", { name: "Delete comment" }).locator("svg").boundingBox();
	if (!deleteIconBox) {
		throw new Error("Delete comment icon is not visible.");
	}
	expect(deleteIconBox.width).toBeGreaterThanOrEqual(24);
	expect(deleteIconBox.height).toBeGreaterThanOrEqual(24);
	const resizeControlBox = await page.getByRole("button", { name: "Resize comment" }).boundingBox();
	if (!resizeControlBox) {
		throw new Error("Resize comment control is not visible.");
	}
	expect(resizeControlBox.width).toBeGreaterThanOrEqual(24);
	expect(resizeControlBox.height).toBeGreaterThanOrEqual(24);

	await commentEditor.fill("abcdef");
	await commentEditor.evaluate((element) => {
		if (!(element instanceof HTMLTextAreaElement)) {
			throw new Error("Comment editor is not a textarea.");
		}

		element.setSelectionRange(3, 3);
	});
	await commentEditor.pressSequentially("XYZ");

	await expect(commentEditor).toHaveValue("abcXYZdef");
});

test("comment nodes support node context menu actions", async ({ page }) => {
	await page.goto("/");

	await page.getByTitle("Add comment").click();
	const commentEditors = page.getByPlaceholder("Write a note...");
	await commentEditors.first().fill("Comment menu note");
	await commentEditors.first().evaluate((element) => {
		if (!(element instanceof HTMLTextAreaElement)) {
			throw new Error("Comment editor is not a textarea.");
		}

		element.blur();
	});

	const commentHandles = page.locator(".baud-comment-drag-handle");
	await commentHandles.first().click({ button: "right" });
	const nodeMenu = page.getByRole("menu", { name: "Node actions" });
	await expect(nodeMenu).toBeVisible();
	await page.getByRole("menuitem", { name: /^Copy$/ }).click();
	await expect(nodeMenu).toBeHidden();

	const paneBox = await page.locator(".react-flow__pane").boundingBox();
	if (!paneBox) {
		throw new Error("React Flow pane is not visible.");
	}

	await page.locator(".react-flow__pane").dispatchEvent("contextmenu", {
		bubbles: true,
		button: 2,
		cancelable: true,
		clientX: paneBox.x + paneBox.width - 96,
		clientY: paneBox.y + 96,
	});
	const canvasNodeMenu = page.getByRole("dialog", { name: "Add node" });
	await expect(canvasNodeMenu).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Search nodes" })).toBeFocused();
	const pasteMenuItem = page.getByRole("button", { name: "Paste copied node" });
	await expect(pasteMenuItem).toBeEnabled();
	await pasteMenuItem.click();

	await expect(commentEditors).toHaveCount(2);
	await expect(commentEditors.nth(1)).toHaveValue("Comment menu note");

	await commentHandles.first().click({ button: "right" });
	await page.getByRole("menuitem", { name: "Duplicate" }).click();
	await expect(commentEditors).toHaveCount(3);

	await commentHandles.first().click({ button: "right" });
	await page.getByRole("menuitem", { name: "Delete" }).click();
	await expect(commentEditors).toHaveCount(2);
});

test("empty canvas context menu searches and adds nodes", async ({ page }) => {
	await page.goto("/");

	const pane = page.locator(".react-flow__pane");
	const paneBox = await pane.boundingBox();
	if (!paneBox) {
		throw new Error("React Flow pane is not visible.");
	}

	await page.mouse.click(paneBox.x + 100, paneBox.y + paneBox.height / 2, { button: "right" });

	const nodeBrowser = page.getByRole("dialog", { name: "Add node" });
	await expect(nodeBrowser).toBeVisible();
	await expect(nodeBrowser.getByRole("button", { name: "Paste copied node" })).toBeDisabled();
	await expect(nodeBrowser.getByText("Triggers", { exact: true })).toBeVisible();
	await expect(nodeBrowser.getByText("Control Flow", { exact: true })).toBeVisible();

	const search = page.getByRole("textbox", { name: "Search nodes" });
	await search.fill("HTTP Request");
	await nodeBrowser.getByRole("button", { name: /HTTP Request/ }).click();

	await expect(nodeBrowser).toBeHidden();
	const httpNodes = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" });
	await expect(httpNodes).toHaveCount(1);

	await httpNodes.first().click({ button: "right" });
	await page.getByRole("menuitem", { name: /^Copy$/ }).click();
	await pane.dispatchEvent("contextmenu", {
		bubbles: true,
		button: 2,
		cancelable: true,
		clientX: paneBox.x + paneBox.width - 80,
		clientY: paneBox.y + paneBox.height - 80,
	});
	await page.getByRole("button", { name: "Paste copied node" }).click();

	await expect(httpNodes).toHaveCount(2);
	const nodeIds = await httpNodes.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-id")));
	expect(new Set(nodeIds).size).toBe(2);
	for (const nodeId of nodeIds) {
		expect(nodeId).toMatch(/^n-[a-z0-9]+(?:-[a-z0-9]+)?$/);
		expect(nodeId?.length).toBeLessThanOrEqual(16);
	}
});

test("keyboard paste uses the canvas pointer and falls back to the canvas center", async ({ page }) => {
	await page.goto("/");

	const pane = page.locator(".react-flow__pane");
	const paneBox = await pane.boundingBox();
	if (!paneBox) {
		throw new Error("React Flow pane is not visible.");
	}

	await page.mouse.click(paneBox.x + 100, paneBox.y + paneBox.height / 2, { button: "right" });
	await page.getByRole("textbox", { name: "Search nodes" }).fill("HTTP Request");
	await page
		.getByRole("dialog", { name: "Add node" })
		.getByRole("button", { name: /HTTP Request/ })
		.click();
	const httpNodes = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" });
	await expect(httpNodes).toHaveCount(1);
	const sourceNodeId = await httpNodes.first().getAttribute("data-id");
	if (!sourceNodeId) {
		throw new Error("Source HTTP Request node has no id.");
	}
	await page.keyboard.press("Control+c");

	const pointerTarget = await page.evaluate(() => {
		const paneElement = document.querySelector(".react-flow__pane");
		if (!(paneElement instanceof HTMLElement)) {
			return null;
		}

		const bounds = paneElement.getBoundingClientRect();
		for (let y = bounds.bottom - 80; y >= bounds.top + 80; y -= 40) {
			for (let x = bounds.right - 80; x >= bounds.left + 160; x -= 40) {
				const pointedElement = document.elementFromPoint(x, y);
				if (pointedElement?.closest(".react-flow__pane, .react-flow__background")) {
					return { x, y };
				}
			}
		}

		return null;
	});
	if (!pointerTarget) {
		throw new Error("No unobstructed canvas position is available for pointer paste.");
	}
	await page.mouse.move(pointerTarget.x - 20, pointerTarget.y - 20);
	await page.mouse.move(pointerTarget.x, pointerTarget.y);
	await page.keyboard.press("Control+v");

	await expect(httpNodes).toHaveCount(2);
	const pointerPasteNodeId = (await httpNodes.evaluateAll(
		(nodes, sourceId) => nodes.map((node) => node.getAttribute("data-id")).find((nodeId) => nodeId !== sourceId),
		sourceNodeId,
	)) as string | undefined;
	if (!pointerPasteNodeId) {
		throw new Error("Pointer-positioned HTTP Request node has no id.");
	}
	const pointerPasteBox = await page.locator(`.react-flow__node[data-id="${pointerPasteNodeId}"]`).boundingBox();
	if (!pointerPasteBox) {
		throw new Error("Pasted HTTP Request node is not visible.");
	}
	expect(Math.abs(pointerPasteBox.x + pointerPasteBox.width / 2 - pointerTarget.x)).toBeLessThan(4);
	expect(Math.abs(pointerPasteBox.y + pointerPasteBox.height / 2 - pointerTarget.y)).toBeLessThan(40);

	await page.mouse.move(8, 8);
	await page.keyboard.press("Control+v");

	await expect(httpNodes).toHaveCount(3);
	const centeredPasteNodeId = (await httpNodes.evaluateAll(
		(nodes, existingIds) =>
			nodes
				.map((node) => node.getAttribute("data-id"))
				.find((nodeId) => nodeId !== existingIds.source && nodeId !== existingIds.pointerPaste),
		{ pointerPaste: pointerPasteNodeId, source: sourceNodeId },
	)) as string | undefined;
	if (!centeredPasteNodeId) {
		throw new Error("Centered HTTP Request node has no id.");
	}
	const centeredPasteBox = await page.locator(`.react-flow__node[data-id="${centeredPasteNodeId}"]`).boundingBox();
	if (!centeredPasteBox) {
		throw new Error("Centered HTTP Request node is not visible.");
	}
	expect(Math.abs(centeredPasteBox.x + centeredPasteBox.width / 2 - (paneBox.x + paneBox.width / 2))).toBeLessThan(4);
	expect(Math.abs(centeredPasteBox.y + centeredPasteBox.height / 2 - (paneBox.y + paneBox.height / 2))).toBeLessThan(
		40,
	);
});

test("copy and paste preserves a selected graph fragment", async ({ page }) => {
	await page.goto("/");

	const pane = page.locator(".react-flow__pane");
	const paneBox = await pane.boundingBox();
	if (!paneBox) {
		throw new Error("React Flow pane is not visible.");
	}
	const addNode = async (name: string, x: number, y: number) => {
		await page.mouse.click(x, y, { button: "right" });
		const browser = page.getByRole("dialog", { name: "Add node" });
		await browser.getByRole("textbox", { name: "Search nodes" }).fill(name);
		await browser.getByRole("button", { name: new RegExp(name) }).click();
	};

	await addNode("Log", paneBox.x + 100, paneBox.y + paneBox.height / 2);
	await addNode("HTTP Request", paneBox.x + paneBox.width - 100, paneBox.y + paneBox.height / 2);

	const logNodes = page.locator(".react-flow__node").filter({ hasText: "Log" });
	const httpNodes = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" });
	const originalLogId = await logNodes.first().getAttribute("data-id");
	const originalHttpId = await httpNodes.first().getAttribute("data-id");
	if (!originalLogId || !originalHttpId) {
		throw new Error("Source graph nodes have no ids.");
	}

	await logNodes.first().locator(".react-flow__handle.source").first().dispatchEvent("click", { bubbles: true });
	await httpNodes.first().locator(".react-flow__handle.target").first().dispatchEvent("click", { bubbles: true });
	const edges = page.locator(".react-flow__edge");
	await expect(edges).toHaveCount(1);

	await logNodes.first().dispatchEvent("click", { bubbles: true });
	await page.keyboard.down("Control");
	await httpNodes.first().dispatchEvent("click", { bubbles: true, ctrlKey: true });
	await edges.first().dispatchEvent("click", { bubbles: true, ctrlKey: true });
	await page.keyboard.up("Control");
	await expect(page.locator(".react-flow__node.selected")).toHaveCount(2);
	await expect(page.locator(".react-flow__edge.selected")).toHaveCount(1);

	const originalLogBox = await page.locator(`.react-flow__node[data-id="${originalLogId}"]`).boundingBox();
	const originalHttpBox = await page.locator(`.react-flow__node[data-id="${originalHttpId}"]`).boundingBox();
	if (!originalLogBox || !originalHttpBox) {
		throw new Error("Source graph nodes are not visible.");
	}
	expect(Math.abs(originalHttpBox.x - originalLogBox.x)).toBeGreaterThan(100);
	await page.keyboard.press("Control+c");
	await page.mouse.move(paneBox.x + paneBox.width / 2, paneBox.y + paneBox.height - 80);
	await page.keyboard.press("Control+v");

	await expect(logNodes).toHaveCount(2);
	await expect(httpNodes).toHaveCount(2);
	await expect(edges).toHaveCount(2);
	const copiedLogId = (await logNodes.evaluateAll(
		(nodes, sourceId) => nodes.map((node) => node.getAttribute("data-id")).find((id) => id !== sourceId),
		originalLogId,
	)) as string | undefined;
	const copiedHttpId = (await httpNodes.evaluateAll(
		(nodes, sourceId) => nodes.map((node) => node.getAttribute("data-id")).find((id) => id !== sourceId),
		originalHttpId,
	)) as string | undefined;
	if (!copiedLogId || !copiedHttpId) {
		throw new Error("Copied graph nodes have no ids.");
	}

	const copiedLogBox = await page.locator(`.react-flow__node[data-id="${copiedLogId}"]`).boundingBox();
	const copiedHttpBox = await page.locator(`.react-flow__node[data-id="${copiedHttpId}"]`).boundingBox();
	if (!copiedLogBox || !copiedHttpBox) {
		throw new Error("Copied graph nodes are not visible.");
	}
	expect(copiedHttpBox.x - copiedLogBox.x).toBeCloseTo(originalHttpBox.x - originalLogBox.x, 5);
	expect(copiedHttpBox.y - copiedLogBox.y).toBeCloseTo(originalHttpBox.y - originalLogBox.y, 5);

	const selectedNodeIds = await page
		.locator(".react-flow__node.selected")
		.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-id")));
	expect(new Set(selectedNodeIds)).toEqual(new Set([copiedLogId, copiedHttpId]));
	await expect(page.locator(".react-flow__edge.selected")).toHaveCount(1);
	await expect(page.getByRole("group", { name: `Edge from ${copiedLogId} to ${copiedHttpId}` })).toHaveCount(1);
});

test("a node cannot connect its output to its own input", async ({ page }) => {
	await page.goto("/");

	const pane = page.locator(".react-flow__pane");
	const paneBox = await pane.boundingBox();
	if (!paneBox) {
		throw new Error("React Flow pane is not visible.");
	}

	await page.mouse.click(paneBox.x + paneBox.width / 2, paneBox.y + paneBox.height / 2, { button: "right" });
	const browser = page.getByRole("dialog", { name: "Add node" });
	await browser.getByRole("textbox", { name: "Search nodes" }).fill("Log");
	await browser.getByRole("button", { name: /^Log/ }).click();

	const logNode = page.locator(".react-flow__node").filter({ hasText: "Log" }).first();
	await logNode.locator(".react-flow__handle.source").first().dispatchEvent("click", { bubbles: true });
	await logNode.locator(".react-flow__handle.target").first().dispatchEvent("click", { bubbles: true });

	await expect(page.locator(".react-flow__edge")).toHaveCount(0);
});

test("fan-out execution order can be changed from the edge inspector", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("textbox", { name: "Search blocks" }).fill("Log");
	await page.getByRole("button", { name: /^Log/ }).click();
	await page.getByRole("textbox", { name: "Search blocks" }).fill("HTTP Request");
	await page.getByRole("button", { name: /HTTP Request/ }).click();

	const manualNode = page.locator(".react-flow__node").filter({ hasText: "Manual" }).first();
	const logNode = page.locator(".react-flow__node").filter({ hasText: "Log" }).first();
	const httpNode = page.locator(".react-flow__node").filter({ hasText: "HTTP Request" }).first();
	const sourceHandle = manualNode.locator(".react-flow__handle.source").first();
	const manualNodeId = await manualNode.getAttribute("data-id");
	const httpNodeId = await httpNode.getAttribute("data-id");
	if (!manualNodeId || !httpNodeId) {
		throw new Error("Fan-out graph nodes have no ids.");
	}

	await sourceHandle.dispatchEvent("click", { bubbles: true });
	await logNode.locator(".react-flow__handle.target").first().dispatchEvent("click", { bubbles: true });
	await sourceHandle.dispatchEvent("click", { bubbles: true });
	await httpNode.locator(".react-flow__handle.target").first().dispatchEvent("click", { bubbles: true });

	await expect(page.locator(".react-flow__edge")).toHaveCount(2);
	await page.locator(".react-flow__edge").last().dispatchEvent("click", { bubbles: true });
	const orderList = page.getByRole("list", { name: /Execution order for Manual.*output out/ });
	await expect(orderList).toBeVisible();
	await expect(orderList.locator("li").nth(0)).toContainText("Log");
	await expect(orderList.locator("li").nth(1)).toContainText("HTTP Request");
	await expect(page.locator(".react-flow__edge-text")).toHaveText(["1", "2"]);

	await page.getByRole("button", { name: "Move HTTP Request earlier" }).click();
	await expect(orderList.locator("li").nth(0)).toContainText("HTTP Request");
	await expect(orderList.locator("li").nth(1)).toContainText("Log");
	await expect(page.locator(".react-flow__edge-text")).toHaveText(["2", "1"]);
});

test("asset editor shows content checks without fixed size caps", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Open asset editor" }).click();

	await expect(page.getByRole("heading", { name: "Asset Editor" })).toBeVisible();
	await expect(page.getByText("Package checks")).toBeVisible();
	await expect(page.getByText("No fixed editor cap")).toBeVisible();
	await expect(page.getByText("content signature")).toBeVisible();
});

test("hostile package import is rejected before loading", async ({ page }, testInfo) => {
	const packagePath = testInfo.outputPath("malformed-package.bbs");
	const zip = new JSZip();
	zip.file("manifest.json", JSON.stringify({ name: "malformed" }));
	zip.file("program.json", "{not valid json");
	zip.file("assets/../evil.txt", "evil");
	writeFileSync(packagePath, await zip.generateAsync({ type: "nodebuffer" }));

	await page.goto("/");
	await page.locator('input[type="file"]').setInputFiles(packagePath);

	await expect(page.getByRole("heading", { name: "Import Rejected" })).toBeVisible();
	await expect(
		page.getByText("The imported package did not pass verification cleanly and was not loaded."),
	).toBeVisible();
	await expect(page.getByText("Package JSON")).toBeVisible();
});

test("exported package preserves editor metadata and imports back", async ({ page }, testInfo) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("button", { name: "Variables", exact: true }).click();
	await page.getByRole("button", { name: "Add variable" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill("counter");
	await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
	await page.getByRole("combobox", { name: "Scope" }).click();
	await page.getByRole("option", { name: "persistent" }).click();
	await page.getByRole("combobox", { name: "Type" }).click();
	await page.getByRole("option", { name: "number" }).click();
	await page.getByRole("textbox", { name: "Default value" }).fill("10");
	await page.getByRole("button", { name: "Save", exact: true }).click();
	await page.getByTitle("Add comment").click();
	await page.getByPlaceholder("Write a note...").fill("Round-trip comment");
	await page.getByRole("button", { name: "Edge style" }).click();
	await page.getByRole("option", { name: "Bezier" }).click();
	await page.getByRole("button", { name: "Export package" }).click();
	await expect(page.getByRole("heading", { name: "Export .bbs" })).toBeVisible();
	await page.getByRole("button", { name: "Next" }).click();
	await page.getByRole("button", { name: "Next" }).click();
	await expect(page.getByText("Verification passed. The download button is now available.")).toBeVisible();

	const downloadPromise = page.waitForEvent("download");
	await page.getByRole("button", { name: "Download .bbs" }).click();
	const download = await downloadPromise;
	const packagePath = testInfo.outputPath(download.suggestedFilename());
	await download.saveAs(packagePath);

	const zip = await JSZip.loadAsync(readFileSync(packagePath));
	const editorEntry = zip.file("editor.json");
	if (!editorEntry) {
		throw new Error("Exported package is missing editor.json.");
	}
	const editorJson = JSON.parse(await editorEntry.async("text"));
	assertEditorMetadata(editorJson);
	const manifestEntry = zip.file("manifest.json");
	if (!manifestEntry) {
		throw new Error("Exported package is missing manifest.json.");
	}
	const manifestJson = JSON.parse(await manifestEntry.async("text"));
	expect(manifestJson.variables).toEqual([
		{
			description: "",
			name: "counter",
			scope: "persistent",
			type: "number",
			value: 10,
		},
	]);

	await page.reload();
	await page.locator('input[type="file"]').setInputFiles(packagePath);

	await expect(page.getByPlaceholder("Write a note...")).toHaveValue("Round-trip comment");
	await expect(page.getByText("Import verified:")).toBeVisible();
	await page.getByRole("button", { name: "Variables", exact: true }).click();
	await expect(page.getByText("Default variables", { exact: true })).toBeVisible();
	await expect(page.getByText("counter", { exact: true }).last()).toBeVisible();
});

test("verification modal remains usable on a 1080p-height viewport", async ({ page }) => {
	await page.setViewportSize({ width: 1366, height: 768 });
	await page.goto("/");

	await page.getByRole("button", { name: "Verify script" }).click();

	await expect(page.getByRole("heading", { name: "Verification" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Close verification" })).toBeVisible();
	await page.getByRole("button", { name: "Close verification" }).click();
	await expect(page.getByRole("heading", { name: "Verification" })).toBeHidden();
});

test("package import rejects unmanifested asset files", async ({ page }, testInfo) => {
	const packagePath = testInfo.outputPath("unmanifested-asset-package.bbs");
	const zip = new JSZip();

	addMinimalPackageDocuments(zip);
	zip.file("assets/orphan.txt", "orphaned asset content");
	writeFileSync(packagePath, await zip.generateAsync({ type: "nodebuffer" }));

	await page.goto("/");
	await page.locator('input[type="file"]').setInputFiles(packagePath);

	await expect(page.getByRole("heading", { name: "Import Rejected" })).toBeVisible();
	await expect(page.getByText("assets/orphan.txt: asset file is not declared in manifest.json assets.")).toBeVisible();
});

function addMinimalPackageDocuments(zip: JSZip) {
	zip.file(
		"manifest.json",
		JSON.stringify({
			created_at: "2026-01-01T00:00:00.000Z",
			created_with: "BaudBound Editor Test",
			format_version: 1,
			id: "c86851ce-b2e5-4385-91cb-e0564040cdfb",
			minimum_runner_version: "2.0.0",
			name: "Asset Validation Test",
			script_language_version: 1,
		}),
	);
	const trigger = {
		action_type: "trigger.manual",
		config: {},
		id: "n-trigger",
		runtime_outputs: [],
		type: "manual",
	};
	zip.file(
		"program.json",
		JSON.stringify({
			entry: {
				program: {
					edges: [],
					execution_model: "directed_graph",
					runtime_context: {
						built_in_variables: { syntax: "{{variable_name}}", variables: [] },
						expression_reference: "{{node-id.data_name}}",
						node_outputs: [],
						template_reference: "{{node-id.data_name}}",
						variables: [],
					},
					steps: [],
					type: "block",
				},
				trigger,
				triggers: [trigger],
			},
		}),
	);
	zip.file("permissions.json", JSON.stringify({ declared_permissions: [], risk_level: "low" }));
	zip.file(
		"capabilities.json",
		JSON.stringify({ required_capabilities: ["trigger.manual"], target_runtime: "Generic Desktop" }),
	);
}

function assertEditorMetadata(editorJson: unknown) {
	expect(editorJson).toMatchObject({
		canvas: {
			edge_style: "bezier",
		},
		comments: [
			expect.objectContaining({
				text: "Round-trip comment",
			}),
		],
	});
}
