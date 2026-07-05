import { type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";

type ReorderDragBase = {
	draggedId: string;
	cardHeight: number;
	cardWidth: number;
	pointerX: number;
	pointerY: number;
	pointerOffsetX: number;
	pointerOffsetY: number;
	pointerId: number;
	startX: number;
	startY: number;
};

type PendingReorderDragState = ReorderDragBase & {
	mode: "pending";
};

export type ActiveReorderDragState = ReorderDragBase & {
	mode: "active";
	dropIndex: number;
};

type ReorderDragState = PendingReorderDragState | ActiveReorderDragState;

type ReorderEntry<Row extends { id: string }> =
	| { type: "row"; row: Row }
	| { type: "drop-space"; id: string; height: number };

const REORDER_ACTIVATION_DISTANCE = 5;

export function useReorderController<Row extends { id: string }>({
	rows,
	onCommit,
}: {
	rows: Row[];
	onCommit: (rows: Row[]) => void;
}) {
	const listRef = useRef<HTMLUListElement>(null);
	const rowElementsRef = useRef(new Map<string, HTMLElement>());
	const dragRef = useRef<ReorderDragState | null>(null);
	const rowsRef = useRef(rows);
	const onCommitRef = useRef(onCommit);
	const animationFrameRef = useRef<number | null>(null);
	const queuedVisualDragRef = useRef<ReorderDragState | null>(null);
	const [drag, setDrag] = useState<ReorderDragState | null>(null);

	rowsRef.current = rows;
	onCommitRef.current = onCommit;

	const activeDrag = drag?.mode === "active" ? drag : null;
	const entries = createReorderEntries(rows, activeDrag);

	useEffect(() => {
		if (!drag) {
			return;
		}

		const previousUserSelect = document.body.style.userSelect;
		const previousCursor = document.body.style.cursor;
		document.body.style.userSelect = "none";
		document.body.style.cursor = "grabbing";

		const commitVisualDragState = (nextDrag: ReorderDragState | null) => {
			queuedVisualDragRef.current = nextDrag;

			if (animationFrameRef.current !== null) {
				return;
			}

			animationFrameRef.current = window.requestAnimationFrame(() => {
				animationFrameRef.current = null;
				setDrag(queuedVisualDragRef.current);
			});
		};

		const cancelQueuedVisualDragState = () => {
			if (animationFrameRef.current !== null) {
				window.cancelAnimationFrame(animationFrameRef.current);
				animationFrameRef.current = null;
			}
			queuedVisualDragRef.current = null;
		};

		const handlePointerMove = (event: PointerEvent) => {
			const current = dragRef.current;
			if (!current || event.pointerId !== current.pointerId) {
				return;
			}

			event.preventDefault();
			const nextDrag = getNextReorderDrag(
				current,
				event.clientX,
				event.clientY,
				rowsRef.current,
				rowElementsRef.current,
			);
			dragRef.current = nextDrag;

			if (nextDrag.mode === "active") {
				commitVisualDragState(nextDrag);
			}
		};

		const handlePointerEnd = (event: PointerEvent) => {
			const current = dragRef.current;
			if (!current || event.pointerId !== current.pointerId) {
				return;
			}

			event.preventDefault();
			if (current.mode === "active") {
				onCommitRef.current(moveRowToIndex(rowsRef.current, current.draggedId, current.dropIndex));
			}

			dragRef.current = null;
			cancelQueuedVisualDragState();
			setDrag(null);
		};

		const handleKeyDown = (event: globalThis.KeyboardEvent) => {
			if (event.key !== "Escape") {
				return;
			}

			dragRef.current = null;
			cancelQueuedVisualDragState();
			setDrag(null);
		};

		window.addEventListener("pointermove", handlePointerMove, { passive: false });
		window.addEventListener("pointerup", handlePointerEnd, { passive: false });
		window.addEventListener("pointercancel", handlePointerEnd, { passive: false });
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			if (animationFrameRef.current !== null) {
				window.cancelAnimationFrame(animationFrameRef.current);
				animationFrameRef.current = null;
			}
			queuedVisualDragRef.current = null;
			document.body.style.userSelect = previousUserSelect;
			document.body.style.cursor = previousCursor;
			window.removeEventListener("pointermove", handlePointerMove);
			window.removeEventListener("pointerup", handlePointerEnd);
			window.removeEventListener("pointercancel", handlePointerEnd);
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, [drag?.pointerId]);

	return {
		drag: activeDrag,
		entries,
		listRef,
		registerRow: (id: string) => (element: HTMLElement | null) => {
			if (element) {
				rowElementsRef.current.set(id, element);
				return;
			}

			rowElementsRef.current.delete(id);
		},
		startDrag: (id: string, event: ReactPointerEvent<HTMLButtonElement>) => {
			if (event.button !== 0) {
				return;
			}

			const sourceElement = rowElementsRef.current.get(id);
			if (!sourceElement) {
				return;
			}

			event.preventDefault();
			event.stopPropagation();
			const sourceRect = sourceElement.getBoundingClientRect();
			const initialDrag: PendingReorderDragState = {
				mode: "pending",
				draggedId: id,
				cardHeight: sourceRect.height,
				cardWidth: sourceRect.width,
				pointerX: event.clientX,
				pointerY: event.clientY,
				pointerOffsetX: event.clientX - sourceRect.left,
				pointerOffsetY: event.clientY - sourceRect.top,
				pointerId: event.pointerId,
				startX: event.clientX,
				startY: event.clientY,
			};

			dragRef.current = initialDrag;
			setDrag(initialDrag);
		},
	};
}

function getNextReorderDrag<Row extends { id: string }>(
	current: ReorderDragState,
	pointerX: number,
	pointerY: number,
	rows: Row[],
	rowElements: Map<string, HTMLElement>,
): ReorderDragState {
	const movedDistance = Math.hypot(pointerX - current.startX, pointerY - current.startY);
	if (current.mode === "pending" && movedDistance < REORDER_ACTIVATION_DISTANCE) {
		return current;
	}

	return {
		...current,
		mode: "active",
		pointerX,
		pointerY,
		dropIndex: getClosestDropIndex(rows, current.draggedId, pointerY, rowElements),
	};
}

function createReorderEntries<Row extends { id: string }>(
	rows: Row[],
	drag: ActiveReorderDragState | null,
): Array<ReorderEntry<Row>> {
	if (!drag) {
		return rows.map((row) => ({ type: "row", row }));
	}

	const remainingRows = rows.filter((row) => row.id !== drag.draggedId);
	const nextEntries: Array<ReorderEntry<Row>> = remainingRows.map((row) => ({ type: "row", row }));
	nextEntries.splice(clampIndex(drag.dropIndex, 0, remainingRows.length), 0, {
		type: "drop-space",
		id: `${drag.draggedId}-drop-space`,
		height: drag.cardHeight,
	});

	return nextEntries;
}

function getClosestDropIndex<Row extends { id: string }>(
	rows: Row[],
	draggedId: string,
	pointerY: number,
	rowElements: Map<string, HTMLElement>,
) {
	const remainingRows = rows.filter((row) => row.id !== draggedId);

	for (let index = 0; index < remainingRows.length; index += 1) {
		const element = rowElements.get(remainingRows[index].id);
		if (!element) {
			continue;
		}

		const rect = element.getBoundingClientRect();
		if (pointerY < rect.top + rect.height / 2) {
			return index;
		}
	}

	return remainingRows.length;
}

function moveRowToIndex<Row extends { id: string }>(rows: Row[], draggedId: string, targetIndex: number) {
	const currentIndex = rows.findIndex((row) => row.id === draggedId);
	if (currentIndex === -1) {
		return rows;
	}

	const remainingRows = rows.filter((row) => row.id !== draggedId);
	const [draggedRow] = rows.slice(currentIndex, currentIndex + 1);
	const nextRows = [...remainingRows];
	nextRows.splice(clampIndex(targetIndex, 0, nextRows.length), 0, draggedRow);

	return nextRows;
}

function clampIndex(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}
