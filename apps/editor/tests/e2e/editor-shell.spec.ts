import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import JSZip from "jszip";

test("editor shell loads the core controls", async ({ page }) => {
	await page.goto("/");

	await expect(page.getByText("BaudBound Editor", { exact: true })).toBeVisible();
	await expect(page.getByRole("button", { name: "Assets" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Project Settings" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Help" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Simulate" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Verify" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Import" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Export" })).toBeVisible();
	await expect(page.getByRole("textbox", { name: "Search blocks" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Properties" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Simulator" })).toBeVisible();
});

test("help modal exposes controls, references, expressions, and node docs", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Help" }).click();
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

	await page.getByRole("button", { name: "Project Settings" }).click();
	await expect(page.getByRole("heading", { name: "Project Settings" })).toBeVisible();

	await page.getByRole("button", { name: "Target runtime" }).click();
	await page.getByRole("option", { name: "Windows Desktop" }).click();
	await page.getByRole("button", { name: "Save Settings" }).click();

	await expect(page.getByText("Desktop", { exact: true })).toBeVisible();
	await expect(page.getByText("Not verified", { exact: true })).toBeVisible();
});

test("verification reports graph errors when the script has no trigger", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Verify" }).click();

	await expect(page.getByRole("heading", { name: "Verification" })).toBeVisible();
	await expect(page.getByText("No trigger node found. Add at least one trigger before export.")).toBeVisible();
	await expect(page.getByText("Verification failed", { exact: true })).toBeVisible();
});

test("manual trigger creation is limited to one node", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Manual" }).click();
	await page.getByRole("button", { name: "Manual" }).click();

	await expect(page.getByRole("heading", { name: "Manual Trigger Already Exists" })).toBeVisible();
	await expect(page.getByText("Remove the existing Manual Trigger before adding another one.")).toBeVisible();
});

test("asset editor shows content checks without fixed size caps", async ({ page }) => {
	await page.goto("/");

	await page.getByRole("button", { name: "Assets" }).click();

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
