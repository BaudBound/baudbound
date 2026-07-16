import { CircleHelp, Download, PackageOpen, ShieldCheck, SlidersHorizontal, Upload } from "lucide-react";
import Image from "next/image";
import type { ChangeEvent, RefObject } from "react";
import { Button } from "@/components/ui/button";

type TopBarProps = {
	importInputRef: RefObject<HTMLInputElement | null>;
	leftCollapsed: boolean;
	leftWidth: number;
	rightCollapsed: boolean;
	rightWidth: number;
	onAssetEditorClick: () => void;
	onImportClick: () => void;
	onImportFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
	onExportClick: () => void;
	onHelpClick: () => void;
	onProjectSettingsClick: () => void;
	onVerifyClick: () => void;
};

export function TopBar({
	importInputRef,
	leftCollapsed,
	leftWidth,
	rightCollapsed,
	rightWidth,
	onAssetEditorClick,
	onImportClick,
	onImportFileChange,
	onExportClick,
	onHelpClick,
	onProjectSettingsClick,
	onVerifyClick,
}: TopBarProps) {
	return (
		<header
			className="grid h-12 shrink-0 border-b border-baud-border bg-baud-panel"
			style={{
				gridTemplateColumns: `${leftWidth}px ${leftCollapsed ? 0 : 4}px minmax(0, 1fr) ${rightCollapsed ? 0 : 4}px ${rightWidth}px`,
			}}
		>
			<input
				ref={importInputRef}
				className="hidden"
				type="file"
				accept=".bbs,application/zip"
				onChange={onImportFileChange}
			/>

			<div className={`flex h-full min-w-0 items-center ${leftCollapsed ? "justify-center px-1" : "gap-2 px-3"}`}>
				<Image
					src="/logo-notext.svg"
					alt=""
					width={28}
					height={28}
					priority
					aria-hidden="true"
					className="size-7 shrink-0 rounded object-contain"
				/>
				{!leftCollapsed && (
					<div className="min-w-0 text-sm font-semibold">
						<span className="text-white">BaudBound</span>{" "}
						<span className="hidden text-baud-muted xl:inline">Editor</span>
					</div>
				)}
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex min-w-0 items-center gap-2 overflow-hidden px-2">
				<Button type="button" onClick={onAssetEditorClick} aria-label="Open asset editor" size="sm" variant="toolbar">
					<PackageOpen size={14} />
					<span className="hidden xl:inline">Assets</span>
				</Button>
				<Button
					type="button"
					onClick={onProjectSettingsClick}
					aria-label="Open project settings"
					size="sm"
					variant="toolbar"
				>
					<SlidersHorizontal size={14} />
					<span className="hidden 2xl:inline">Project Settings</span>
				</Button>
				<Button type="button" onClick={onHelpClick} aria-label="Open help" size="sm" variant="toolbar">
					<CircleHelp size={14} />
					<span className="hidden xl:inline">Help</span>
				</Button>
				<div className="ml-auto flex min-w-0 shrink-0 items-center gap-1.5">
					<Button type="button" onClick={onVerifyClick} aria-label="Verify script" size="sm" variant="toolbarActive">
						<ShieldCheck size={14} />
						<span className="hidden xl:inline">Verify</span>
					</Button>
					{rightCollapsed && (
						<>
							<Button type="button" onClick={onImportClick} aria-label="Import package" size="sm" variant="toolbar">
								<Upload size={14} />
								<span className="hidden xl:inline">Import</span>
							</Button>
							<Button type="button" onClick={onExportClick} aria-label="Export package" size="sm" variant="primary">
								<Download size={14} />
								<span className="hidden xl:inline">Export</span>
							</Button>
						</>
					)}
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex h-full min-w-0 items-center justify-end gap-1.5 px-2">
				{!rightCollapsed && (
					<>
						<Button type="button" onClick={onImportClick} aria-label="Import package" size="sm" variant="toolbar">
							<Upload size={14} />
							<span className="hidden xl:inline">Import</span>
						</Button>
						<Button type="button" onClick={onExportClick} aria-label="Export package" size="sm" variant="primary">
							<Download size={14} />
							<span className="hidden xl:inline">Export</span>
						</Button>
					</>
				)}
			</div>
		</header>
	);
}
