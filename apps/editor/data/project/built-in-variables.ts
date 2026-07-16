import type { ProjectSettings } from "@/lib/types";
import { DEFAULT_MINIMUM_RUNNER_VERSION } from "@/lib/version";
import type { EditorVariable } from "./variables";

export type BuiltInVariable = Omit<
	EditorVariable<BuiltInVariableValue | undefined>,
	"read_only" | "scope" | "source" | "value"
> & {
	description: string;
	example: string;
	runtimeBinding: string;
};

export type BuiltInVariableGroup = {
	id: BuiltInVariableScope;
	label: string;
	description: string;
	variables: BuiltInVariable[];
};

type BuiltInVariableScope = "manifest" | "system";
type BuiltInVariableValue = string | number;
type BuiltInVariableRuntimeEntry = BuiltInVariable &
	EditorVariable<BuiltInVariableValue | undefined> & {
		scope: BuiltInVariableScope;
		value?: BuiltInVariableValue;
	};

const MANIFEST_FORMAT_VERSION = 1;

const manifestValueResolvers: Record<string, (settings: ProjectSettings) => BuiltInVariableValue> = {
	manifest_name: (settings: ProjectSettings) => settings.name,
	manifest_version: () => MANIFEST_FORMAT_VERSION,
	manifest_author: (settings: ProjectSettings) => settings.author,
	manifest_description: (settings: ProjectSettings) => settings.description,
	manifest_website: (settings: ProjectSettings) => settings.website,
	manifest_repository: (settings: ProjectSettings) => settings.repository,
	manifest_minimum_runner_version: (settings: ProjectSettings) => settings.minimumRunnerVersion,
};

const systemRuntimeBindings = {
	system_os: "runner.system.os",
	system_arch: "runner.system.arch",
	system_hostname: "runner.system.hostname",
	system_user: "runner.system.user",
	system_locale: "runner.system.locale",
	system_timezone: "runner.system.timezone",
	system_date: "runner.system.date",
	system_time: "runner.system.time",
} satisfies Record<string, string>;

export const builtInVariableGroups: BuiltInVariableGroup[] = [
	{
		id: "manifest",
		label: "Manifest",
		description: "Values from the script manifest and project settings.",
		variables: [
			{
				name: "manifest_name",
				token: "{{manifest_name}}",
				type: "string",
				description: "Current script name from project settings.",
				example: "server-health-check",
				runtimeBinding: "manifest.name",
			},
			{
				name: "manifest_version",
				token: "{{manifest_version}}",
				type: "number",
				description: "Package version written to exported scripts.",
				example: "1",
				runtimeBinding: "manifest.format_version",
			},
			{
				name: "manifest_author",
				token: "{{manifest_author}}",
				type: "string",
				description: "Author from project settings.",
				example: "NATroutter",
				runtimeBinding: "manifest.author",
			},
			{
				name: "manifest_description",
				token: "{{manifest_description}}",
				type: "string",
				description: "Description from project settings.",
				example: "Checks server health and reports status.",
				runtimeBinding: "manifest.description",
			},
			{
				name: "manifest_website",
				token: "{{manifest_website}}",
				type: "string",
				description: "Project website from project settings.",
				example: "https://example.com",
				runtimeBinding: "manifest.website",
			},
			{
				name: "manifest_repository",
				token: "{{manifest_repository}}",
				type: "string",
				description: "Repository URL from project settings.",
				example: "https://github.com/example/script",
				runtimeBinding: "manifest.repository",
			},
			{
				name: "manifest_minimum_runner_version",
				token: "{{manifest_minimum_runner_version}}",
				type: "string",
				description: "Minimum runner version required by the package.",
				example: DEFAULT_MINIMUM_RUNNER_VERSION,
				runtimeBinding: "manifest.minimum_runner_version",
			},
		],
	},
	{
		id: "system",
		label: "System",
		description: "Runner-provided values that are always available at execution time.",
		variables: [
			{
				name: "system_os",
				token: "{{system_os}}",
				type: "string",
				description: "Operating system reported by the runner.",
				example: "windows",
				runtimeBinding: systemRuntimeBindings.system_os,
			},
			{
				name: "system_arch",
				token: "{{system_arch}}",
				type: "string",
				description: "CPU architecture reported by the runner.",
				example: "x64",
				runtimeBinding: systemRuntimeBindings.system_arch,
			},
			{
				name: "system_hostname",
				token: "{{system_hostname}}",
				type: "string",
				description: "Host name of the machine running the script.",
				example: "DESKTOP-01",
				runtimeBinding: systemRuntimeBindings.system_hostname,
			},
			{
				name: "system_user",
				token: "{{system_user}}",
				type: "string",
				description: "Current runner user name when the platform exposes it.",
				example: "runner",
				runtimeBinding: systemRuntimeBindings.system_user,
			},
			{
				name: "system_locale",
				token: "{{system_locale}}",
				type: "string",
				description: "Locale reported by the runner environment.",
				example: "en-US",
				runtimeBinding: systemRuntimeBindings.system_locale,
			},
			{
				name: "system_timezone",
				token: "{{system_timezone}}",
				type: "string",
				description: "Time zone reported by the runner environment.",
				example: "Europe/Helsinki",
				runtimeBinding: systemRuntimeBindings.system_timezone,
			},
			{
				name: "system_date",
				token: "{{system_date}}",
				type: "string",
				description: "Current runner-local date in ISO date format.",
				example: "2026-07-03",
				runtimeBinding: systemRuntimeBindings.system_date,
			},
			{
				name: "system_time",
				token: "{{system_time}}",
				type: "string",
				description: "Current runner-local time in 24-hour format.",
				example: "14:30:00",
				runtimeBinding: systemRuntimeBindings.system_time,
			},
		],
	},
];

