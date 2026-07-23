import { ShieldCheck, ShieldOff } from "lucide-react";

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
import { approvalReviewState } from "@/lib/approval-review";
import {
  approveScript,
  revokeScriptApproval,
  type ScriptStatus,
} from "@/lib/runner-api";
import {
  approvalLabel,
  approvalVariant,
  isPackageHashValid,
  packageHashLabel,
  riskVariant,
} from "@/lib/status-format";

export function ScriptApprovalDialog({
  approveBusy,
  onOpenChange,
  open,
  runAction,
  script,
  revokeBusy,
}: {
  approveBusy: boolean;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  runAction: DashboardAction;
  script: ScriptStatus;
  revokeBusy: boolean;
}) {
  const reference = script.installed.id;
  const actionId = `approve:${reference}`;
  const revokeActionId = `revoke-approval:${reference}`;
  const hashLabel = packageHashLabel(script.package_hash_status);
  const {
    approvalIsCurrent,
    approvalIsStored,
    approveBlockedReason,
    packageIsApprovable,
  } = approvalReviewState(script);
  const busy = approveBusy || revokeBusy;

  async function approve() {
    if (!packageIsApprovable) return;
    const approved = await runAction(actionId, () => approveScript(reference));
    if (approved) onOpenChange(false);
  }

  async function revoke() {
    const revoked = await runAction(revokeActionId, () => revokeScriptApproval(reference));
    if (revoked) onOpenChange(false);
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(calc(100vw-2rem),620px)]">
        <DialogHeader>
          <DialogTitle>Review script approval</DialogTitle>
          <DialogDescription>
            Approving this script allows the installed package with this hash and permission set to
            run on this runner.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-4">
          <section className="rounded-md border border-border bg-background p-3">
            <div className="font-medium">{script.installed.name}</div>
            <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
              {script.installed.id}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <ReviewBadge
                label="Risk"
                value={titleCase(script.installed.risk_level)}
                variant={riskVariant(script.installed.risk_level)}
              />
              <ReviewBadge
                label="Integrity"
                value={hashLabel}
                variant={isPackageHashValid(script.package_hash_status) ? "good" : "destructive"}
              />
              <ReviewBadge
                label="Approval"
                value={approvalLabel(script.approval_status)}
                variant={approvalVariant(script.approval_status)}
              />
              <ReviewBadge
                label="Target"
                value={script.installed.target_runtime}
                variant="muted"
              />
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
            Close
          </Button>
          {approvalIsStored ? (
            <Button disabled={busy} onClick={revoke} variant="destructive">
              <ShieldOff />
              {revokeBusy ? "Revoking..." : "Revoke approval"}
            </Button>
          ) : null}
          {!approvalIsCurrent ? (
            <Button disabled={busy || !packageIsApprovable} onClick={approve}>
              <ShieldCheck />
              {approveBusy ? "Approving..." : "Approve package"}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReviewBadge({
  label,
  value,
  variant,
}: {
  label: string;
  value: string;
  variant: "default" | "destructive" | "good" | "medium" | "muted" | "red";
}) {
  return (
    <Badge className="h-6 gap-1.5 px-2.5" variant={variant}>
      <span className="opacity-70">{label}</span>
      <span aria-hidden="true" className="h-3 w-px bg-current opacity-30" />
      <span>{value}</span>
    </Badge>
  );
}

function titleCase(value: string) {
  return value.replace(/\b\w/g, (letter) => letter.toUpperCase());
}
