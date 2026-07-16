"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const channelName = "baudbound-editor-project-sessions-v1";
const claimDelayMs = 250;
const heartbeatIntervalMs = 2_000;
const ownerTimeoutMs = 6_000;

type SessionStatus = "checking" | "owner" | "occupied";

type SessionMessage = {
	dirty?: boolean;
	projectId: string;
	requestId?: string;
	sessionId: string;
	type: "probe" | "claim" | "heartbeat" | "release" | "takeover-request" | "takeover-granted" | "takeover-denied";
};

export type ProjectSession = {
	coordinationAvailable: boolean;
	requestTakeover: () => void;
	status: SessionStatus;
	takeoverError: string | null;
};

export function useProjectSession(projectId: string, dirty: boolean): ProjectSession {
	const dirtyRef = useRef(dirty);
	const channelRef = useRef<BroadcastChannel | null>(null);
	const sessionIdRef = useRef<string | null>(null);
	const ownerSessionRef = useRef<string | null>(null);
	const statusRef = useRef<SessionStatus>("checking");
	const lastOwnerSignalRef = useRef(0);
	const pendingTakeoverRef = useRef<string | null>(null);
	const [status, setStatus] = useState<SessionStatus>("checking");
	const [coordinationAvailable, setCoordinationAvailable] = useState(true);
	const [takeoverError, setTakeoverError] = useState<string | null>(null);

	dirtyRef.current = dirty;

	useEffect(() => {
		const sessionId = crypto.randomUUID();
		sessionIdRef.current = sessionId;
		let claimTimer: ReturnType<typeof setTimeout> | null = null;
		let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
		let staleOwnerTimer: ReturnType<typeof setInterval> | null = null;
		let disposed = false;

		const updateStatus = (nextStatus: SessionStatus, ownerSession: string | null = null) => {
			statusRef.current = nextStatus;
			ownerSessionRef.current = ownerSession;
			setStatus(nextStatus);
		};

		if (typeof BroadcastChannel === "undefined") {
			setCoordinationAvailable(false);
			updateStatus("owner");
			return;
		}

		const channel = new BroadcastChannel(channelName);
		channelRef.current = channel;
		setCoordinationAvailable(true);

		const post = (message: Omit<SessionMessage, "projectId" | "sessionId">) => {
			if (!disposed) channel.postMessage({ ...message, projectId, sessionId } satisfies SessionMessage);
		};
		const announceOwnership = () => post({ dirty: dirtyRef.current, type: "heartbeat" });
		const becomeOwner = () => {
			if (claimTimer) clearTimeout(claimTimer);
			claimTimer = null;
			updateStatus("owner");
			setTakeoverError(null);
			post({ type: "claim" });
			announceOwnership();
		};
		const scheduleClaim = () => {
			if (claimTimer) clearTimeout(claimTimer);
			updateStatus("checking");
			post({ type: "probe" });
			claimTimer = setTimeout(() => {
				if (statusRef.current === "checking") becomeOwner();
			}, claimDelayMs);
		};
		const observeOwner = (ownerSession: string) => {
			lastOwnerSignalRef.current = Date.now();
			if (claimTimer) clearTimeout(claimTimer);
			claimTimer = null;
			updateStatus("occupied", ownerSession);
		};

		channel.addEventListener("message", (event: MessageEvent<unknown>) => {
			const message = parseSessionMessage(event.data);
			if (!message || message.projectId !== projectId || message.sessionId === sessionId) return;

			switch (message.type) {
				case "probe":
					if (statusRef.current === "owner") announceOwnership();
					break;
				case "claim":
				case "heartbeat":
					if (statusRef.current !== "owner") {
						observeOwner(message.sessionId);
						break;
					}
					if (message.sessionId < sessionId && !dirtyRef.current) {
						observeOwner(message.sessionId);
					} else {
						announceOwnership();
					}
					break;
				case "release":
					if (statusRef.current === "occupied" && ownerSessionRef.current === message.sessionId) scheduleClaim();
					break;
				case "takeover-request":
					if (statusRef.current !== "owner" || !message.requestId) break;
					if (dirtyRef.current) {
						post({ requestId: message.requestId, type: "takeover-denied" });
					} else {
						observeOwner(message.sessionId);
						post({ requestId: message.requestId, type: "takeover-granted" });
					}
					break;
				case "takeover-granted":
					if (message.requestId && pendingTakeoverRef.current === message.requestId) {
						pendingTakeoverRef.current = null;
						becomeOwner();
					}
					break;
				case "takeover-denied":
					if (message.requestId && pendingTakeoverRef.current === message.requestId) {
						pendingTakeoverRef.current = null;
						setTakeoverError("The other tab has unsaved changes. Save or discard them there before taking control.");
					}
					break;
			}
		});

		heartbeatTimer = setInterval(() => {
			if (statusRef.current === "owner") announceOwnership();
		}, heartbeatIntervalMs);
		staleOwnerTimer = setInterval(() => {
			if (statusRef.current === "occupied" && Date.now() - lastOwnerSignalRef.current > ownerTimeoutMs) {
				scheduleClaim();
			}
		}, heartbeatIntervalMs);
		scheduleClaim();

		return () => {
			disposed = true;
			if (statusRef.current === "owner") {
				channel.postMessage({ projectId, sessionId, type: "release" } satisfies SessionMessage);
			}
			if (claimTimer) clearTimeout(claimTimer);
			if (heartbeatTimer) clearInterval(heartbeatTimer);
			if (staleOwnerTimer) clearInterval(staleOwnerTimer);
			channel.close();
			channelRef.current = null;
			sessionIdRef.current = null;
		};
	}, [projectId]);

	const requestTakeover = useCallback(() => {
		if (statusRef.current !== "occupied" || !channelRef.current || !sessionIdRef.current) return;
		const requestId = crypto.randomUUID();
		pendingTakeoverRef.current = requestId;
		setTakeoverError(null);
		channelRef.current.postMessage({
			projectId,
			requestId,
			sessionId: sessionIdRef.current,
			type: "takeover-request",
		} satisfies SessionMessage);
	}, [projectId]);

	return { coordinationAvailable, requestTakeover, status, takeoverError };
}

function parseSessionMessage(value: unknown): SessionMessage | null {
	if (!value || typeof value !== "object") return null;
	const message = value as Partial<SessionMessage>;
	if (typeof message.projectId !== "string" || typeof message.sessionId !== "string") return null;
	if (
		message.type !== "probe" &&
		message.type !== "claim" &&
		message.type !== "heartbeat" &&
		message.type !== "release" &&
		message.type !== "takeover-request" &&
		message.type !== "takeover-granted" &&
		message.type !== "takeover-denied"
	) {
		return null;
	}
	if (message.requestId !== undefined && typeof message.requestId !== "string") return null;
	return message as SessionMessage;
}
