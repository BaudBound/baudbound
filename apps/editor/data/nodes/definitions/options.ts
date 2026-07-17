import {
	variableOperationDefinitions,
	variableOperations,
	variableScopes,
	variableTypes,
} from "../../project/variables";

export type SelectOption = {
	label: string;
	value: string;
};

export const timeUnitOptions: SelectOption[] = [
	{ label: "Milliseconds", value: "milliseconds" },
	{ label: "Seconds", value: "seconds" },
	{ label: "Minutes", value: "minutes" },
	{ label: "Hours", value: "hours" },
	{ label: "Days", value: "days" },
];

export const httpMethodOptions: SelectOption[] = [
	{ label: "GET", value: "GET" },
	{ label: "POST", value: "POST" },
	{ label: "PUT", value: "PUT" },
	{ label: "PATCH", value: "PATCH" },
	{ label: "DELETE", value: "DELETE" },
	{ label: "HEAD", value: "HEAD" },
	{ label: "OPTIONS", value: "OPTIONS" },
];

export const fileWatchEventOptions: SelectOption[] = [
	{ label: "Created", value: "created" },
	{ label: "Modified", value: "modified" },
	{ label: "Deleted", value: "deleted" },
	{ label: "Renamed", value: "renamed" },
];

export const processMatchModeOptions: SelectOption[] = [
	{ label: "Process name", value: "process_name" },
	{ label: "Executable path", value: "executable_path" },
	{ label: "Window title", value: "window_title" },
];

export const killProcessMatchModeOptions: SelectOption[] = [
	...processMatchModeOptions,
	{ label: "Process ID", value: "pid" },
];

export const comparisonOperatorOptions: SelectOption[] = [
	{ label: "equals", value: "==" },
	{ label: "does not equal", value: "!=" },
	{ label: "greater than", value: ">" },
	{ label: "greater than or equal", value: ">=" },
	{ label: "less than", value: "<" },
	{ label: "less than or equal", value: "<=" },
	{ label: "contains", value: "contains" },
	{ label: "starts with", value: "starts_with" },
	{ label: "ends with", value: "ends_with" },
	{ label: "regex match", value: "regex_match" },
	{ label: "is empty", value: "is_empty" },
	{ label: "is null", value: "is_null" },
];

export const combinatorOptions: SelectOption[] = [
	{ label: "AND", value: "and" },
	{ label: "OR", value: "or" },
];

export const variableTypeOptions: SelectOption[] = variableTypes.map((type) => ({
	label: type,
	value: type,
}));

export const variableScopeOptions: SelectOption[] = variableScopes.map((scope) => ({
	label: scope,
	value: scope,
}));

export const variableOperationOptions: SelectOption[] = variableOperations.map((operation) => ({
	label: variableOperationDefinitions[operation].label,
	value: operation,
}));

export const textTransformOperationOptions: SelectOption[] = [
	{ label: "Template", value: "template" },
	{ label: "Trim", value: "trim" },
	{ label: "Uppercase", value: "uppercase" },
	{ label: "Lowercase", value: "lowercase" },
	{ label: "Sentence case", value: "sentence_case" },
	{ label: "Capitalize words", value: "capitalize_words" },
	{ label: "Replace text", value: "replace" },
	{ label: "Regex replace", value: "regex_replace" },
	{ label: "Split", value: "split" },
	{ label: "Join", value: "join" },
	{ label: "Substring", value: "substring" },
	{ label: "Pad start", value: "pad_start" },
	{ label: "Pad end", value: "pad_end" },
	{ label: "URL encode", value: "url_encode" },
	{ label: "URL decode", value: "url_decode" },
	{ label: "Base64 encode", value: "base64_encode" },
	{ label: "Base64 decode", value: "base64_decode" },
	{ label: "JSON escape", value: "json_escape" },
	{ label: "JSON unescape", value: "json_unescape" },
];

export const logLevelOptions: SelectOption[] = [
	{ label: "Info", value: "info" },
	{ label: "Warning", value: "warn" },
	{ label: "Error", value: "error" },
	{ label: "Debug", value: "debug" },
];

export const mouseButtonOptions: SelectOption[] = [
	{ label: "Left", value: "left" },
	{ label: "Right", value: "right" },
	{ label: "Middle", value: "middle" },
	{ label: "Back", value: "back" },
	{ label: "Forward", value: "forward" },
];

export const mouseClickTypeOptions: SelectOption[] = [
	{ label: "Single click", value: "single" },
	{ label: "Double click", value: "double" },
];

export const fileWriteModeOptions: SelectOption[] = [
	{ label: "Overwrite file", value: "overwrite" },
	{ label: "Append to file", value: "append" },
];

export const fileOverwriteOptions: SelectOption[] = [
	{ label: "Do not overwrite", value: "false" },
	{ label: "Overwrite existing file", value: "true" },
];

export const playSoundSourceOptions: SelectOption[] = [
	{ label: "Asset library", value: "asset" },
	{ label: "File path", value: "file_path" },
];

export const messageBoxTypeOptions: SelectOption[] = [
	{ label: "Info", value: "info" },
	{ label: "Warning", value: "warning" },
	{ label: "Error", value: "error" },
];

export const messageBoxButtonOptions: SelectOption[] = [
	{ label: "OK", value: "ok" },
	{ label: "OK / Cancel", value: "ok_cancel" },
	{ label: "Yes / No", value: "yes_no" },
	{ label: "Yes / No / Cancel", value: "yes_no_cancel" },
];
