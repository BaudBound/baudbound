import { Clipboard, Copy, CopyPlus, Trash2, Unlink2 } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";

export type CanvasContextMenuTarget =
	| {
			id: string;
			type: "node";
	  }
	| {
			id: string;
			type: "edge";
	  }
	| {
			type: "pane";
	  };

export type CanvasContextMenuState = {
	x: number;
	y: number;
	flowPosition: {
		x: number;
		y: number;
	};
	target: CanvasContextMenuTarget;
};

type CanvasContextMenuProps = {
	canPaste: boolean;
	menu: CanvasContextMenuState;
	onClose: () => void;
	onCopyNode: (nodeId: string) => void;
	onDeleteNode: (nodeId: string) => void;
	onDeleteEdge: (edgeId: string) => void;
	onDuplicateNode: (nodeId: string) => void;
	onPaste: (position: { x: number; y: number }) => void;
};

export function CanvasContextMenu({
	canPaste,
	menu,
	onClose,
	onCopyNode,
	onDeleteNode,
	onDeleteEdge,
	onDuplicateNode,
	onPaste,
}: CanvasContextMenuProps) {
	const target = menu.target;
	const runAction = (action: () => void) => {
		action();
		onClose();
	};

	return (
		<div
			className="fixed z-10000 min-w-44 rounded border border-baud-border bg-baud-panel py-1 shadow-[0_18px_48px_rgba(0,0,0,0.42)]"
			style={{ left: menu.x, top: menu.y }}
			role="menu"
			aria-label={`${getContextMenuTargetLabel(target.type)} actions`}
			onContextMenu={(event) => event.preventDefault()}
		>
			{target.type === "node" && (
				<>
					<ContextMenuButton
						icon={<Copy size={14} />}
						label="Copy"
						onClick={() => runAction(() => onCopyNode(target.id))}
					/>
					<ContextMenuButton
						icon={<CopyPlus size={14} />}
						label="Duplicate"
						onClick={() => runAction(() => onDuplicateNode(target.id))}
					/>
					<ContextMenuButton
						danger
						icon={<Trash2 size={14} />}
						label="Delete"
						onClick={() => runAction(() => onDeleteNode(target.id))}
					/>
				</>
			)}
			{target.type === "edge" && (
				<ContextMenuButton
					danger
					icon={<Unlink2 size={14} />}
					label="Disconnect"
					onClick={() => runAction(() => onDeleteEdge(target.id))}
				/>
			)}
			{target.type === "pane" && (
				<ContextMenuButton
					disabled={!canPaste}
					icon={<Clipboard size={14} />}
					label="Paste"
					onClick={() => runAction(() => onPaste(menu.flowPosition))}
				/>
			)}
		</div>
	);
}

function getContextMenuTargetLabel(type: CanvasContextMenuTarget["type"]) {
	if (type === "node") {
		return "Node";
	}

	if (type === "edge") {
		return "Edge";
	}

	return "Canvas";
}

function ContextMenuButton({
	danger,
	disabled,
	icon,
	label,
	onClick,
}: {
	danger?: boolean;
	disabled?: boolean;
	icon: ReactNode;
	label: string;
	onClick: () => void;
}) {
	return (
		<Button
			type="button"
			disabled={disabled}
			onClick={onClick}
			className={`w-full justify-start rounded-none px-3 text-left font-normal ${
				danger ? "text-baud-danger hover:bg-baud-danger/10" : "text-baud-text hover:bg-baud-soft hover:text-white"
			}`}
			role="menuitem"
			variant="ghost"
		>
			<span className="grid size-4 place-items-center">{icon}</span>
			<span>{label}</span>
		</Button>
	);
}
