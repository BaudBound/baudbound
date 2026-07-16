import { Copy, Download, FolderOpen, LoaderCircle, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ProjectSummary } from "@/data/projects/model";

export function ProjectList({
	projects,
	onDelete,
	onDuplicate,
	onExport,
	onOpen,
	exportingProjectId,
}: {
	projects: ProjectSummary[];
	onDelete: (project: ProjectSummary) => void;
	onDuplicate: (project: ProjectSummary) => void;
	onExport: (project: ProjectSummary) => void;
	onOpen: (project: ProjectSummary) => void;
	exportingProjectId: string | null;
}) {
	if (projects.length === 0) {
		return (
			<div className="border-y border-baud-border py-10 text-center">
				<div className="text-sm font-medium text-baud-text">No local projects</div>
				<p className="mt-1 text-sm text-baud-muted">Create a project or open an existing BaudBound package.</p>
			</div>
		);
	}

	return (
		<div className="divide-y divide-baud-border border-y border-baud-border">
			{projects.map((project) => {
				const exporting = exportingProjectId === project.id;
				return (
					<div className="grid min-w-0 gap-3 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center" key={project.id}>
						<button
							type="button"
							className="min-w-0 rounded px-1 py-1 text-left outline-none transition hover:bg-baud-soft focus-visible:ring-2 focus-visible:ring-baud-red/50"
							onClick={() => onOpen(project)}
						>
							<div className="truncate text-sm font-semibold text-baud-text">{project.name}</div>
							<div className="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-xs text-baud-muted">
								<span>{project.targetRuntime}</span>
								<span>{formatProjectCounts(project)}</span>
								<span>Updated {formatUpdatedAt(project.updatedAt)}</span>
							</div>
						</button>
						<div className="flex items-center gap-1 justify-self-start sm:justify-self-end">
							<Button type="button" variant="toolbar" size="sm" onClick={() => onOpen(project)}>
								<FolderOpen />
								Open
							</Button>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								disabled={exportingProjectId !== null}
								aria-label={`Export ${project.name}`}
								title="Export project"
								onClick={() => onExport(project)}
							>
								{exporting ? <LoaderCircle className="animate-spin" /> : <Download />}
							</Button>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								aria-label={`Duplicate ${project.name}`}
								title="Duplicate project"
								onClick={() => onDuplicate(project)}
							>
								<Copy />
							</Button>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								className="hover:bg-rose-400/10 hover:text-rose-200"
								aria-label={`Delete ${project.name}`}
								title="Delete project"
								onClick={() => onDelete(project)}
							>
								<Trash2 />
							</Button>
						</div>
					</div>
				);
			})}
		</div>
	);
}

function formatProjectCounts(project: ProjectSummary) {
	const nodeLabel = project.nodeCount === 1 ? "node" : "nodes";
	const edgeLabel = project.edgeCount === 1 ? "connection" : "connections";
	return `${project.nodeCount} ${nodeLabel}, ${project.edgeCount} ${edgeLabel}`;
}

function formatUpdatedAt(value: string) {
	const timestamp = Date.parse(value);
	if (!Number.isFinite(timestamp)) return "unknown";
	return new Intl.DateTimeFormat(undefined, {
		dateStyle: "medium",
		timeStyle: "short",
	}).format(new Date(timestamp));
}
