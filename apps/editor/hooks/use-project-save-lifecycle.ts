"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import type { EditorProject } from "@/data/projects/model";
import { saveProject } from "@/data/storage/project-repository";
import { describeSaveFailure, type SaveFailure } from "@/data/storage/save-failure";

export type ProjectSaveStatus = "error" | "saved" | "saving" | "unsaved";

type SaveOperation = "error" | "idle" | "saving";

export function useProjectSaveLifecycle({
	currentProject,
	currentSignature,
	expectedRevision,
	initialSavedSignature,
	onCommitted,
	onDirtyChange,
	onReturn,
}: {
	currentProject: EditorProject;
	currentSignature: string;
	expectedRevision: number;
	initialSavedSignature: string;
	onCommitted: (project: EditorProject) => void;
	onDirtyChange?: (dirty: boolean) => void;
	onReturn: () => void;
}) {
	const savingRef = useRef(false);
	const [savedSignature, setSavedSignature] = useState(initialSavedSignature);
	const [operation, setOperation] = useState<SaveOperation>("idle");
	const [failure, setFailure] = useState<SaveFailure | null>(null);
	const [leaveDialogOpen, setLeaveDialogOpen] = useState(false);
	const hasUnsavedChanges = currentSignature !== savedSignature;

	useEffect(() => onDirtyChange?.(hasUnsavedChanges), [hasUnsavedChanges, onDirtyChange]);

	useEffect(() => {
		if (!hasUnsavedChanges) return;

		const handleBeforeUnload = (event: BeforeUnloadEvent) => {
			event.preventDefault();
			event.returnValue = "";
		};
		window.addEventListener("beforeunload", handleBeforeUnload);
		return () => window.removeEventListener("beforeunload", handleBeforeUnload);
	}, [hasUnsavedChanges]);

	const save = useCallback(async () => {
		if (savingRef.current) return false;
		if (!hasUnsavedChanges) return true;

		const snapshot = currentProject;
		const snapshotSignature = currentSignature;
		savingRef.current = true;
		setOperation("saving");
		setFailure(null);

		try {
			const saved = await saveProject(snapshot, expectedRevision);
			onCommitted(saved);
			setSavedSignature(snapshotSignature);
			setOperation("idle");
			toast.success("Project saved.");
			return true;
		} catch (error) {
			setOperation("error");
			setFailure(describeSaveFailure(error));
			toast.error("Project was not saved.");
			return false;
		} finally {
			savingRef.current = false;
		}
	}, [currentProject, currentSignature, expectedRevision, hasUnsavedChanges, onCommitted]);

	const requestReturn = useCallback(() => {
		if (hasUnsavedChanges) {
			setLeaveDialogOpen(true);
			return;
		}
		onReturn();
	}, [hasUnsavedChanges, onReturn]);

	const saveAndReturn = useCallback(async () => {
		if (await save()) {
			setLeaveDialogOpen(false);
			onReturn();
		}
	}, [onReturn, save]);

	const discardAndReturn = useCallback(() => {
		setLeaveDialogOpen(false);
		onReturn();
	}, [onReturn]);

	const status: ProjectSaveStatus =
		operation === "saving" ? "saving" : operation === "error" ? "error" : hasUnsavedChanges ? "unsaved" : "saved";

	return {
		closeFailure: () => setFailure(null),
		closeLeaveDialog: () => setLeaveDialogOpen(false),
		discardAndReturn,
		failure,
		hasUnsavedChanges,
		leaveDialogOpen,
		requestReturn,
		save,
		saveAndReturn,
		saving: operation === "saving",
		status,
	};
}
