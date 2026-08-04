import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import {
  clearSystemLogs,
  getSystemLogSummary,
  markSystemLogsRead,
  recordSystemLog,
  type NewSystemLog,
  type StoredSystemLog,
  type SystemLogSeverity,
  type SystemLogSummary,
} from "@/lib/runner-api";
import {
  createSystemLog,
  type SystemLogOptions,
} from "@/lib/system-log-model";

type OpenLogRequest = {
  id: string;
  sequence: number;
};

type SystemLogContextValue = {
  clear: () => Promise<number>;
  clearOpenRequest: (request: OpenLogRequest) => void;
  log: (log: NewSystemLog) => Promise<StoredSystemLog | null>;
  markAllRead: () => Promise<void>;
  notify: Record<
    SystemLogSeverity,
    (message: string, options: SystemLogOptions) => void
  >;
  openRequest: OpenLogRequest | null;
  revision: number;
  setViewing: (viewing: boolean) => void;
  summary: SystemLogSummary;
};

const emptySummary: SystemLogSummary = {
  total: 0,
  unread: 0,
  unread_errors: 0,
  unread_info: 0,
  unread_successes: 0,
  unread_warnings: 0,
};

const SystemLogContext = createContext<SystemLogContextValue | null>(null);

export function SystemLogProvider({ children }: { children: ReactNode }) {
  const [summary, setSummary] = useState(emptySummary);
  const [revision, setRevision] = useState(0);
  const [openRequest, setOpenRequest] = useState<OpenLogRequest | null>(null);
  const mutationQueue = useRef<Promise<void>>(Promise.resolve());
  const viewing = useRef(false);

  const enqueueMutation = useCallback(<T,>(operation: () => Promise<T>) => {
    const result = mutationQueue.current.then(operation, operation);
    mutationQueue.current = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }, []);

  const refreshSummary = useCallback(async () => {
    try {
      setSummary(await enqueueMutation(getSystemLogSummary));
    } catch (error) {
      console.error("Could not load the system log summary.", error);
    }
  }, [enqueueMutation]);

  useEffect(() => {
    void refreshSummary();
  }, [refreshSummary]);

  const persist = useCallback(
    async (log: NewSystemLog) => {
      try {
        return await enqueueMutation(async () => {
          const stored = await recordSystemLog(log);
          if (viewing.current) {
            await markSystemLogsRead();
          }
          setSummary(await getSystemLogSummary());
          setRevision((current) => current + 1);
          return stored;
        });
      } catch (error) {
        console.error("Could not persist a runner system log.", error);
        return null;
      }
    },
    [enqueueMutation],
  );

  const showNotification = useCallback(
    (
      severity: SystemLogSeverity,
      message: string,
      options: SystemLogOptions,
    ) => {
      const log = createSystemLog(severity, message, options);
      const stored = persist(log);
      const openDetails = () => {
        void stored.then((record) => {
          if (!record) return;
          setOpenRequest((current) => ({
            id: record.id,
            sequence: (current?.sequence ?? 0) + 1,
          }));
        });
      };
      const toastOptions = {
        action: { label: "Details", onClick: openDetails },
        description: log.message,
      };
      if (severity === "error") {
        toast.error(log.title, toastOptions);
      } else if (severity === "warning") {
        toast.warning(log.title, toastOptions);
      } else if (severity === "success") {
        toast.success(log.title, toastOptions);
      } else {
        toast.info(log.title, toastOptions);
      }
    },
    [persist],
  );

  const notify = useMemo(
    () => ({
      error: (message: string, options: SystemLogOptions) =>
        showNotification("error", message, options),
      info: (message: string, options: SystemLogOptions) =>
        showNotification("info", message, options),
      success: (message: string, options: SystemLogOptions) =>
        showNotification("success", message, options),
      warning: (message: string, options: SystemLogOptions) =>
        showNotification("warning", message, options),
    }),
    [showNotification],
  );

  const markAllRead = useCallback(async () => {
    try {
      await enqueueMutation(async () => {
        setSummary(await markSystemLogsRead());
        setRevision((current) => current + 1);
      });
    } catch (error) {
      console.error("Could not mark runner system logs as read.", error);
    }
  }, [enqueueMutation]);

  const clear = useCallback(async () => {
    return enqueueMutation(async () => {
      const removed = await clearSystemLogs();
      setSummary(await getSystemLogSummary());
      setRevision((current) => current + 1);
      return removed;
    });
  }, [enqueueMutation]);

  const setViewing = useCallback((next: boolean) => {
    viewing.current = next;
  }, []);

  const clearOpenRequest = useCallback((request: OpenLogRequest) => {
    setOpenRequest((current) =>
      current?.id === request.id && current.sequence === request.sequence
        ? null
        : current,
    );
  }, []);

  const value = useMemo<SystemLogContextValue>(
    () => ({
      clear,
      clearOpenRequest,
      log: persist,
      markAllRead,
      notify,
      openRequest,
      revision,
      setViewing,
      summary,
    }),
    [
      clear,
      clearOpenRequest,
      markAllRead,
      notify,
      openRequest,
      persist,
      revision,
      setViewing,
      summary,
    ],
  );

  return (
    <SystemLogContext.Provider value={value}>
      {children}
    </SystemLogContext.Provider>
  );
}

export function useSystemLog() {
  const context = useContext(SystemLogContext);
  if (!context) {
    throw new Error("useSystemLog must be used inside SystemLogProvider");
  }
  return context;
}
