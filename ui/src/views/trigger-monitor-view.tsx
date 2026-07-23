import {
  Eye,
  Pause,
  Play,
  RadioTower,
  RotateCcw,
  Square,
  Trash2,
} from "lucide-react";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { CodeBlock } from "@/components/code-block";
import { DetailDialog } from "@/components/detail-dialog";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { TriggerMonitorController } from "@/hooks/use-trigger-monitor";
import type {
  DashboardPayload,
  TriggerMonitorEvent,
} from "@/lib/runner-api";
import { formatCount } from "@/lib/count-format";
import { SEARCH_INPUT_MAX_LENGTH } from "@/lib/input-limits";
import { triggerMonitorEventMatches } from "@/lib/trigger-monitor-events";
import { useDesktopTime } from "@/lib/time-format";
import { visibleText } from "@/lib/visible-text";

export function TriggerMonitorView({
  controller,
  dashboard,
}: {
  controller: TriggerMonitorController;
  dashboard: DashboardPayload;
}) {
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [actionType, setActionType] = useState("all");
  const [status, setStatus] = useState("all");
  const [follow, setFollow] = useState(true);
  const [selected, setSelected] = useState<TriggerMonitorEvent | null>(null);
  const [busy, setBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const scriptNames = useMemo(
    () =>
      Object.fromEntries(
        dashboard.runner.scripts.map((script) => [
          script.installed.id,
          script.installed.name,
        ]),
      ),
    [dashboard.runner.scripts],
  );
  const actionTypes = useMemo(
    () =>
      [...new Set(controller.events.map((event) => event.action_type))].sort(),
    [controller.events],
  );
  const filteredEvents = useMemo(
    () =>
      controller.events.filter((event) =>
        triggerMonitorEventMatches(
          event,
          deferredSearch,
          actionType,
          status,
          scriptNames[event.script_id] ?? "",
        ),
      ),
    [
      actionType,
      controller.events,
      deferredSearch,
      scriptNames,
      status,
    ],
  );

  useEffect(() => {
    if (!follow || controller.paused) return;
    const container = scrollRef.current;
    if (container) container.scrollTop = container.scrollHeight;
  }, [controller.events, controller.paused, filteredEvents, follow]);

  async function perform(action: () => Promise<void>, failure: string) {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      toast.error(`${failure}: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="flex items-center gap-2">
              <RadioTower className="size-4 text-baud-blue" />
              Live trigger monitor
            </CardTitle>
            <p className="mt-1 text-xs text-muted-foreground">
              Captured input remains in memory and is cleared when BaudBound exits.
            </p>
          </div>
          <Badge variant={controller.monitorState.enabled ? "good" : "muted"}>
            {controller.monitorState.enabled ? "Monitoring" : "Stopped"}
          </Badge>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          {controller.monitorState.enabled ? (
            <Button
              disabled={busy}
              onClick={() =>
                void perform(controller.stop, "Could not stop trigger monitoring")
              }
              size="sm"
              variant="outline"
            >
              <Square />
              Stop monitoring
            </Button>
          ) : (
            <Button
              disabled={busy}
              onClick={() =>
                void perform(controller.start, "Could not start trigger monitoring")
              }
              size="sm"
            >
              <Play />
              Start monitoring
            </Button>
          )}
          <Button
            disabled={!controller.monitorState.enabled || busy}
            onClick={controller.togglePaused}
            size="sm"
            variant="outline"
          >
            {controller.paused ? <RotateCcw /> : <Pause />}
            {controller.paused ? "Resume view" : "Pause view"}
          </Button>
          <Button
            disabled={controller.events.length === 0 || busy}
            onClick={() =>
              void perform(controller.clear, "Could not clear trigger events")
            }
            size="sm"
            variant="outline"
          >
            <Trash2 />
            Clear
          </Button>
          <label className="ml-auto flex items-center gap-2 text-sm">
            <Checkbox
              checked={follow}
              onCheckedChange={(checked) => setFollow(checked === true)}
            />
            Follow latest
          </label>
        </CardContent>
      </Card>

      {controller.initializationError ? (
        <EmptyState>
          Could not initialize trigger monitoring: {controller.initializationError}
        </EmptyState>
      ) : null}
      {controller.omittedEventCount > 0 ? (
        <div className="rounded-md border border-baud-amber/30 bg-baud-amber/10 px-4 py-3 text-sm text-baud-amber">
          {formatCount(controller.omittedEventCount, "monitor event")}{" "}
          {controller.omittedEventCount === 1 ? "was" : "were"} omitted because the
          interface could not keep up. Script execution was not affected.
        </div>
      ) : null}
      {controller.pausedOmittedEventCount > 0 ? (
        <div className="rounded-md border border-baud-amber/30 bg-baud-amber/10 px-4 py-3 text-sm text-baud-amber">
          {formatCount(controller.pausedOmittedEventCount, "paused event")}{" "}
          {controller.pausedOmittedEventCount === 1 ? "was" : "were"} omitted after
          the 500 event pause buffer filled. Script execution was not affected.
        </div>
      ) : null}

      <Card>
        <CardHeader className="grid gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <CardTitle>Trigger events</CardTitle>
            <div className="text-xs text-muted-foreground">
              {controller.paused
                ? `${controller.pausedEventCount} waiting while paused`
                : `${controller.receivedEventCount} received this session`}
            </div>
          </div>
          <div className="grid min-w-0 grid-cols-[minmax(12rem,1fr)_minmax(12rem,18rem)_minmax(10rem,14rem)] gap-2 max-md:grid-cols-1">
            <Input
              aria-label="Search trigger events"
              maxLength={SEARCH_INPUT_MAX_LENGTH}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search script, trigger, type, source, or payload"
              value={search}
            />
            <Select onValueChange={setActionType} value={actionType}>
              <SelectTrigger aria-label="Filter trigger type">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All trigger types</SelectItem>
                {actionTypes.map((type) => (
                  <SelectItem key={type} value={type}>
                    {type}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select onValueChange={setStatus} value={status}>
              <SelectTrigger aria-label="Filter trigger status">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All statuses</SelectItem>
                <SelectItem value="queued">Queued</SelectItem>
                <SelectItem value="rejected">Rejected</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent className="p-0 max-[1280px]:p-3">
          {filteredEvents.length === 0 ? (
            <div className="p-4">
              <EmptyState>
                {controller.monitorState.enabled
                  ? "Waiting for a registered trigger to receive input."
                  : "Start monitoring to inspect incoming trigger data."}
              </EmptyState>
            </div>
          ) : (
            <TriggerEventTable
              events={filteredEvents}
              onView={setSelected}
              scriptNames={scriptNames}
              scrollRef={scrollRef}
            />
          )}
        </CardContent>
      </Card>

      <TriggerEventDialog
        event={selected}
        onOpenChange={(open) => {
          if (!open) setSelected(null);
        }}
        scriptName={selected ? scriptNames[selected.script_id] : undefined}
      />
    </div>
  );
}

function TriggerEventTable({
  events,
  onView,
  scriptNames,
  scrollRef,
}: {
  events: TriggerMonitorEvent[];
  onView: (event: TriggerMonitorEvent) => void;
  scriptNames: Record<string, string>;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}) {
  const { formatUnixMilliseconds } = useDesktopTime();
  return (
    <div className="max-h-[60vh] overflow-auto" ref={scrollRef}>
      <table className="responsive-table w-full border-collapse text-sm">
        <thead className="sticky top-0 z-10 bg-card">
          <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
            <th className="px-3 py-2">Time</th>
            <th className="px-3 py-2">Script</th>
            <th className="px-3 py-2">Trigger</th>
            <th className="px-3 py-2">Type</th>
            <th className="px-3 py-2">Status</th>
            <th className="px-3 py-2">Payload</th>
            <th className="w-12 px-3 py-2"><span className="sr-only">View</span></th>
          </tr>
        </thead>
        <tbody>
          {events.map((event) => (
            <tr
              className="border-b border-border align-top last:border-0"
              key={`${event.session_id}-${event.sequence}`}
            >
              <td className="whitespace-nowrap px-3 py-3" data-label="Time">
                {formatUnixMilliseconds(event.timestamp_unix_ms)}
              </td>
              <td className="px-3 py-3" data-label="Script">
                <div className="font-medium">
                  {scriptNames[event.script_id] ?? "Unknown script"}
                </div>
                <div className="break-all font-mono text-xs text-muted-foreground">
                  {event.script_id}
                </div>
              </td>
              <td className="px-3 py-3 font-mono text-xs" data-label="Trigger">
                {event.node_id}
              </td>
              <td
                className="px-3 py-3 font-mono text-xs text-muted-foreground"
                data-label="Type"
              >
                {event.action_type}
              </td>
              <td className="px-3 py-3" data-label="Status">
                <Badge variant={event.status === "queued" ? "good" : "destructive"}>
                  {event.status}
                </Badge>
              </td>
              <td className="max-w-[28rem] px-3 py-3" data-label="Payload">
                <span className="line-clamp-2 break-all font-mono text-xs">
                  {visibleText(event.payload_json)}
                </span>
              </td>
              <td className="px-3 py-3" data-label="View">
                <Button
                  aria-label={`View trigger event ${event.sequence}`}
                  className="w-7 px-0"
                  onClick={() => onView(event)}
                  size="sm"
                  title="View trigger event"
                  variant="outline"
                >
                  <Eye />
                </Button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function TriggerEventDialog({
  event,
  onOpenChange,
  scriptName,
}: {
  event: TriggerMonitorEvent | null;
  onOpenChange: (open: boolean) => void;
  scriptName?: string;
}) {
  if (!event) return null;
  return (
    <DetailDialog
      description={`${event.action_type} from ${scriptName ?? event.script_id}`}
      onOpenChange={onOpenChange}
      open
      title="Trigger event"
    >
      <div className="grid gap-4">
        <Card>
          <CardHeader><CardTitle>Event</CardTitle></CardHeader>
          <CardContent className="grid grid-cols-2 gap-4 text-sm max-md:grid-cols-1">
            <Detail label="Script" value={scriptName ?? "Unknown script"} />
            <Detail label="Script ID" value={event.script_id} mono />
            <Detail label="Trigger" value={event.node_id} mono />
            <Detail label="Type" value={event.action_type} mono />
            <Detail label="Source" value={event.source} />
            <Detail label="Status" value={event.status} />
            <Detail label="Payload size" value={formatBytes(event.payload_bytes)} />
            <Detail label="Sequence" value={String(event.sequence)} />
          </CardContent>
        </Card>
        {event.error ? (
          <Card>
            <CardHeader><CardTitle>Rejection</CardTitle></CardHeader>
            <CardContent className="text-sm text-destructive">{event.error}</CardContent>
          </Card>
        ) : null}
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-3">
            <CardTitle>Payload</CardTitle>
            {event.payload_truncated ? (
              <Badge variant="medium">Truncated at 64 KiB</Badge>
            ) : null}
          </CardHeader>
          <CardContent>
            <CodeBlock className="max-h-[50vh]">{payloadText(event)}</CodeBlock>
          </CardContent>
        </Card>
      </div>
    </DetailDialog>
  );
}

function Detail({
  label,
  mono = false,
  value,
}: {
  label: string;
  mono?: boolean;
  value: string;
}) {
  return (
    <div className="min-w-0">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={`mt-1 break-all ${mono ? "font-mono text-xs" : ""}`}>
        {value}
      </div>
    </div>
  );
}

function payloadText(event: TriggerMonitorEvent) {
  if (event.payload_truncated) {
    return `${visibleText(event.payload_json)}\n\n[Payload truncated. Original size: ${formatBytes(event.payload_bytes)}]`;
  }
  try {
    return visibleText(JSON.stringify(JSON.parse(event.payload_json), null, 2));
  } catch {
    return visibleText(event.payload_json);
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
