import { ProjectWorkspace } from "@/components/projects/project-workspace";

export default async function ProjectPage({ params }: { params: Promise<{ projectId: string }> }) {
	const { projectId } = await params;
	return <ProjectWorkspace projectId={projectId} />;
}
