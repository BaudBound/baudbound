"use client";

import { type Node, type NodeProps, useReactFlow } from "@xyflow/react";
import { GripVertical, Trash2 } from "lucide-react";
import { createContext, type PointerEvent, useCallback, useContext } from "react";
import { Button } from "@/components/ui/button";
import type { CommentNodeData } from "@/lib/types";
import { cn } from "@/lib/utils";

export type CommentFlowNode = Node<CommentNodeData, "commentNode">;

type CommentNodeActions = {
	onDelete: (commentId: string) => void;
	onUpdate: (commentId: string, patch: Partial<CommentNodeData>) => void;
};

export const CommentNodeActionsContext = createContext<CommentNodeActions | null>(null);

const MIN_COMMENT_WIDTH = 220;
const MIN_COMMENT_HEIGHT = 116;

const colorClasses: Record<CommentNodeData["color"], string> = {
	amber: "border-amber-400/60 bg-amber-950/34 shadow-amber-500/10",
	blue: "border-blue-400/60 bg-blue-950/32 shadow-blue-500/10",
	green: "border-emerald-400/60 bg-emerald-950/30 shadow-emerald-500/10",
	rose: "border-rose-400/60 bg-rose-950/32 shadow-rose-500/10",
	violet: "border-violet-400/60 bg-violet-950/32 shadow-violet-500/10",
};

const colorSwatches: Array<{
	color: CommentNodeData["color"];
	label: string;
	className: string;
}> = [
	{ color: "amber", label: "Amber", className: "bg-amber-300" },
	{ color: "blue", label: "Blue", className: "bg-blue-300" },
	{ color: "green", label: "Green", className: "bg-emerald-300" },
	{ color: "rose", label: "Rose", className: "bg-rose-300" },
	{ color: "violet", label: "Violet", className: "bg-violet-300" },
];

export function CommentCard({ data, id, selected }: NodeProps<CommentFlowNode>) {
	const actions = useContext(CommentNodeActionsContext);
	const { screenToFlowPosition } = useReactFlow();

	const startResize = useCallback(
		(event: PointerEvent<HTMLButtonElement>) => {
			event.preventDefault();
			event.stopPropagation();

			const startPointer = screenToFlowPosition({ x: event.clientX, y: event.clientY });
			const startSize = data.size;

			const handlePointerMove = (moveEvent: globalThis.PointerEvent) => {
				const nextPointer = screenToFlowPosition({ x: moveEvent.clientX, y: moveEvent.clientY });
				actions?.onUpdate(id, {
					size: {
						width: Math.max(MIN_COMMENT_WIDTH, startSize.width + nextPointer.x - startPointer.x),
						height: Math.max(MIN_COMMENT_HEIGHT, startSize.height + nextPointer.y - startPointer.y),
					},
				});
			};

			const handlePointerUp = () => {
				window.removeEventListener("pointermove", handlePointerMove);
				window.removeEventListener("pointerup", handlePointerUp);
				window.removeEventListener("pointercancel", handlePointerUp);
			};

			window.addEventListener("pointermove", handlePointerMove);
			window.addEventListener("pointerup", handlePointerUp, { once: true });
			window.addEventListener("pointercancel", handlePointerUp, { once: true });
		},
		[actions, data.size, id, screenToFlowPosition],
	);

	return (
		<div
			className={cn(
				"baud-comment-card nowheel pointer-events-auto h-full w-full rounded-md border text-baud-text shadow-lg backdrop-blur-sm",
				colorClasses[data.color],
				selected && "border-baud-red ring-2 ring-baud-red/35",
			)}
		>
			<div className="baud-comment-drag-handle flex h-9 cursor-grab items-center gap-2 border-b border-white/10 px-2 active:cursor-grabbing">
				<div className="rounded p-1 text-baud-muted transition" aria-hidden="true">
					<GripVertical size={15} />
				</div>
				<div className="min-w-0 flex-1 text-xs font-semibold uppercase tracking-[0.16em] text-baud-muted">Comment</div>
				<fieldset className="nodrag flex items-center gap-1">
					<legend className="sr-only">Comment color</legend>
					{colorSwatches.map((swatch) => (
						<button
							key={swatch.color}
							type="button"
							className={cn(
								"grid size-4 place-items-center rounded-full border border-white/20 transition hover:scale-110 hover:border-white/70",
								data.color === swatch.color && "border-white ring-2 ring-white/25",
							)}
							title={`Set comment color to ${swatch.label}`}
							aria-label={`Set comment color to ${swatch.label}`}
							aria-pressed={data.color === swatch.color}
							onPointerDown={(event) => event.stopPropagation()}
							onClick={(event) => {
								event.stopPropagation();
								actions?.onUpdate(id, { color: swatch.color });
							}}
						>
							<span className={cn("size-2.5 rounded-full", swatch.className)} />
						</button>
					))}
				</fieldset>
				<Button
					type="button"
					size="icon"
					variant="ghost"
					className="nodrag h-7 w-7 text-baud-muted hover:text-rose-200"
					aria-label="Delete comment"
					onPointerDown={(event) => event.stopPropagation()}
					onClick={(event) => {
						event.stopPropagation();
						actions?.onDelete(id);
					}}
				>
					<Trash2 size={14} />
				</Button>
			</div>
			<textarea
				value={data.text}
				className="nowheel nodrag h-[calc(100%-2.25rem)] w-full resize-none bg-transparent px-3 py-2 text-sm leading-5 text-baud-text outline-none placeholder:text-baud-muted/75"
				placeholder="Write a note..."
				spellCheck
				onPointerDown={(event) => event.stopPropagation()}
				onChange={(event) => actions?.onUpdate(id, { text: event.target.value })}
			/>
			<button
				type="button"
				className="nodrag absolute right-1 bottom-1 h-4 w-4 cursor-nwse-resize rounded-sm border-b-2 border-r-2 border-baud-muted/70 opacity-80 transition hover:border-baud-text"
				aria-label="Resize comment"
				onPointerDown={startResize}
			/>
		</div>
	);
}
