"use client";

import { type Node, type NodeProps, useReactFlow, type XYPosition } from "@xyflow/react";
import { GripVertical, Trash2 } from "lucide-react";
import { createContext, type PointerEvent, useCallback, useContext, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import type { CommentNodeData, EditorComment } from "@/lib/types";
import { cn } from "@/lib/utils";

export type CommentFlowNode = Node<CommentNodeData, "commentNode">;

type CommentNodeActions = {
	onDelete: (commentId: string) => void;
	onUpdate: (commentId: string, patch: Partial<CommentNodeData>) => void;
};

export const CommentNodeActionsContext = createContext<CommentNodeActions | null>(null);

const MIN_COMMENT_WIDTH = 260;
const MIN_COMMENT_HEIGHT = 156;
const MIN_COMMENT_FONT_SIZE = 12;
const MAX_COMMENT_FONT_SIZE = 72;
export const DEFAULT_COMMENT_FONT_SIZE = 14;
export const DEFAULT_COMMENT_SIZE = {
	width: 320,
	height: 196,
};

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
	const fontSize = clampCommentFontSize(data.fontSize);
	const [draftText, setDraftText] = useState(data.text);
	const [fontSizeDraft, setFontSizeDraft] = useState(String(fontSize));
	const actionsRef = useRef(actions);
	const committedTextRef = useRef(data.text);
	const draftTextRef = useRef(data.text);
	const isEditingTextRef = useRef(false);
	const isEditingFontSizeRef = useRef(false);

	useEffect(() => {
		actionsRef.current = actions;
	}, [actions]);

	useEffect(() => {
		committedTextRef.current = data.text;
		if (!isEditingTextRef.current && draftTextRef.current !== data.text) {
			draftTextRef.current = data.text;
			setDraftText(data.text);
		}
	}, [data.text]);

	useEffect(() => {
		if (!isEditingFontSizeRef.current) {
			setFontSizeDraft(String(fontSize));
		}
	}, [fontSize]);

	const commitText = useCallback(
		(text = draftTextRef.current) => {
			if (text === committedTextRef.current) {
				return;
			}

			committedTextRef.current = text;
			actionsRef.current?.onUpdate(id, { text });
		},
		[id],
	);

	const updateFontSize = useCallback(
		(nextFontSize: number) => {
			actions?.onUpdate(id, {
				fontSize: clampCommentFontSize(nextFontSize),
			});
		},
		[actions, id],
	);

	const commitFontSizeDraft = useCallback(
		(nextDraft = fontSizeDraft) => {
			const parsedFontSize = Number.parseInt(nextDraft, 10);
			const nextFontSize = Number.isFinite(parsedFontSize) ? clampCommentFontSize(parsedFontSize) : fontSize;
			setFontSizeDraft(String(nextFontSize));
			updateFontSize(nextFontSize);
		},
		[fontSize, fontSizeDraft, updateFontSize],
	);

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
			<div className="baud-comment-drag-handle flex h-16 cursor-grab flex-col gap-1 border-b border-white/10 px-2 py-1.5 active:cursor-grabbing">
				<div className="flex min-h-0 items-center gap-2">
					<div className="rounded p-1 text-baud-muted transition" aria-hidden="true">
						<GripVertical size={15} />
					</div>
					<div className="min-w-0 flex-1 truncate text-[11px] font-semibold uppercase tracking-[0.16em] text-baud-muted">
						Comment
					</div>
					<Button
						type="button"
						size="icon"
						variant="ghost"
						className="nodrag h-6 w-6 shrink-0 text-baud-muted hover:text-rose-200"
						aria-label="Delete comment"
						onPointerDown={(event) => event.stopPropagation()}
						onClick={(event) => {
							event.stopPropagation();
							actions?.onDelete(id);
						}}
					>
						<Trash2 size={13} />
					</Button>
				</div>
				<div className="flex min-h-0 min-w-0 items-center justify-between gap-2">
					<div className="nodrag flex shrink-0 items-center gap-0.5 rounded border border-white/10 bg-black/12 p-0.5">
						<button
							type="button"
							className="grid h-5 min-w-5 place-items-center rounded px-1 text-[10px] font-semibold text-baud-muted transition hover:bg-white/10 hover:text-baud-text disabled:cursor-not-allowed disabled:opacity-40"
							title="Decrease comment font size"
							aria-label="Decrease comment font size"
							disabled={fontSize <= MIN_COMMENT_FONT_SIZE}
							onPointerDown={(event) => event.stopPropagation()}
							onClick={(event) => {
								event.stopPropagation();
								updateFontSize(fontSize - 1);
							}}
						>
							A-
						</button>
						<input
							type="text"
							inputMode="numeric"
							pattern="[0-9]*"
							className="h-5 w-8 rounded border border-transparent bg-transparent px-0.5 text-center text-[10px] font-semibold tabular-nums text-baud-muted outline-none transition focus:border-baud-red/60 focus:bg-black/20 focus:text-baud-text"
							aria-label="Comment font size"
							title={`Comment font size (${MIN_COMMENT_FONT_SIZE}-${MAX_COMMENT_FONT_SIZE})`}
							value={fontSizeDraft}
							onBlur={() => {
								isEditingFontSizeRef.current = false;
								commitFontSizeDraft();
							}}
							onChange={(event) => {
								const nextDraft = event.target.value.replace(/\D/g, "").slice(0, 3);
								setFontSizeDraft(nextDraft);
							}}
							onFocus={() => {
								isEditingFontSizeRef.current = true;
							}}
							onKeyDown={(event) => {
								if (event.key === "Enter") {
									event.currentTarget.blur();
									return;
								}

								if (event.key === "Escape") {
									setFontSizeDraft(String(fontSize));
									event.currentTarget.blur();
								}
							}}
							onPointerDown={(event) => event.stopPropagation()}
						/>
						<button
							type="button"
							className="grid h-5 min-w-5 place-items-center rounded px-1 text-[10px] font-semibold text-baud-muted transition hover:bg-white/10 hover:text-baud-text disabled:cursor-not-allowed disabled:opacity-40"
							title="Increase comment font size"
							aria-label="Increase comment font size"
							disabled={fontSize >= MAX_COMMENT_FONT_SIZE}
							onPointerDown={(event) => event.stopPropagation()}
							onClick={(event) => {
								event.stopPropagation();
								updateFontSize(fontSize + 1);
							}}
						>
							A+
						</button>
					</div>
					<fieldset className="nodrag flex min-w-0 shrink items-center justify-end gap-1 overflow-hidden">
						<legend className="sr-only">Comment color</legend>
						{colorSwatches.map((swatch) => (
							<button
								key={swatch.color}
								type="button"
								className={cn(
									"grid size-4 shrink-0 place-items-center rounded-full border border-white/20 transition hover:scale-110 hover:border-white/70",
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
				</div>
			</div>
			<textarea
				value={draftText}
				className="nowheel nodrag h-[calc(100%-4rem)] w-full resize-none bg-transparent px-3 py-2 text-sm leading-5 text-baud-text outline-none placeholder:text-baud-muted/75"
				style={{
					fontSize: `${fontSize}px`,
					lineHeight: `${Math.round(fontSize * 1.45)}px`,
				}}
				placeholder="Write a note..."
				spellCheck
				onPointerDown={(event) => event.stopPropagation()}
				onBlur={() => {
					isEditingTextRef.current = false;
					commitText();
				}}
				onChange={(event) => {
					const nextText = event.target.value;
					draftTextRef.current = nextText;
					setDraftText(nextText);
				}}
				onFocus={() => {
					isEditingTextRef.current = true;
				}}
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

export function createCommentFlowNode(position: XYPosition): CommentFlowNode {
	return toCommentFlowNode(
		{
			id: `c-${crypto.randomUUID()}`,
			text: "",
			position,
			size: DEFAULT_COMMENT_SIZE,
			color: "amber",
			fontSize: DEFAULT_COMMENT_FONT_SIZE,
		},
		true,
	);
}

export function toCommentFlowNode(comment: EditorComment, selected = false): CommentFlowNode {
	return {
		id: comment.id,
		type: "commentNode",
		position: comment.position,
		data: {
			editorOnly: true,
			text: comment.text,
			size: comment.size,
			color: comment.color,
			fontSize: comment.fontSize,
		},
		style: {
			width: comment.size.width,
			height: comment.size.height,
		},
		className: "baud-comment-flow-node",
		connectable: false,
		deletable: true,
		draggable: true,
		dragHandle: ".baud-comment-drag-handle",
		selectable: true,
		selected,
		zIndex: 5,
	};
}

export function toEditorComment(node: CommentFlowNode): EditorComment {
	return {
		id: node.id,
		text: node.data.text,
		position: node.position,
		size: node.data.size,
		color: node.data.color,
		fontSize: node.data.fontSize,
	};
}

export function isCommentFlowNode(node: Node): node is CommentFlowNode {
	return node.type === "commentNode";
}

export function areCommentNodeDataEqual(first: CommentNodeData, second: CommentNodeData) {
	return (
		first.text === second.text &&
		first.color === second.color &&
		first.fontSize === second.fontSize &&
		first.size.width === second.size.width &&
		first.size.height === second.size.height
	);
}

function clampCommentFontSize(fontSize: number) {
	if (!Number.isFinite(fontSize)) {
		return DEFAULT_COMMENT_FONT_SIZE;
	}

	return Math.min(MAX_COMMENT_FONT_SIZE, Math.max(MIN_COMMENT_FONT_SIZE, fontSize));
}
