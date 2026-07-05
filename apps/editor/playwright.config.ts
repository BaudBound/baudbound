import { defineConfig, devices } from "@playwright/test";

const e2eServerCommand =
	process.platform === "win32" ? "pnpm start --hostname 127.0.0.1 --port 3100" : "node .next/standalone/server.js";

export default defineConfig({
	testDir: "./tests/e2e",
	timeout: 30_000,
	expect: {
		timeout: 10_000,
	},
	use: {
		baseURL: "http://127.0.0.1:3100",
		trace: "retain-on-failure",
	},
	projects: [
		{
			name: "chromium",
			use: { ...devices["Desktop Chrome"] },
		},
		{
			name: "firefox",
			use: { ...devices["Desktop Firefox"] },
		},
	],
	webServer: {
		command: e2eServerCommand,
		env: {
			HOSTNAME: "127.0.0.1",
			PORT: "3100",
		},
		reuseExistingServer: !process.env.CI,
		timeout: 60_000,
		url: "http://127.0.0.1:3100",
	},
});
