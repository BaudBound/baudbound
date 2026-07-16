import {
	CircleHelp,
	Download,
	House,
	PackageOpen,
	Redo2,
	Save,
	ShieldCheck,
	SlidersHorizontal,
	Undo2,
} from "lucide-react";
import Image from "next/image";
import { Button } from "@/components/ui/button";

type TopBarProps = {
	leftCollapsed: boolean;
	leftWidth: number;
	rightCollapsed: boolean;
	rightWidth: number;
	saveDisabled: boolean;
	canRedo: boolean;
	canUndo: boolean;
	onAssetEditorClick: () => void;
	onExportClick: () => void;
	onHomeClick: () => void;
	onHelpClick: () => void;
	onProjectSettingsClick: () => void;
	onRedoClick: () => void;
	onSaveClick: () => void;
	onUndoClick: () => void;
	onVerifyClick: () => void;
};

export function TopBar({
	leftCollapsed,
	leftWidth,
	rightCollapsed,
	rightWidth,
	saveDisabled,
	canRedo,
	canUndo,
	onAssetEditorClick,
	onExportClick,
	onHomeClick,
	onHelpClick,
	onProjectSettingsClick,
	onRedoClick,
	onSaveClick,
	onUndoClick,
	onVerifyClick,
}: TopBarProps) {
	return (
		<header
			className="grid h-12 shrink-0 border-b border-baud-border bg-baud-panel"
			style={{
				gridTemplateColumns: `${leftWidth}px ${leftCollapsed ? 0 : 4}px minmax(0, 1fr) ${rightCollapsed ? 0 : 4}px ${rightWidth}px`,
			}}
		>
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
				<Button
					type="button"
					onClick={onHomeClick}
					aria-label="Return to projects"
					title="Projects"
					size="icon-sm"
					variant="toolbar"
				>
					<House />
				</Button>
				<Button
					type="button"
					onClick={onSaveClick}
					disabled={saveDisabled}
					aria-label="Save project"
					size="sm"
					variant="toolbar"
				>
					<Save size={14} />
					<span className="hidden xl:inline">Save</span>
				</Button>
				<div className="flex items-center gap-0.5">
					<Button
						type="button"
						onClick={onUndoClick}
						disabled={!canUndo}
						aria-label="Undo"
						title="Undo"
						size="icon-sm"
						variant="ghost"
					>
						<Undo2 />
					</Button>
					<Button
						type="button"
						onClick={onRedoClick}
						disabled={!canRedo}
						aria-label="Redo"
						title="Redo"
						size="icon-sm"
						variant="ghost"
					>
						<Redo2 />
					</Button>
				</div>
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
						<Button type="button" onClick={onExportClick} aria-label="Export package" size="sm" variant="primary">
							<Download size={14} />
							<span className="hidden xl:inline">Export</span>
						</Button>
					)}
				</div>
			</div>

			<div className="bg-baud-border/30" />

			<div className="flex h-full min-w-0 items-center justify-end gap-1.5 px-2">
				{!rightCollapsed && (
					<Button type="button" onClick={onExportClick} aria-label="Export package" size="sm" variant="primary">
						<Download size={14} />
						<span className="hidden xl:inline">Export</span>
					</Button>
				)}
			</div>
		</header>
	);
}
