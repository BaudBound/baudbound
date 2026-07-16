import {
	defaultPanelCollapsedState,
	defaultPanelSizes,
	type EditorPanelCollapsedState,
	type EditorPanelSizes,
	sanitizePanelCollapsedState,
	sanitizePanelSizes,
} from "@/data/editor/panel-layout";
import { openEditorDatabase, preferenceStoreName, requestResult, transactionComplete } from "./database";

const panelPreferenceKey = "panel-layout.v1";
const migrationMarkerKey = "migration.local-storage-panels.v1";
const legacyPanelSizesKey = "baudbound.editor.panel-sizes.v1";
const legacyPanelCollapsedKey = "baudbound.editor.panel-collapsed.v1";

export type EditorPanelPreferences = {
	collapsed: EditorPanelCollapsedState;
	sizes: EditorPanelSizes;
};

type PreferenceRecord = {
	key: string;
	value: unknown;
};

export async function initializeEditorPreferences() {
	const database = await openEditorDatabase();
	const transaction = database.transaction(preferenceStoreName, "readwrite");
	const store = transaction.objectStore(preferenceStoreName);
	const migrated = await requestResult(store.get(migrationMarkerKey));
	if (migrated) {
		await transactionComplete(transaction);
		return;
	}

	const preferences = readLegacyPanelPreferences();
	store.put({ key: panelPreferenceKey, value: preferences } satisfies PreferenceRecord);
	store.put({ key: migrationMarkerKey, value: true } satisfies PreferenceRecord);
	await transactionComplete(transaction);

	try {
		window.localStorage.removeItem(legacyPanelSizesKey);
		window.localStorage.removeItem(legacyPanelCollapsedKey);
	} catch {
		// Migration is committed. In restricted contexts the obsolete values may remain inaccessible.
	}
}

export async function getPanelPreferences(): Promise<EditorPanelPreferences> {
	await initializeEditorPreferences();
	const database = await openEditorDatabase();
	const transaction = database.transaction(preferenceStoreName, "readonly");
	const record = (await requestResult(transaction.objectStore(preferenceStoreName).get(panelPreferenceKey))) as
		| PreferenceRecord
		| undefined;
	await transactionComplete(transaction);
	return sanitizePreferences(record?.value);
}

export async function setPanelPreferences(preferences: EditorPanelPreferences) {
	const database = await openEditorDatabase();
	const transaction = database.transaction(preferenceStoreName, "readwrite");
	transaction.objectStore(preferenceStoreName).put({
		key: panelPreferenceKey,
		value: sanitizePreferences(preferences),
	} satisfies PreferenceRecord);
	await transactionComplete(transaction);
}

function readLegacyPanelPreferences(): EditorPanelPreferences {
	let sizes: unknown;
	let collapsed: unknown;
	try {
		sizes = parseLegacyValue(window.localStorage.getItem(legacyPanelSizesKey));
		collapsed = parseLegacyValue(window.localStorage.getItem(legacyPanelCollapsedKey));
	} catch {
		return { sizes: defaultPanelSizes, collapsed: defaultPanelCollapsedState };
	}
	return {
		sizes: sanitizePanelSizes(sizes),
		collapsed: sanitizePanelCollapsedState(collapsed),
	};
}

function parseLegacyValue(value: string | null) {
	return value === null ? undefined : (JSON.parse(value) as unknown);
}

function sanitizePreferences(value: unknown): EditorPanelPreferences {
	const record = typeof value === "object" && value !== null ? (value as Partial<EditorPanelPreferences>) : {};
	return {
		sizes: sanitizePanelSizes(record.sizes),
		collapsed: sanitizePanelCollapsedState(record.collapsed),
	};
}
