import { useEffect, useMemo, useRef, useState } from "react";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { RunLogEntry } from "@/lib/runner-api";
import { countLogsByLevel, filterLogs, logLevels } from "@/lib/run-inspection";
import { useDesktopTime } from "@/lib/time-format";

const allLevels = "all";

export function RunLogPanel({
  emptyMessage = "No log entries were recorded for this run.",
  followOutputControl = false,
  logs,
}: {
  emptyMessage?: string;
  followOutputControl?: boolean;
  logs: RunLogEntry[];
}) {
  const { formatUnixMilliseconds } = useDesktopTime();
  const [followOutput, setFollowOutput] = useState(followOutputControl);
  const [levelFilter, setLevelFilter] = useState(allLevels);
  const [query, setQuery] = useState("");
  const logViewportRef = useRef<HTMLDivElement>(null);
  const levelCounts = useMemo(() => countLogsByLevel(logs), [logs]);
  const levels = useMemo(() => logLevels(logs), [logs]);
  const visibleLogs = useMemo(
    () => filterLogs(logs, { level: levelFilter, query }),
    [levelFilter, logs, query],
  );

  useEffect(() => {
    const viewport = logViewportRef.current;
    if (followOutput && viewport) {
      viewport.scrollTop = viewport.scrollHeight;
    }
  }, [followOutput, visibleLogs]);

  if (logs.length === 0) {
    return (
      <div className="grid gap-3">
        {followOutputControl ? (
          <FollowOutputControl checked={followOutput} onChange={setFollowOutput} />
        ) : null}
        <EmptyState>{emptyMessage}</EmptyState>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      <div className="grid items-center gap-2 lg:grid-cols-[minmax(0,1fr)_180px_auto]">
        <Input
          aria-label="Search run logs"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search log message, node, or level"
          value={query}
        />
        <Select onValueChange={setLevelFilter} value={levelFilter}>
          <SelectTrigger aria-label="Filter logs by level">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={allLevels}>All levels</SelectItem>
            {levels.map((level) => (
              <SelectItem key={level} value={level}>
                {level} ({levelCounts[level]})
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {followOutputControl ? (
          <FollowOutputControl checked={followOutput} onChange={setFollowOutput} />
        ) : null}
      </div>

      <div className="flex flex-wrap gap-2">
        {levels.map((level) => (
          <Badge key={level} variant={logLevelVariant(level)}>
            {level}: {levelCounts[level]}
          </Badge>
        ))}
      </div>

      {visibleLogs.length === 0 ? (
        <EmptyState>No log entries match the current filters.</EmptyState>
      ) : (
        <div
          className="max-h-[380px] overflow-auto rounded-md border border-border p-0 max-[900px]:border-0"
          ref={logViewportRef}
        >
          <table className="responsive-table w-full border-collapse text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                <th className="px-3 py-2">#</th>
                <th className="px-3 py-2">Time</th>
                <th className="px-3 py-2">Level</th>
                <th className="px-3 py-2">Node</th>
                <th className="px-3 py-2">Message</th>
              </tr>
            </thead>
            <tbody>
              {visibleLogs.map((log, index) => (
                <tr
                  className="border-b border-border align-top last:border-b-0"
                  key={`${index}-${log.level}-${log.node_id ?? "run"}-${log.message}`}
                >
                  <td
                    className="px-3 py-2 font-mono text-xs text-muted-foreground"
                    data-label="#"
                  >
                    {index + 1}
                  </td>
                  <td className="whitespace-nowrap px-3 py-2" data-label="Time">
                    {formatUnixMilliseconds(log.timestamp_unix_ms)}
                  </td>
                  <td className="px-3 py-2" data-label="Level">
                    <Badge variant={logLevelVariant(log.level)}>{log.level}</Badge>
                  </td>
                  <td
                    className="px-3 py-2 font-mono text-xs text-muted-foreground"
                    data-label="Node"
                  >
                    {log.node_id ?? "-"}
                  </td>
                  <td className="px-3 py-2" data-label="Message">
                    {log.message}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function FollowOutputControl({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex min-h-9 w-fit cursor-pointer items-center gap-2 whitespace-nowrap text-sm">
      <Switch
        aria-label="Follow live output"
        checked={checked}
        onCheckedChange={onChange}
      />
      Follow output
    </label>
  );
}

function logLevelVariant(level: string) {
  if (level === "error") {
    return "destructive";
  }
  if (level === "warn" || level === "warning") {
    return "medium";
  }
  if (level === "info") {
    return "good";
  }
  return "muted";
}
