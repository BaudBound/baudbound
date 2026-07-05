import { CircleHelp, Download, PackageOpen, Play, ShieldCheck, SlidersHorizontal, Square, Upload } from "lucide-react";
import type { ChangeEvent, RefObject } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { TargetRuntime } from "@/lib/types";
import type { VerificationStatus } from "@/utils/verification";

type TopBarProps = {
	importInputRef: RefObject<HTMLInputElement | null>;
	leftWidth: number;
	rightWidth: number;
	isSimulationRunning: boolean;
	targetRuntime: TargetRuntime;
	verificationStatus: VerificationStatus;
	onAssetEditorClick: () => void;
	onImportClick: () => void;
	onImportFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
	onExportClick: () => void;
	onHelpClick: () => void;
	onProjectSettingsClick: () => void;
	onSimulateClick: () => void;
	onStopSimulationClick: () => void;
	onVerifyClick: () => void;
};

export function TopBar({
	importInputRef,
	isSimulationRunning,
	leftWidth,
	rightWidth,
	targetRuntime,
	verificationStatus,
	onAssetEditorClick,
	onImportClick,
	onImportFileChange,
	onExportClick,
	onHelpClick,
	onProjectSettingsClick,
	onSimulateClick,
	onStopSimulationClick,
	onVerifyClick,
}: TopBarProps) {
	return (
		<header
			className="grid h-12 shrink-0 border-b border-baud-border bg-baud-panel"
			style={{
				gridTemplateColumns: `${leftWidth}px 4px minmax(0, 1fr) 4px ${rightWidth}px`,
			}}
		>
			<div className="flex h-full min-w-0 items-center gap-2 px-3">
				<div className="grid size-7 place-items-center rounded bg-baud-red text-xs font-bold text-white">BB</div>
				<div className="min-w-0 text-sm font-semibold">
					<span className="text-white">BaudBound</span> <span className="text-baud-muted">Editor</span>
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex min-w-0 items-center gap-3 px-3">
				<Button type="button" onClick={onAssetEditorClick} variant="toolbar">
					<PackageOpen size={14} />
					Assets
				</Button>
				<Button type="button" onClick={onProjectSettingsClick} variant="toolbar">
					<SlidersHorizontal size={14} />
					Project Settings
				</Button>
				<Button type="button" onClick={onHelpClick} variant="toolbar">
					<CircleHelp size={14} />
					Help
				</Button>
				<Badge className="px-2 py-1 text-sm font-bold" variant="medium">
					{targetRuntime.toLowerCase().includes("headless") ? "Headless" : "Desktop"}
				</Badge>
				<Badge className="px-2 py-1 text-sm font-bold" variant={getVerificationBadgeVariant(verificationStatus)}>
					{getVerificationLabel(verificationStatus)}
				</Badge>
				<div className="ml-auto flex items-center gap-2">
					<Button
						type="button"
						onClick={isSimulationRunning ? onStopSimulationClick : onSimulateClick}
						variant={isSimulationRunning ? "destructive" : "toolbar"}
					>
						{isSimulationRunning ? <Square size={14} /> : <Play size={14} />}
						{isSimulationRunning ? "Stop" : "Simulate"}
					</Button>
					<Button type="button" onClick={onVerifyClick} variant="toolbarActive">
						<ShieldCheck size={14} />
						Verify
					</Button>
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex h-full min-w-0 items-center justify-end gap-2 px-3">
				<input
					ref={importInputRef}
					className="hidden"
					type="file"
					accept=".bbs,application/zip"
					onChange={onImportFileChange}
				/>
				<Button type="button" onClick={onImportClick} variant="toolbar">
					<Upload size={14} />
					Import
				</Button>
				<Button type="button" onClick={onExportClick} variant="primary">
					<Download size={14} />
					Export
				</Button>
			</div>
		</header>
	);
}

function getVerificationBadgeVariant(status: VerificationStatus) {
	if (status === "verified") {
		return "low";
	}

	if (status === "warning") {
		return "medium";
	}

	if (status === "failed") {
		return "high";
	}

	return "outline";
}

function getVerificationLabel(status: VerificationStatus) {
	if (status === "verified") {
		return "Verified";
	}

	if (status === "warning") {
		return "Verified with warnings";
	}

	if (status === "failed") {
		return "Verification failed";
	}

	return "Not verified";
}