export const builtInVariableNames = new Set(
	builtInVariableGroups.flatMap((group) => group.variables.map((variable) => variable.name)),
);

export function createBuiltInVariableRuntimeContext(projectSettings: ProjectSettings) {
	const variables = getBuiltInVariableRuntimeEntries(projectSettings);

	return {
		syntax: "{{variable_name}}",
		variables: variables.map((variable) => ({
			name: variable.name,
			token: variable.token,
			type: variable.type,
			scope: variable.scope,
			source: variable.source,
			read_only: variable.read_only,
			binding: variable.runtimeBinding,
			value: variable.value,
		})),
	};
}

export function getBuiltInVariableRuntimeEntries(projectSettings: ProjectSettings): BuiltInVariableRuntimeEntry[] {
	return builtInVariableGroups.flatMap((group) =>
		group.variables.map((variable) => ({
			...variable,
			read_only: true,
			scope: group.id,
			source: "built_in",
			value: resolveBuiltInVariableValue(group.id, variable.name, projectSettings),
		})),
	);
}

export function createSimulationBuiltInVariableValues(projectSettings: ProjectSettings, now = new Date()) {
	const manifestValues = Object.fromEntries(
		Object.entries(manifestValueResolvers).map(([name, resolver]) => [name, resolver(projectSettings)]),
	);
	const locale = getSimulationLocale();
	const timezone = getSimulationTimeZone();

	return {
		...manifestValues,
		system_os: getSimulationOperatingSystem(),
		system_arch: "simulated",
		system_hostname: "simulator",
		system_user: "simulator",
		system_locale: locale,
		system_timezone: timezone,
		system_date: formatSimulationDate(now),
		system_time: formatSimulationTime(now),
	} satisfies Record<string, BuiltInVariableValue>;
}

function resolveBuiltInVariableValue(
	scope: BuiltInVariableScope,
	name: string,
	projectSettings: ProjectSettings,
): BuiltInVariableValue | undefined {
	if (scope === "system") {
		return undefined;
	}

	const resolver = manifestValueResolvers[name];
	if (!resolver) {
		throw new Error(`Manifest built-in variable ${name} is missing a value resolver.`);
	}

	return resolver(projectSettings);
}

function getSimulationOperatingSystem() {
	const platform =
		typeof navigator === "undefined"
			? ""
			: String(
					(navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
						navigator.platform,
				);
	const normalizedPlatform = platform.toLowerCase();

	if (normalizedPlatform.includes("win")) {
		return "windows";
	}

	if (normalizedPlatform.includes("mac")) {
		return "unsupported";
	}

	if (normalizedPlatform.includes("linux")) {
		return "linux";
	}

	return "simulated";
}

function getSimulationLocale() {
	if (typeof navigator !== "undefined" && navigator.language) {
		return navigator.language;
	}

	return "en-US";
}

function getSimulationTimeZone() {
	try {
		return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
	} catch {
		return "UTC";
	}
}

function formatSimulationDate(date: Date) {
	return date.toISOString().slice(0, 10);
}

function formatSimulationTime(date: Date) {
	return date.toTimeString().slice(0, 8);
}
