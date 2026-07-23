import type { TriggerMonitorEvent } from "@/lib/runner-api";

export const triggerMonitorEventLimit = 500;

export function appendTriggerMonitorEvents(
  current: TriggerMonitorEvent[],
  incoming: TriggerMonitorEvent[],
) {
  if (incoming.length === 0) return current;
  const merged = [...current, ...incoming].sort(
    (left, right) => left.sequence - right.sequence,
  );
  return merged.slice(-triggerMonitorEventLimit);
}

export function triggerMonitorEventMatches(
  event: TriggerMonitorEvent,
  search: string,
  actionType: string,
  status: string,
  scriptName: string,
) {
  if (actionType !== "all" && event.action_type !== actionType) return false;
  if (status !== "all" && event.status !== status) return false;
  const query = search.trim().toLowerCase();
  if (!query) return true;
  return [
    event.action_type,
    event.error ?? "",
    event.node_id,
    event.payload_json,
    event.script_id,
    scriptName,
    event.source,
    event.status,
  ]
    .join("\n")
    .toLowerCase()
    .includes(query);
}
