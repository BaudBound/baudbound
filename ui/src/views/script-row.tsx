import { Eye, Play, Power, ShieldCheck, Square, Trash2 } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DashboardAction } from "@/lib/app-types";
import {
  removeScript,
  runScript,
  type ActiveRun,
  type ScriptStatus,
  type ScriptUpdateState,
  setScriptEnabled,
  stopScriptRuns,
} from "@/lib/runner-api";
import {
  approvalLabel,
  isApprovalCurrent,
  packageHashLabel,
  riskVariant,
} from "@/lib/status-format";
import { scriptRunControl } from "@/lib/script-run-control";

export function ScriptRow({
  activeRuns,
  busyActions,
  onReviewApproval,
  onViewDetails,
  runAction,
  script,
  updateState,
}: {
  activeRuns: ActiveRun[];
  busyActions: Set<string>;
  onReviewApproval: (scriptId: string) => void;
  onViewDetails: (scriptId: string) => void;
  runAction: DashboardAction;
  script: ScriptStatus;
  updateState: ScriptUpdateState;
}) {
  const [confirmRemoveOpen, setConfirmRemoveOpen] = useState(false);
  const reference = script.installed.id;
  const approveAction = `approve:${reference}`;
  const revokeApprovalAction = `revoke-approval:${reference}`;
  const removeAction = `remove:${reference}`;
  const runScriptAction = `run:${reference}`;
  const stopScriptAction = `stop-script:${reference}`;
  const toggleAction = `toggle:${reference}`;
  const runUnavailableReason = manualRunUnavailableReason(script);
  const canRun = runUnavailableReason === null;
  const runTitle = runUnavailableReason ?? "Run";
  const runControl = scriptRunControl(script, activeRuns);
  const isStopping =
    activeRuns.length > 0 && activeRuns.every((run) => run.cancellation_requested);

  return (
    <tr className="border-b border-border align-top last:border-b-0">
      <td className="px-3 py-3" data-label="Name">
        <div className="min-w-0">
          <div className="font-medium">{script.installed.name}</div>
          <div className="mt-0.5 text-xs text-muted-foreground">{reference}</div>
        </div>
        {script.package_error ? (
          <div className="mt-1 max-w-[360px] text-xs text-destructive">
            {script.package_error}
          </div>
        ) : null}
      </td>
      <td className="px-3 py-3" data-label="State">
        {script.installed.enabled ? "enabled" : "disabled"}
      </td>
      <td className="px-3 py-3" data-label="Risk">
        <Badge variant={riskVariant(script.installed.risk_level)}>
          {script.installed.risk_level}
        </Badge>
      </td>
      <td className="hidden px-3 py-3 xl:table-cell" data-label="Hash">
        {packageHashLabel(script.package_hash_status)}
      </td>
      <td className="px-3 py-3" data-label="Approval">
        {approvalLabel(script.approval_status)}
      </td>
      <td className="px-3 py-3" data-label="Triggers">
        {script.triggers.length}
      </td>
      <td className="hidden px-3 py-3 xl:table-cell" data-label="Target runtimes">
        {script.installed.target_runtime}
      </td>
      <td className="px-3 py-3" data-label="Update">
        <Badge variant={updateStatusVariant(updateState.status)}>
          {updateStatusLabel(updateState.status)}
        </Badge>
      </td>
      <td className="px-3 py-3" data-label="Actions">
        <div className="ml-auto flex w-[11.5rem] justify-between max-[1280px]:ml-0">
          {runControl === "stop" ? (
            <Button
              aria-label={`Stop ${script.installed.name}`}
              className="size-8 p-0"
              disabled={isStopping || busyActions.has(stopScriptAction)}
              onClick={() =>
                runAction(stopScriptAction, () => stopScriptRuns(reference))
              }
              size="sm"
              title={
                isStopping
                  ? "Stop requested"
                    : `Stop ${activeRuns.length} active run${activeRuns.length === 1 ? "" : "s"}`
              }
              variant="destructive"
            >
              <Square />
            </Button>
          ) : (
            <span title={runTitle}>
              <Button
                aria-label={`Run ${script.installed.name}`}
                className="size-8 p-0"
                disabled={!canRun || busyActions.has(runScriptAction)}
                onClick={() => runAction(runScriptAction, () => runScript(reference))}
                size="sm"
                title={runTitle}
                variant="default"
              >
                <Play />
              </Button>
            </span>
          )}
          <Button
            aria-label={`View details for ${script.installed.name}`}
            className="size-8 p-0"
            onClick={() => onViewDetails(reference)}
            size="sm"
            title="View details"
            variant="outline"
          >
            <Eye />
          </Button>
          <Button
            aria-label={`Review approval for ${script.installed.name}`}
            className="size-8 p-0"
            disabled={busyActions.has(approveAction) || busyActions.has(revokeApprovalAction)}
            onClick={() => onReviewApproval(reference)}
            size="sm"
            title="Review approval"
            variant="outline"
          >
            <ShieldCheck />
          </Button>
          <Button
            aria-label={`${script.installed.enabled ? "Disable" : "Enable"} ${script.installed.name}`}
            className="size-8 p-0"
            disabled={busyActions.has(toggleAction)}
            onClick={() =>
              runAction(toggleAction, () =>
                setScriptEnabled(reference, !script.installed.enabled),
              )
            }
            size="sm"
            title={script.installed.enabled ? "Disable" : "Enable"}
            variant="outline"
          >
            <Power />
          </Button>
          <Button
            aria-label={`Remove ${script.installed.name}`}
            className="size-8 p-0"
            disabled={busyActions.has(removeAction)}
            onClick={() => setConfirmRemoveOpen(true)}
            size="sm"
            title="Remove"
            variant="destructive"
          >
            <Trash2 />
          </Button>
          <ConfirmDialog
            confirmLabel="Remove"
            description={`Remove ${script.installed.name} from this runner. The installed package copy and approval record will be deleted from local runner storage.`}
            destructive
            disabled={busyActions.has(removeAction)}
            onConfirm={async () => {
              await runAction(removeAction, () => removeScript(reference));
            }}
            onOpenChange={setConfirmRemoveOpen}
            open={confirmRemoveOpen}
            title="Remove script?"
          />
        </div>
      </td>
    </tr>
  );
}

function updateStatusLabel(status: ScriptUpdateState["status"]) {
  switch (status) {
    case "available": return "Available";
    case "failed": return "Failed";
    case "not_checked": return "Not checked";
    case "unavailable": return "Unavailable";
    case "unconfigured": return "Not configured";
    case "up_to_date": return "Up to date";
  }
}

function updateStatusVariant(status: ScriptUpdateState["status"]) {
  if (status === "available") return "medium" as const;
  if (status === "failed" || status === "unavailable") return "destructive" as const;
  if (status === "up_to_date") return "good" as const;
  return "muted" as const;
}

function manualRunUnavailableReason(script: ScriptStatus) {
  if (!script.installed.enabled) return "Enable this script before running it";
  if (!isApprovalCurrent(script.approval_status)) {
    return "Approve this script before running it";
  }
  if (!script.triggers.some((trigger) => trigger.action_type === "trigger.manual")) {
    return "This script does not have a Manual trigger";
  }
  return null;
}
