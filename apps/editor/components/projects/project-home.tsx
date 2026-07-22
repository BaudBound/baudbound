"use client";

import { CircleHelp, FolderOpen, Plus } from "lucide-react";
import Image from "next/image";
import { type ChangeEvent, useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { HelpModal } from "@/components/modals/help-modal";
import { ProjectSettingsModal } from "@/components/modals/project-settings-modal";
import { VerificationErrorModal } from "@/components/modals/verification-error-modal";
import { DeleteProjectDialog, ImportConflictDialog } from "@/components/projects/project-dialogs";
import { ProjectList } from "@/components/projects/project-list";
import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import { createEmptyEditorProject, createProjectIdentity, defaultProjectSettings } from "@/data/projects/defaults";
import type { EditorProject, ProjectSummary } from "@/data/projects/model";
import { editorProjectSchemaVersion } from "@/data/projects/model";
import {
	createProject,
	deleteProject,
	getProject,
	listProjects,
	projectExists,
	replaceProject,
	requestPersistentEditorStorage,
} from "@/data/storage/project-repository";
import type { ProjectSettings } from "@/lib/types";
import { exportBbsPackage, importBbsPackage, verifyBbsPackage } from "@/utils/bbs-package";
import type { VerificationCheck } from "@/utils/verification";

type PendingImport = { fileName: string; project: EditorProject };

export function ProjectHome() {
	const packageInputRef = useRef<HTMLInputElement>(null);
	const [projects, setProjects] = useState<ProjectSummary[]>([]);
	const [loading, setLoading] = useState(true);
	const [createOpen, setCreateOpen] = useState(false);
	const [helpOpen, setHelpOpen] = useState(false);
	const [exportingProjectId, setExportingProjectId] = useState<string | null>(null);
	const [deleteTarget, setDeleteTarget] = useState<ProjectSummary | null>(null);
	const [pendingImport, setPendingImport] = useState<PendingImport | null>(null);
	const [importError, setImportError] = useState<{ checks: VerificationCheck[]; description: string } | null>(null);

	const refreshProjects = useCallback(async () => {
		setProjects(await listProjects());
	}, []);

	useEffect(() => {
		void refreshProjects()
			.catch((error) => toast.error(toErrorMessage(error)))
			.finally(() => setLoading(false));
	}, [refreshProjects]);

	useEffect(() => {
		const disableNativeContextMenu = (event: MouseEvent) => event.preventDefault();
		document.addEventListener("contextmenu", disableNativeContextMenu);
		return () => document.removeEventListener("contextmenu", disableNativeContextMenu);
	}, []);

	const openProject = (projectId: string) => window.location.assign(`/projects/${projectId}`);

	const handleCreate = async (settings: ProjectSettings) => {
		try {
			const project = await createProject(createEmptyEditorProject(settings));
			void requestPersistentEditorStorage();
			openProject(project.identity.id);
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	};

	const handleDuplicate = async (summary: ProjectSummary) => {
		try {
			const source = await getProject(summary.id);
			const identity = createProjectIdentity();
			const duplicate = await createProject({
				...source,
				identity,
				revision: 1,
				settings: { ...source.settings, name: `${source.settings.name} copy` },
				updatedAt: identity.createdAt,
			});
			await refreshProjects();
			toast.success(`Created ${duplicate.settings.name}.`);
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	};

	const handleExport = async (summary: ProjectSummary) => {
		if (exportingProjectId !== null) return;
		setExportingProjectId(summary.id);
		try {
			const project = await getProject(summary.id);
			await exportBbsPackage({
				assets: project.assets,
				comments: project.comments,
				defaultVariables: project.defaultVariables,
				edges: project.edges,
				edgeStyle: project.edgeStyle,
				identity: project.identity,
				nodes: project.nodes,
				projectSettings: project.settings,
				secretDeclarations: project.secretDeclarations,
			});
			toast.success(`Exported ${project.settings.name}.`);
		} catch (error) {
			toast.error(toErrorMessage(error));
		} finally {
			setExportingProjectId(null);
		}
	};

	const handleDelete = async () => {
		if (!deleteTarget) return;
		try {
			await deleteProject(deleteTarget.id);
			setDeleteTarget(null);
			await refreshProjects();
			toast.success("Project deleted.");
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	};

	const handlePackageSelected = async (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = "";
		if (!file) return;

		try {
			const verification = await verifyBbsPackage(file);
			if (verification.summary.status !== "verified") {
				setImportError({
					checks: verification.checks,
					description: "The imported package did not pass verification cleanly and was not loaded.",
				});
				return;
			}
			const imported = await importBbsPackage(file);
			const project = importedToProject(imported);
			if (await projectExists(project.identity.id)) {
				setPendingImport({ fileName: file.name, project });
				return;
			}
			const created = await createProject(project);
			void requestPersistentEditorStorage();
			openProject(created.identity.id);
		} catch (error) {
			const message = toErrorMessage(error);
			setImportError({
				checks: [
					{
						id: "package-read",
						title: "Package Read",
						description: "Checking that the package can be opened.",
						outcome: "failed",
						message,
					},
				],
				description: message,
			});
		}
	};

	const importAsCopy = async () => {
		if (!pendingImport) return;
		try {
			const identity = createProjectIdentity();
			const created = await createProject({
				...pendingImport.project,
				identity,
				revision: 1,
				settings: { ...pendingImport.project.settings, name: `${pendingImport.project.settings.name} copy` },
				updatedAt: identity.createdAt,
			});
			setPendingImport(null);
			openProject(created.identity.id);
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	};

	const replaceImported = async () => {
		if (!pendingImport) return;
		try {
			const replaced = await replaceProject(pendingImport.project);
			setPendingImport(null);
			openProject(replaced.identity.id);
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	};

	return (
		<div className="min-h-dvh select-none bg-baud-bg text-baud-text">
			<input
				ref={packageInputRef}
				className="hidden"
				type="file"
				accept=".bbs,application/zip"
				onChange={handlePackageSelected}
			/>
			<header className="border-b border-baud-border bg-baud-panel">
				<div className="mx-auto flex min-h-16 max-w-5xl items-center gap-3 px-4 sm:px-6">
					<Image src="/logo-notext.svg" alt="" width={34} height={34} priority aria-hidden="true" />
					<div>
						<div className="font-semibold text-white">BaudBound Editor</div>
						<div className="text-xs text-baud-muted">Local project workspace</div>
					</div>
					<Button
						className="ml-auto"
						type="button"
						variant="ghost"
						size="icon"
						aria-label="Open help"
						title="Help"
						onClick={() => setHelpOpen(true)}
					>
						<CircleHelp />
					</Button>
				</div>
			</header>

			<main className="mx-auto max-w-5xl px-4 py-8 sm:px-6 sm:py-12">
				<div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
					<div>
						<h1 className="text-2xl font-semibold text-white">Projects</h1>
						<p className="mt-1 max-w-xl text-sm text-baud-muted">
							Create, open, and manage projects stored in this browser profile.
						</p>
					</div>
					<div className="flex flex-wrap gap-2">
						<Button type="button" variant="toolbar" onClick={() => packageInputRef.current?.click()}>
							<FolderOpen /> Open package
						</Button>
						<Button type="button" variant="primary" onClick={() => setCreateOpen(true)}>
							<Plus /> New project
						</Button>
					</div>
				</div>

				<section className="mt-8" aria-label="Local projects">
					{loading ? (
						<p className="py-10 text-center text-sm text-baud-muted">Loading projects...</p>
					) : (
						<ProjectList
							projects={projects}
							exportingProjectId={exportingProjectId}
							onOpen={(project) => openProject(project.id)}
							onDuplicate={handleDuplicate}
							onExport={(project) => void handleExport(project)}
							onDelete={setDeleteTarget}
						/>
					)}
				</section>
			</main>

			<ProjectSettingsModal
				open={createOpen}
				settings={defaultProjectSettings}
				title="Create project"
				description="Set the project identity and target runtime before opening the workspace."
				saveLabel="Create project"
				onClose={() => setCreateOpen(false)}
				onSave={handleCreate}
			/>
			<DeleteProjectDialog
				name={deleteTarget?.name ?? ""}
				open={deleteTarget !== null}
				onCancel={() => setDeleteTarget(null)}
				onConfirm={handleDelete}
			/>
			<ImportConflictDialog
				name={pendingImport?.project.settings.name ?? pendingImport?.fileName ?? "package"}
				open={pendingImport !== null}
				onCancel={() => setPendingImport(null)}
				onOpenExisting={() => pendingImport && openProject(pendingImport.project.identity.id)}
				onImportCopy={importAsCopy}
				onReplace={replaceImported}
			/>
			<VerificationErrorModal
				checks={importError?.checks ?? []}
				description={importError?.description ?? ""}
				open={importError !== null}
				title="Import Rejected"
				onClose={() => setImportError(null)}
			/>
			<HelpModal open={helpOpen} onClose={() => setHelpOpen(false)} />
			<Toaster position="top-center" closeButton richColors />
		</div>
	);
}

function importedToProject(imported: Awaited<ReturnType<typeof importBbsPackage>>): EditorProject {
	return {
		assets: imported.assets,
		comments: imported.comments,
		defaultVariables: imported.defaultVariables,
		edgeStyle: imported.edgeStyle,
		edges: imported.edges,
		identity: imported.identity,
		nodes: imported.nodes,
		revision: 1,
		schemaVersion: editorProjectSchemaVersion,
		secretDeclarations: imported.secretDeclarations,
		settings: imported.projectSettings,
		updatedAt: new Date().toISOString(),
	};
}

function toErrorMessage(error: unknown) {
	return error instanceof Error ? error.message : "The project operation failed.";
}
