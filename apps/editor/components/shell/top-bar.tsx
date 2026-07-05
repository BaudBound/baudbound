import { CircleHelp, Download, PackageOpen, Play, ShieldCheck, SlidersHorizontal, Square, Upload } from "lucide-react";
import Image from "next/image";
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
				<Image
					src="/logo-notext.png"
					alt=""
					width={28}
					height={28}
					priority
					aria-hidden="true"
					className="size-7 shrink-0 rounded object-contain"
				/>
				<div className="min-w-0 text-sm font-semibold">
					<span className="text-white">BaudBound</span> <span className="text-baud-muted">Editor</span>
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex min-w-0 items-center gap-2 overflow-hidden px-2">
				<Button type="button" onClick={onAssetEditorClick} size="sm" variant="toolbar">
					<PackageOpen size={14} />
					Assets
				</Button>
				<Button type="button" onClick={onProjectSettingsClick} size="sm" variant="toolbar">
					<SlidersHorizontal size={14} />
					Project Settings
				</Button>
				<Button type="button" onClick={onHelpClick} size="sm" variant="toolbar">
					<CircleHelp size={14} />
					Help
				</Button>
				<Badge className="px-1.5 py-0.5 text-xs font-bold" variant="medium">
					{targetRuntime.toLowerCase().includes("headless") ? "Headless" : "Desktop"}
				</Badge>
				<Badge className="px-1.5 py-0.5 text-xs font-bold" variant={getVerificationBadgeVariant(verificationStatus)}>
					{getVerificationLabel(verificationStatus)}
				</Badge>
				<div className="ml-auto flex min-w-0 shrink-0 items-center gap-1.5">
					<Button
						type="button"
						onClick={isSimulationRunning ? onStopSimulationClick : onSimulateClick}
						size="sm"
						variant={isSimulationRunning ? "destructive" : "toolbar"}
					>
						{isSimulationRunning ? <Square size={14} /> : <Play size={14} />}
						{isSimulationRunning ? "Stop" : "Simulate"}
					</Button>
					<Button type="button" onClick={onVerifyClick} size="sm" variant="toolbarActive">
						<ShieldCheck size={14} />
						Verify
					</Button>
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex h-full min-w-0 items-center justify-end gap-1.5 px-2">
				<input
					ref={importInputRef}
					className="hidden"
					type="file"
					accept=".bbs,application/zip"
					onChange={onImportFileChange}
				/>
				<Button type="button" onClick={onImportClick} size="sm" variant="toolbar">
					<Upload size={14} />
					Import
				</Button>
				<Button type="button" onClick={onExportClick} size="sm" variant="primary">
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
