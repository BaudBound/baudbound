import { ShieldCheck } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { DashboardAction } from "@/lib/app-types";
import { approveScript, type ScriptStatus } from "@/lib/runner-api";
import { approvalLabel, packageHashLabel, riskVariant } from "@/lib/status-format";

export function ScriptApprovalDialog({
  busy,
  onOpenChange,
  open,
  runAction,
  script,
}: {
  busy: boolean;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  runAction: DashboardAction;
  script: ScriptStatus;
}) {
  const reference = script.installed.id;
  const actionId = `approve:${reference}`;
  const hashLabel = packageHashLabel(script.package_hash_status);
  const packageIsApprovable = !script.package_error && hashLabel === "valid";
  const approveBlockedReason = approvalBlockReason(script, hashLabel);

  async function approve() {
    if (!packageIsApprovable) return;
    const approved = await runAction(actionId, () => approveScript(reference));
    if (approved) onOpenChange(false);
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(calc(100vw-2rem),620px)]">
        <DialogHeader>
          <DialogTitle>Approve script?</DialogTitle>
          <DialogDescription>
            Approval allows this installed package hash and declared permission set to run on this runner.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-4">
          <section className="rounded-md border border-border bg-background p-3">
            <div className="font-medium">{script.installed.name}</div>
            <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
              {script.installed.id}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge variant={riskVariant(script.installed.risk_level)}>
                {script.installed.risk_level} risk
              </Badge>
              <Badge variant={hashLabel === "valid" ? "good" : "destructive"}>
                hash {hashLabel}
              </Badge>
              <Badge variant={approvalLabel(script.approval_status) === "current" ? "good" : "medium"}>
                approval {approvalLabel(script.approval_status)}
              </Badge>
              <Badge variant="muted">{script.installed.target_runtime}</Badge>
            </div>
          </section>

          <section>
            <h3 className="mb-2 text-sm font-medium">Declared permissions</h3>
            {script.declared_permissions.length === 0 ? (
              <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground">
                This script declares no permissions.
              </div>
            ) : (
              <div className="flex flex-wrap gap-1.5">
                {script.declared_permissions.map((permission) => (
                  <Badge key={permission} variant="muted">
                    {permission}
                  </Badge>
                ))}
              </div>
            )}
          </section>

          {script.package_error ? (
            <div className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              {script.package_error}
            </div>
          ) : null}
          {approveBlockedReason ? (
            <div className="rounded-md border border-baud-amber/35 bg-baud-amber/10 p-3 text-sm text-baud-amber">
              {approveBlockedReason}
            </div>
          ) : null}
        </div>

        <DialogFooter>
          <Button disabled={busy} onClick={() => onOpenChange(false)} variant="outline">
            Cancel
          </Button>
          <Button disabled={busy || !packageIsApprovable} onClick={approve}>
            <ShieldCheck />
            {busy ? "Approving..." : "Approve"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function approvalBlockReason(script: ScriptStatus, hashLabel: string) {
  if (script.package_error) {
    return "This package cannot be approved because the runner cannot load the installed package. Update or remove the script first.";
  }
  if (hashLabel !== "valid") {
    return "This package cannot be approved while its stored package hash is invalid. Update the installed package before approving it.";
  }
  return null;
}
