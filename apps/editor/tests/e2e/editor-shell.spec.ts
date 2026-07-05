import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
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
	await expect(page.getByRole("menu", { name: "Canvas actions" })).toBeVisible();
	const pasteMenuItem = page.getByRole("menuitem", { name: "Paste" });
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

	await page.reload();
	await page.locator('input[type="file"]').setInputFiles(packagePath);

	await expect(page.getByPlaceholder("Write a note...")).toHaveValue("Round-trip comment");
	await expect(page.getByText("Import verified:")).toBeVisible();
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
	const exampleRoot = join(process.cwd(), "..", "..", "examples", "hello-log");
	const zip = new JSZip();

	for (const fileName of readdirSync(exampleRoot)) {
		zip.file(fileName, readFileSync(join(exampleRoot, fileName)));
	}
	zip.file("assets/orphan.txt", "orphaned asset content");
	writeFileSync(packagePath, await zip.generateAsync({ type: "nodebuffer" }));

	await page.goto("/");
	await page.locator('input[type="file"]').setInputFiles(packagePath);

	await expect(page.getByRole("heading", { name: "Import Rejected" })).toBeVisible();
	await expect(page.getByText("assets/orphan.txt: asset file is not declared in manifest.json assets.")).toBeVisible();
});

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
