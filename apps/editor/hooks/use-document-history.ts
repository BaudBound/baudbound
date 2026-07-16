"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const HISTORY_LIMIT = 100;
const COALESCE_DELAY_MS = 300;

type HistoryEntry<T> = { signature: string; value: T };
type PendingChange<T> = {
	base: HistoryEntry<T>;
	coalesceKey: string;
	latest: HistoryEntry<T>;
	timer: ReturnType<typeof setTimeout>;
};

export function useDocumentHistory<T>({
	signature,
	value,
	onRestore,
	getCoalesceKey,
}: {
	signature: string;
	value: T;
	onRestore: (value: T) => void;
	getCoalesceKey: (previous: T, next: T) => string | null;
}) {
	const currentRef = useRef<HistoryEntry<T>>({ signature, value });
	const pastRef = useRef<HistoryEntry<T>[]>([]);
	const futureRef = useRef<HistoryEntry<T>[]>([]);
	const pendingRef = useRef<PendingChange<T> | null>(null);
	const restoredSignatureRef = useRef<string | null>(null);
	const onRestoreRef = useRef(onRestore);
	const [availability, setAvailability] = useState({ canRedo: false, canUndo: false });

	onRestoreRef.current = onRestore;

	const updateAvailability = useCallback(() => {
		setAvailability({
			canRedo: futureRef.current.length > 0,
			canUndo: pastRef.current.length > 0 || pendingRef.current !== null,
		});
	}, []);

	const commitPending = useCallback(() => {
		const pending = pendingRef.current;
		if (!pending) return;
		clearTimeout(pending.timer);
		pendingRef.current = null;
		pastRef.current = appendLimited(pastRef.current, pending.base);
		currentRef.current = pending.latest;
		futureRef.current = [];
		updateAvailability();
	}, [updateAvailability]);

	useEffect(() => {
		const next = { signature, value };
		if (restoredSignatureRef.current === signature) {
			restoredSignatureRef.current = null;
			currentRef.current = next;
			return;
		}
		if (pendingRef.current) {
			const key = getCoalesceKey(pendingRef.current.latest.value, next.value);
			if (key === pendingRef.current.coalesceKey) {
				clearTimeout(pendingRef.current.timer);
				pendingRef.current.latest = next;
				pendingRef.current.timer = setTimeout(commitPending, COALESCE_DELAY_MS);
				return;
			}
			commitPending();
		}
		if (currentRef.current.signature === signature) {
			currentRef.current = next;
			return;
		}

		const coalesceKey = getCoalesceKey(currentRef.current.value, next.value);
		if (!coalesceKey) {
			pastRef.current = appendLimited(pastRef.current, currentRef.current);
			currentRef.current = next;
			futureRef.current = [];
			updateAvailability();
			return;
		}
		pendingRef.current = {
			base: currentRef.current,
			coalesceKey,
			latest: next,
			timer: setTimeout(commitPending, COALESCE_DELAY_MS),
		};
		futureRef.current = [];
		updateAvailability();
	}, [commitPending, getCoalesceKey, signature, updateAvailability, value]);

	useEffect(
		() => () => {
			if (pendingRef.current) clearTimeout(pendingRef.current.timer);
		},
		[],
	);

	const restore = useCallback((entry: HistoryEntry<T>) => {
		restoredSignatureRef.current = entry.signature;
		currentRef.current = entry;
		onRestoreRef.current(entry.value);
	}, []);

	const undo = useCallback(() => {
		const pending = pendingRef.current;
		if (pending) {
			clearTimeout(pending.timer);
			pendingRef.current = null;
			futureRef.current = appendLimited(futureRef.current, pending.latest);
			restore(pending.base);
			updateAvailability();
			return;
		}

		const target = pastRef.current.at(-1);
		if (!target) return;
		pastRef.current = pastRef.current.slice(0, -1);
		futureRef.current = appendLimited(futureRef.current, currentRef.current);
		restore(target);
		updateAvailability();
	}, [restore, updateAvailability]);

	const redo = useCallback(() => {
		commitPending();
		const target = futureRef.current.at(-1);
		if (!target) return;
		futureRef.current = futureRef.current.slice(0, -1);
		pastRef.current = appendLimited(pastRef.current, currentRef.current);
		restore(target);
		updateAvailability();
	}, [commitPending, restore, updateAvailability]);

	return { ...availability, redo, undo };
}

function appendLimited<T>(entries: HistoryEntry<T>[], entry: HistoryEntry<T>) {
	const next = [...entries, entry];
	return next.length > HISTORY_LIMIT ? next.slice(next.length - HISTORY_LIMIT) : next;
}
