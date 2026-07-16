import { ProjectRevisionConflictError } from "@/data/projects/model";
import { EditorStorageError } from "@/data/storage/database";

export type SaveFailure = {
	description: string;
	retryable: boolean;
	title: string;
};

export function describeSaveFailure(error: unknown): SaveFailure {
	if (error instanceof ProjectRevisionConflictError) {
		return {
			description:
				"A newer project revision is already stored in this browser. Your current edits remain open, but they were not allowed to overwrite the newer revision. Export this project before reloading if you need to preserve these edits.",
			retryable: false,
			title: "Project changed in another session",
		};
	}

	if (error instanceof EditorStorageError) {
		switch (error.kind) {
			case "quota":
				return {
					description:
						"The browser does not have enough storage space to save this revision. The previous saved revision is intact and your current edits remain open. Free browser storage or remove unused assets, then retry.",
					retryable: true,
					title: "Browser storage is full",
				};
			case "blocked":
				return {
					description:
						"The browser blocked access to project storage. Close other BaudBound Editor tabs that may be upgrading storage, then retry.",
					retryable: true,
					title: "Project storage is blocked",
				};
			case "unavailable":
			case "corrupt":
			case "transaction":
				return {
					description:
						"The browser could not commit this revision. The previous saved revision is intact and your current edits remain open. Retry the save or export a package before leaving this page.",
					retryable: true,
					title: "Project was not saved",
				};
		}
	}

	return {
		description:
			"The project could not be saved. The previous saved revision is intact and your current edits remain open. Retry the save or export a package before leaving this page.",
		retryable: true,
		title: "Project was not saved",
	};
}
