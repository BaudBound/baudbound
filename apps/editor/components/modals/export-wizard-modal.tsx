"use client";

import { CheckCircle2, Download, FileText, FolderClosed, ShieldAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { riskTone } from "@/data/editor/risk";
import type { CapabilitySummary, ExportSummary, PermissionSummary, ProjectSettings } from "@/lib/types";
import type { VerificationCheck, VerificationSummary } from "@/utils/verification";
import { RiskBadge } from "../shell/risk-badge";
import { VerificationProgress } from "./verification-modal";

type ExportWizardStep = "project" | "access" | "verification";

type ExportWizardModalProps = {
	capabilities: CapabilitySummary[];
	checks: VerificationCheck[];
	exportSummary: ExportSummary;
	onClose: () => void;
	onDownload: () => Promise<void>;
	onVerificationComplete: (summary: VerificationSummary) => void;
	open: boolean;
	permissions: PermissionSummary[];
	projectSettings: ProjectSettings;
	riskLevel: PermissionSummary["risk"];
};

const exportSteps: Array<{ id: ExportWizardStep; label: string }> = [
	{ id: "project", label: "Project" },
	{ id: "access", label: "Access" },
	{ id: "verification", label: "Verify" },
];

export function ExportWizardModal({
	capabilities,
	checks,
	exportSummary,
	onClose,
	onDownload,
	onVerificationComplete,
	open,
	permissions,
	projectSettings,
	riskLevel,
}: ExportWizardModalProps) {
	const [stepIndex, setStepIndex] = useState(0);
	const [verificationSummary, setVerificationSummary] = useState<VerificationSummary | null>(null);
	const [exporting, setExporting] = useState(false);
	const currentStep = exportSteps[stepIndex];
	const canGoNext = currentStep.id !== "verification" || isPassingVerification(verificationSummary);

	useEffect(() => {
		if (!open) {
			return;
		}

		setStepIndex(0);
		setVerificationSummary(null);
		setExporting(false);
	}, [open]);

	const handleVerificationComplete = useCallback(
		(summary: VerificationSummary) => {
			setVerificationSummary(summary);
			onVerificationComplete(summary);
		},
		[onVerificationComplete],
	);

	const handleDownload = async () => {
		setExporting(true);
		try {
			await onDownload();
		} finally {
			setExporting(false);
		}
	};

	const handleNext = () => {
		const nextIndex = Math.min(exportSteps.length - 1, stepIndex + 1);
		if (exportSteps[nextIndex]?.id === "verification") {
			setVerificationSummary(null);
		}

		setStepIndex(nextIndex);
	};

	const handleStepSelect = (index: number) => {
		if (index === stepIndex) {
			return;
		}

		if (exportSteps[index]?.id === "verification") {
			setVerificationSummary(null);
		}

		setStepIndex(index);
	};

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent
				className="grid h-[86vh] max-h-[86vh] grid-rows-[auto_auto_minmax(0,1fr)_auto] overflow-hidden p-0 sm:max-w-5xl"
				onInteractOutside={(event) => event.preventDefault()}
				showCloseButton={false}
			>
				<DialogHeader className="border-b border-baud-border px-6 py-5">
					<div className="flex items-start justify-between gap-4">
						<div>
							<DialogTitle className="text-lg text-baud-text">Export .bbs</DialogTitle>
							<DialogDescription>Review the package, verify the script, then download the export.</DialogDescription>
						</div>
						<Button type="button" onClick={onClose} aria-label="Cancel export" size="icon" variant="icon">
							<X size={15} />
						</Button>
					</div>
				</DialogHeader>

				<ExportStepIndicator activeIndex={stepIndex} onStepSelect={handleStepSelect} />

				<div className="min-h-0 overflow-y-auto px-6 py-5 [scrollbar-gutter:stable]">
					{currentStep.id === "project" && (
						<ProjectReviewStep exportSummary={exportSummary} projectSettings={projectSettings} />
					)}
					{currentStep.id === "access" && (
						<AccessReviewStep capabilities={capabilities} permissions={permissions} riskLevel={riskLevel} />
					)}
					{currentStep.id === "verification" && (
						<VerificationStep
							checks={checks}
							active={open && currentStep.id === "verification"}
							summary={verificationSummary}
							onComplete={handleVerificationComplete}
						/>
					)}
				</div>

				<div className="flex items-center justify-between border-t border-baud-border px-6 py-4">
					<Button type="button" onClick={onClose} variant="toolbar">
						Cancel
					</Button>
					<div className="flex items-center gap-2">
						{stepIndex > 0 && (
							<Button type="button" onClick={() => setStepIndex((index) => Math.max(0, index - 1))} variant="toolbar">
								Back
							</Button>
						)}
						{currentStep.id !== "verification" ? (
							<Button type="button" disabled={!canGoNext} onClick={handleNext} variant="toolbarActive">
								Next
							</Button>
						) : (
							<Button
								type="button"
								disabled={exporting || !isPassingVerification(verificationSummary)}
								onClick={handleDownload}
								variant="primary"
							>
								<Download size={14} />
								{exporting ? "Preparing..." : "Download .bbs"}
							</Button>
						)}
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

function ExportStepIndicator({
	activeIndex,
	onStepSelect,
}: {
	activeIndex: number;
	onStepSelect: (index: number) => void;
}) {
	return (
		<div className="border-b border-baud-border bg-baud-bg/35 px-6 py-3">
			<div className="mx-auto grid max-w-3xl grid-cols-3">
				{exportSteps.map((step, index) => {
					const complete = index < activeIndex;
					const active = index === activeIndex;

					return (
						<button
							key={step.id}
							type="button"
							onClick={() => onStepSelect(index)}
							className="relative flex min-w-0 flex-col items-center text-center outline-none"
						>
							{index < exportSteps.length - 1 && (
								<div
									className={`absolute top-4 left-[calc(50%+1rem)] z-0 h-px w-[calc(100%-2rem)] ${
										index < activeIndex ? "bg-baud-red/70" : "bg-baud-border"
									}`}
								/>
							)}
							<div
								className={`relative z-10 grid size-8 shrink-0 place-items-center rounded-full border font-mono text-xs font-bold ${
									complete
										? "border-baud-red bg-baud-red text-white"
										: active
											? "border-baud-red bg-baud-red/10 text-baud-text shadow-[0_0_0_3px_rgb(230_45_62_/_0.12)]"
											: "border-baud-border bg-baud-panel text-baud-muted"
								}`}
							>
								{complete ? <CheckCircle2 size={15} /> : index + 1}
							</div>
							<div className="mt-2 min-w-0">
								<div
									className={`truncate text-sm font-semibold ${
										active || complete ? "text-baud-text" : "text-baud-muted"
									}`}
								>
									{step.label}
								</div>
								<div className="mt-0.5 hidden text-xs text-baud-muted md:block">{getStepDescription(step.id)}</div>
							</div>
						</button>
					);
				})}
			</div>
		</div>
	);
}

function getStepDescription(step: ExportWizardStep) {
	switch (step) {
		case "project":
			return "Package metadata";
		case "access":
			return "Risk and access";
		case "verification":
			return "Verify and download";
	}
}

function ProjectReviewStep({
	exportSummary,
	projectSettings,
}: {
	exportSummary: ExportSummary;
	projectSettings: ProjectSettings;
}) {
	return (
		<div className="grid gap-5 lg:grid-cols-[1fr_1fr]">
			<section className="space-y-3">
				<SectionTitle title="Project Information" />
				<div className="rounded border border-baud-border bg-baud-elevated p-4">
					<SummaryRow label="Name" value={projectSettings.name} />
					<SummaryRow label="Target" value={projectSettings.targetRuntime} />
					<SummaryRow label="Author" value={projectSettings.author || "Not set"} />
					<SummaryRow label="Website" value={projectSettings.website || "Not set"} />
					<SummaryRow label="Repository" value={projectSettings.repository || "Not set"} />
					<SummaryRow label="Min runner" value={projectSettings.minimumRunnerVersion} />
				</div>
				{projectSettings.description && (
					<div className="rounded border border-baud-border bg-baud-elevated p-4 text-sm leading-6 text-baud-muted">
						{projectSettings.description}
					</div>
				)}
				{projectSettings.tags.length > 0 && (
					<div className="flex flex-wrap gap-2">
						{projectSettings.tags.map((tag) => (
							<Badge key={tag} variant="outline">
								{tag}
							</Badge>
						))}
					</div>
				)}
			</section>

			<section className="space-y-3">
				<SectionTitle title="Export Preview" />
				<div className="rounded border border-baud-border bg-baud-elevated p-4">
					<SummaryRow label="Filename" value={exportSummary.filename} />
					<SummaryRow label="Package" value={`v${exportSummary.formatVersion}`} />
					<SummaryRow label="Runtime" value={`v${exportSummary.languageVersion}`} />
					<SummaryRow label="Target" value={exportSummary.targetRuntime} />
				</div>
				<div className="rounded border border-baud-border bg-baud-elevated p-4">
					<h3 className="mb-3 text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Package Contents</h3>
					<PackageContentsTree contents={exportSummary.contents} />
				</div>
			</section>
		</div>
	);
}

type PackageTreeEntry = {
	children: PackageTreeEntry[];
	isFile: boolean;
	name: string;
	path: string;
};

type MutablePackageTreeEntry = Omit<PackageTreeEntry, "children"> & {
	children: Map<string, MutablePackageTreeEntry>;
};

function PackageContentsTree({ contents }: { contents: string[] }) {
	const tree = buildPackageTree(contents);

	return (
		<div className="space-y-1 font-mono text-sm text-baud-muted">
			{tree.map((entry) => (
				<PackageContentsTreeNode key={entry.path} entry={entry} level={0} />
			))}
		</div>
	);
}

function PackageContentsTreeNode({ entry, level }: { entry: PackageTreeEntry; level: number }) {
	const Icon = entry.isFile ? FileText : FolderClosed;

	return (
		<div>
			<div className="flex min-w-0 items-center gap-2 rounded px-1 py-1" style={{ paddingLeft: level * 16 + 4 }}>
				<Icon size={13} className={entry.isFile ? "text-baud-muted" : "text-baud-blue"} />
				<span className="truncate">{entry.name}</span>
			</div>
			{entry.children.map((child) => (
				<PackageContentsTreeNode key={child.path} entry={child} level={level + 1} />
			))}
		</div>
	);
}

function buildPackageTree(contents: string[]): PackageTreeEntry[] {
	const root = new Map<string, MutablePackageTreeEntry>();

	for (const path of [...contents].sort((a, b) => a.localeCompare(b))) {
		const parts = path.split("/").filter(Boolean);
		let currentChildren = root;

		parts.forEach((part, index) => {
			const entryPath = parts.slice(0, index + 1).join("/");
			const isFile = index === parts.length - 1;
			const existing = currentChildren.get(part);

			if (existing) {
				existing.isFile = existing.isFile || isFile;
				currentChildren = existing.children;
				return;
			}

			const entry: MutablePackageTreeEntry = {
				children: new Map(),
				isFile,
				name: part,
				path: entryPath,
			};
			currentChildren.set(part, entry);
			currentChildren = entry.children;
		});
	}

	return toPackageTreeEntries(root);
}

function toPackageTreeEntries(entries: Map<string, MutablePackageTreeEntry>): PackageTreeEntry[] {
	return [...entries.values()]
		.sort((a, b) => Number(a.isFile) - Number(b.isFile) || a.name.localeCompare(b.name))
		.map((entry) => ({
			children: toPackageTreeEntries(entry.children),
			isFile: entry.isFile,
			name: entry.name,
			path: entry.path,
		}));
}

function AccessReviewStep({
	capabilities,
	permissions,
	riskLevel,
}: {
	capabilities: CapabilitySummary[];
	permissions: PermissionSummary[];
	riskLevel: PermissionSummary["risk"];
}) {
	return (
		<div className="space-y-5">
			<section className="space-y-3">
				<SectionTitle title="Calculated Risk" />
				<div
					className="flex items-center gap-2 rounded border px-4 py-3 text-sm font-semibold"
					style={riskTone[riskLevel]}
				>
					<ShieldAlert size={16} />
					{riskLevel === "low" ? "Low risk" : `${riskLevel[0].toUpperCase()}${riskLevel.slice(1)} risk`}
					{riskLevel !== "low" && " - review required on import"}
				</div>
			</section>

			<div className="grid gap-5 lg:grid-cols-2">
				<section className="space-y-3">
					<SectionTitle title="Required Permissions" />
					<div className="rounded border border-baud-border bg-baud-elevated">
						{permissions.length === 0 ? (
							<div className="px-4 py-3 text-sm text-baud-muted">No permissions required.</div>
						) : (
							<div className="divide-y divide-baud-border">
								{permissions.map((permission) => (
									<div key={permission.name} className="flex items-center justify-between gap-3 px-4 py-3">
										<span className="font-mono text-sm text-baud-text">{permission.name}</span>
										<RiskBadge risk={permission.risk} />
									</div>
								))}
							</div>
						)}
					</div>
				</section>

				<section className="space-y-3">
					<SectionTitle title="Required Capabilities" />
					<div className="rounded border border-baud-border bg-baud-elevated">
						{capabilities.length === 0 ? (
							<div className="px-4 py-3 text-sm text-baud-muted">No capabilities required.</div>
						) : (
							<div className="divide-y divide-baud-border">
								{capabilities.map((capability) => (
									<div
										key={capability.name}
										className="flex items-center gap-2 px-4 py-3 font-mono text-sm text-baud-muted"
									>
										<CheckCircle2 size={13} className="text-baud-green" />
										{capability.name}
									</div>
								))}
							</div>
						)}
					</div>
				</section>
			</div>
		</div>
	);
}

function VerificationStep({
	active,
	checks,
	onComplete,
	summary,
}: {
	active: boolean;
	checks: VerificationCheck[];
	onComplete: (summary: VerificationSummary) => void;
	summary: VerificationSummary | null;
}) {
	return (
		<div className="space-y-4">
			<SectionTitle title="Verification" />
			<VerificationProgress active={active} checks={checks} onComplete={onComplete} />
			{summary?.status === "failed" && (
				<div className="rounded border border-baud-danger/35 bg-baud-danger/10 px-4 py-3 text-sm text-baud-danger">
					Resolve failed checks before continuing to download.
				</div>
			)}
			{isPassingVerification(summary) && (
				<div className="rounded border border-baud-green/35 bg-baud-green/10 px-4 py-3 text-sm text-baud-green">
					Verification passed. The download button is now available.
				</div>
			)}
		</div>
	);
}

function SectionTitle({ title }: { title: string }) {
	return <h2 className="text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">{title}</h2>;
}

function SummaryRow({ label, value }: { label: string; value: string }) {
	return (
		<div className="flex items-center justify-between gap-4 border-b border-baud-border py-2 text-sm last:border-b-0">
			<span className="text-baud-muted">{label}</span>
			<span className="min-w-0 truncate text-right font-mono font-semibold text-baud-text">{value}</span>
		</div>
	);
}

function isPassingVerification(summary: VerificationSummary | null) {
	return summary?.status === "verified" || summary?.status === "warning";
}
