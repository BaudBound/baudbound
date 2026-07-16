"use client";

import { ArrowLeft, RefreshCw } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { EditorPage } from "@/app/editor-page";
import { ProjectAccessGate } from "@/components/projects/project-access-gate";
import { WorkspaceNotice } from "@/components/projects/workspace-notice";
import { Button } from "@/components/ui/button";
import type { EditorProject } from "@/data/projects/model";
import { getProject, requestPersistentEditorStorage } from "@/data/storage/project-repository";
import { useProjectSession } from "@/hooks/use-project-session";

export function ProjectWorkspace({ projectId }: { projectId: string }) {
	const router = useRouter();
	const [loadAttempt, setLoadAttempt] = useState(0);
	const [project, setProject] = useState<EditorProject | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [dirty, setDirty] = useState(false);
	const [persistenceWarning, setPersistenceWarning] = useState(false);
	const [noticeDismissed, setNoticeDismissed] = useState(false);
	const session = useProjectSession(projectId, dirty);

	useEffect(() => {
		let active = true;
		setProject(null);
		setError(null);
		void getProject(projectId)
			.then((loaded) => active && setProject(loaded))
			.catch(
				(cause) => active && setError(cause instanceof Error ? cause.message : "The project could not be opened."),
			);
		return () => {
			active = false;
		};
	}, [loadAttempt, projectId]);

	useEffect(() => {
		if (!project || session.status !== "owner") return;
		let active = true;
		void requestPersistentEditorStorage().then((persistent) => {
			if (active) setPersistenceWarning(!persistent);
		});
		return () => {
			active = false;
		};
	}, [project, session.status]);

	if (session.status !== "owner") {
		return (
			<ProjectAccessGate
				checking={session.status === "checking"}
				error={session.takeoverError}
				onReturn={() => router.push("/")}
				onTakeControl={session.requestTakeover}
			/>
		);
	}

	if (project) {
		return (
			<>
				<EditorPage initialProject={project} onDirtyChange={setDirty} />
				{!noticeDismissed && (!session.coordinationAvailable || persistenceWarning) && (
					<WorkspaceNotice onDismiss={() => setNoticeDismissed(true)}>
						{!session.coordinationAvailable
							? "This browser cannot coordinate editor tabs. Keep this project open in only one tab to prevent revision conflicts."
							: "Protected browser storage was not granted. This project remains local to this browser profile and device, and clearing site data removes it. Export .bbs backups regularly."}
					</WorkspaceNotice>
				)}
			</>
		);
	}

	return (
		<div className="grid min-h-dvh place-items-center bg-baud-bg px-4 text-baud-text">
			<div className="max-w-md text-center">
				{error ? (
					<>
						<h1 className="text-xl font-semibold text-white">Project unavailable</h1>
						<p className="mt-2 text-sm leading-6 text-baud-muted">{error}</p>
						<div className="mt-5 flex justify-center gap-2">
							<Button type="button" variant="toolbar" onClick={() => router.push("/")}>
								<ArrowLeft /> Projects
							</Button>
							<Button type="button" variant="primary" onClick={() => setLoadAttempt((attempt) => attempt + 1)}>
								<RefreshCw /> Retry
							</Button>
						</div>
					</>
				) : (
					<p className="text-sm text-baud-muted">Opening project...</p>
				)}
			</div>
		</div>
	);
}
