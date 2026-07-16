import packageMetadata from "@/package.json";

export const EDITOR_VERSION = packageMetadata.version;
export const EDITOR_CREATED_WITH = `BaudBound Editor ${EDITOR_VERSION}`;

// Raise this only when newly exported packages require a newer runner contract.
export const DEFAULT_MINIMUM_RUNNER_VERSION = "2.0.0";
