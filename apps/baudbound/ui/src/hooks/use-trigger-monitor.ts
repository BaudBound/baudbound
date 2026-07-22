import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  clearTriggerMonitor,
  getTriggerMonitorState,
  startTriggerMonitor,
  stopTriggerMonitor,
  type TriggerMonitorEvent,
  type TriggerMonitorState,
} from "@/lib/runner-api";
import {
  appendTriggerMonitorEvents,
  triggerMonitorEventLimit,
} from "@/lib/trigger-monitor-events";

const triggerMonitorEventChannel = "runner-trigger-monitor";

export function useTriggerMonitor() {
  const [monitorState, setMonitorState] = useState<TriggerMonitorState>({
    enabled: false,
    omitted_event_count: 0,
    session_id: 0,
  });
  const [events, setEvents] = useState<TriggerMonitorEvent[]>([]);
  const [paused, setPaused] = useState(false);
  const [omittedEventCount, setOmittedEventCount] = useState(0);
  const [pausedOmittedEventCount, setPausedOmittedEventCount] = useState(0);
  const [receivedEventCount, setReceivedEventCount] = useState(0);
  const [initializationError, setInitializationError] = useState<string | null>(
    null,
  );
  const stateRef = useRef(monitorState);
  const pausedRef = useRef(paused);
  const pausedEventsRef = useRef<TriggerMonitorEvent[]>([]);
  const transitionEventsRef = useRef<TriggerMonitorEvent[]>([]);
  const transitionPendingRef = useRef(true);

  const installState = useCallback((state: TriggerMonitorState) => {
    stateRef.current = state;
    setMonitorState(state);
  }, []);

  const ingestEvents = useCallback((incoming: TriggerMonitorEvent[]) => {
    if (incoming.length === 0) return;
    setOmittedEventCount((current) =>
      incoming.reduce(
        (total, event) => total + event.omitted_event_count,
        current,
      ),
    );
    setReceivedEventCount((current) => current + incoming.length);
    if (pausedRef.current) {
      const pending = [...pausedEventsRef.current, ...incoming];
      const overflow = Math.max(0, pending.length - triggerMonitorEventLimit);
      if (overflow > 0) {
        setPausedOmittedEventCount((current) => current + overflow);
      }
      pausedEventsRef.current = pending.slice(-triggerMonitorEventLimit);
      return;
    }
    setEvents((current) => appendTriggerMonitorEvents(current, incoming));
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen<TriggerMonitorEvent>(triggerMonitorEventChannel, ({ payload }) => {
      if (transitionPendingRef.current) {
        transitionEventsRef.current = appendTriggerMonitorEvents(
          transitionEventsRef.current,
          [payload],
        );
        return;
      }
      const current = stateRef.current;
      if (!current.enabled || payload.session_id !== current.session_id) return;
      ingestEvents([payload]);
    })
      .then((cleanup) => {
        if (disposed) {
          cleanup();
          return;
        }
        unlisten = cleanup;
        return getTriggerMonitorState();
      })
      .then((state) => {
        if (disposed || !state) return;
        installState(state);
        transitionPendingRef.current = false;
        if (state.enabled) {
          const matching = transitionEventsRef.current.filter(
            (event) => event.session_id === state.session_id,
          );
          ingestEvents(matching);
        }
        transitionEventsRef.current = [];
        setInitializationError(null);
      })
      .catch((error) => {
        if (!disposed) {
          transitionPendingRef.current = false;
          setInitializationError(String(error));
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [ingestEvents, installState]);

  const start = useCallback(async () => {
    transitionPendingRef.current = true;
    transitionEventsRef.current = [];
    const state = await startTriggerMonitor();
    installState(state);
    setEvents([]);
    setOmittedEventCount(0);
    setPausedOmittedEventCount(0);
    setReceivedEventCount(0);
    pausedEventsRef.current = [];
    pausedRef.current = false;
    setPaused(false);
    transitionPendingRef.current = false;
    const matching = transitionEventsRef.current.filter(
      (event) => event.session_id === state.session_id,
    );
    transitionEventsRef.current = [];
    ingestEvents(matching);
  }, [ingestEvents, installState]);

  const stop = useCallback(async () => {
    const state = await stopTriggerMonitor();
    if (state.omitted_event_count > 0) {
      setOmittedEventCount((current) =>
        current + state.omitted_event_count,
      );
    }
    installState(state);
    pausedEventsRef.current = [];
    pausedRef.current = false;
    setPaused(false);
  }, [installState]);

  const clear = useCallback(async () => {
    transitionPendingRef.current = true;
    transitionEventsRef.current = [];
    const state = await clearTriggerMonitor();
    installState(state);
    setEvents([]);
    setOmittedEventCount(0);
    setPausedOmittedEventCount(0);
    setReceivedEventCount(0);
    pausedEventsRef.current = [];
    transitionPendingRef.current = false;
    const matching = transitionEventsRef.current.filter(
      (event) => event.session_id === state.session_id,
    );
    transitionEventsRef.current = [];
    ingestEvents(matching);
  }, [ingestEvents, installState]);

  const togglePaused = useCallback(() => {
    if (pausedRef.current) {
      pausedRef.current = false;
      setPaused(false);
      const pending = pausedEventsRef.current;
      pausedEventsRef.current = [];
      setEvents((current) => appendTriggerMonitorEvents(current, pending));
      return;
    }
    pausedRef.current = true;
    setPaused(true);
  }, []);

  return {
    clear,
    events,
    initializationError,
    monitorState,
    omittedEventCount,
    paused,
    pausedEventCount: pausedEventsRef.current.length,
    pausedOmittedEventCount,
    receivedEventCount,
    start,
    stop,
    togglePaused,
  };
}

export type TriggerMonitorController = ReturnType<typeof useTriggerMonitor>;
