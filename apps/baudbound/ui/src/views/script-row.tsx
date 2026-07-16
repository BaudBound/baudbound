import { ChevronDown, ChevronUp, Play, Power, ShieldCheck, Trash2 } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { ActionMenu } from "@/components/ui/action-menu";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DashboardAction } from "@/lib/app-types";
import {
  removeScript,
  runScript,
  type ScriptStatus,
  setScriptEnabled,
} from "@/lib/runner-api";
import {
  approvalLabel,
  isApprovalCurrent,
  packageHashLabel,
  riskVariant,
} from "@/lib/status-format";
import { cn } from "@/lib/utils";

export function ScriptRow({
  busyActions,
  expanded,
  onToggleDetails,
  onReviewApproval,
  runAction,
  script,
}: {
  busyActions: Set<string>;
  expanded: boolean;
  onToggleDetails: (scriptId: string) => void;
  onReviewApproval: (scriptId: string) => void;
  runAction: DashboardAction;
  script: ScriptStatus;
}) {
  const [confirmRemoveOpen, setConfirmRemoveOpen] = useState(false);
  const reference = script.installed.id;
  const approveAction = `approve:${reference}`;
  const revokeApprovalAction = `revoke-approval:${reference}`;
  const removeAction = `remove:${reference}`;
  const runScriptAction = `run:${reference}`;
  const toggleAction = `toggle:${reference}`;
  const runUnavailableReason = manualRunUnavailableReason(script);
  const canRun = runUnavailableReason === null;
  const runTitle = runUnavailableReason ?? "Run";

  return (
    <tr
      className={cn(
        "border-b border-border align-top last:border-b-0",
        expanded && "bg-muted/35",
      )}
    >
      <td className="px-3 py-3" data-label="Name">
        <div className="flex items-start gap-2">
          <Button
            aria-label={`${expanded ? "Hide" : "Show"} details for ${script.installed.name}`}
            className="mt-[-3px] size-7 p-0"
            onClick={() => onToggleDetails(reference)}
            size="sm"
            title={expanded ? "Hide details" : "Show details"}
            variant={expanded ? "secondary" : "outline"}
          >
            {expanded ? <ChevronUp /> : <ChevronDown />}
          </Button>
          <div className="min-w-0">
            <div className="font-medium">{script.installed.name}</div>
            <div className="mt-0.5 text-xs text-muted-foreground">{reference}</div>
          </div>
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
      <td className="hidden px-3 py-3 xl:table-cell" data-label="Target">
        {script.installed.target_runtime}
      </td>
      <td className="px-3 py-3" data-label="Actions">
        <div className="grid w-fit grid-cols-3 gap-1.5">
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
          <ActionMenu
            items={[
              {
                disabled: busyActions.has(toggleAction),
                icon: Power,
                id: "toggle",
                label: script.installed.enabled ? "Disable" : "Enable",
                onSelect: () =>
                  runAction(toggleAction, () =>
                    setScriptEnabled(reference, !script.installed.enabled),
                  ),
              },
              {
                destructive: true,
                disabled: busyActions.has(removeAction),
                icon: Trash2,
                id: "remove",
                label: "Remove",
                onSelect: () => setConfirmRemoveOpen(true),
              },
            ]}
            label={`More actions for ${script.installed.name}`}
          />
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

function manualRunUnavailableReason(script: ScriptStatus) {
  if (!script.installed.enabled) return "Enable this script before running it";
  if (!isApprovalCurrent(script.approval_status)) {
    return "Approve this script before running it";
  }
  if (!script.triggers.some((trigger) => trigger.action_type === "trigger.manual")) {
    return "This script has no Manual trigger";
  }
  return null;
}
