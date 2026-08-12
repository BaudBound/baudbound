import { Copy, RefreshCw, ShieldCheck, ShieldOff } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { useSystemLog } from "@/components/system-log-provider";
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
import {
  type DashboardPayload,
  rotateNetworkTriggerToken,
  setNetworkTriggerAuthEnabled,
  type TriggerAuthStatus,
} from "@/lib/runner-api";

type TriggerAuthConfirmation = "disable" | "rotate" | null;

export function NetworkTriggerAuthStatus({ auth }: { auth: TriggerAuthStatus }) {
  return (
    <div className="grid gap-1">
      <Badge variant={auth.auth_enabled ? "good" : "destructive"}>
        {auth.auth_enabled ? "Protected" : "Unprotected"}
      </Badge>
      {auth.auth_enabled ? (
        <span className="font-mono text-xs text-muted-foreground">ends in {auth.token_preview}</span>
      ) : null}
    </div>
  );
}

export function NetworkTriggerAuthActions({
  auth,
  busyActions,
  onDashboard,
  runAction,
}: {
  auth: TriggerAuthStatus;
  busyActions: Set<string>;
  onDashboard: (dashboard: DashboardPayload) => void;
  runAction: DashboardAction;
}) {
  const { notify } = useSystemLog();
  const [confirmation, setConfirmation] = useState<TriggerAuthConfirmation>(null);
  const [generatedToken, setGeneratedToken] = useState<string | null>(null);
  const [rotating, setRotating] = useState(false);
  const actionId = `trigger-auth:${auth.script_id}:${auth.node_id}`;
  const busy = rotating || busyActions.has(actionId);

  async function rotateToken() {
    if (busy) return;
    setRotating(true);
    try {
      const result = await rotateNetworkTriggerToken(
        auth.script_id,
        auth.node_id,
        auth.trigger_type,
      );
      onDashboard(result.dashboard);
      setGeneratedToken(result.token);
      notify.success(result.message, {
        details: [
          { label: "Script ID", value: auth.script_id },
          { label: "Trigger node ID", value: auth.node_id },
          { label: "Trigger type", value: auth.trigger_type },
        ],
        source: "Network trigger security",
        title: "Trigger token rotated",
      });
    } catch (error) {
      notify.error("The network trigger token could not be rotated.", {
        details: [
          { label: "Script ID", value: auth.script_id },
          { label: "Trigger node ID", value: auth.node_id },
          { label: "Trigger type", value: auth.trigger_type },
        ],
        error,
        source: "Network trigger security",
        title: "Token rotation failed",
      });
    } finally {
      setRotating(false);
    }
  }

  async function setEnabled(enabled: boolean) {
    await runAction(actionId, () =>
      setNetworkTriggerAuthEnabled(
        auth.script_id,
        auth.node_id,
        auth.trigger_type,
        enabled,
      ),
    );
  }

  return (
    <>
      <div className="flex items-center gap-1.5">
        <Button
          aria-label="Generate a new token"
          className="size-7 p-0"
          disabled={busy}
          onClick={() => setConfirmation("rotate")}
          size="sm"
          title="Generate a new token"
          variant="outline"
        >
          <RefreshCw />
        </Button>
        <Button
          aria-label={auth.auth_enabled ? "Disable authentication" : "Enable authentication"}
          className="size-7 p-0"
          disabled={busy}
          onClick={() =>
            auth.auth_enabled ? setConfirmation("disable") : void setEnabled(true)
          }
          size="sm"
          title={auth.auth_enabled ? "Disable authentication" : "Enable authentication"}
          variant={auth.auth_enabled ? "outline" : "default"}
        >
          {auth.auth_enabled ? <ShieldOff /> : <ShieldCheck />}
        </Button>
      </div>

      <ConfirmDialog
        confirmLabel="Generate token"
        description="The current token will stop working immediately. Any integration using it must be updated. The new token is shown only once."
        disabled={busy}
        onConfirm={rotateToken}
        onOpenChange={(open) => setConfirmation(open ? "rotate" : null)}
        open={confirmation === "rotate"}
        title="Generate a new trigger token?"
      />
      <ConfirmDialog
        confirmLabel="Disable authentication"
        description="Anyone who can reach this listener will be able to trigger the script without a token. Public listeners are blocked unless the unsafe public bind override is enabled."
        destructive
        disabled={busy}
        onConfirm={() => setEnabled(false)}
        onOpenChange={(open) => setConfirmation(open ? "disable" : null)}
        open={confirmation === "disable"}
        title="Disable trigger authentication?"
      />
      <Dialog onOpenChange={(open) => !open && setGeneratedToken(null)} open={generatedToken !== null}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Save this token now</DialogTitle>
            <DialogDescription>
              This token cannot be shown again. If it is lost, generate a new one and update the
              integration.
            </DialogDescription>
          </DialogHeader>
          <code className="select-text break-all rounded-md border border-border bg-background p-3 font-mono text-sm">
            {generatedToken}
          </code>
          <DialogFooter>
            <Button
              disabled={!generatedToken}
              onClick={() => {
                if (!generatedToken) return;
                void navigator.clipboard.writeText(generatedToken).then(
                  () =>
                    notify.success("The trigger token was copied to the clipboard.", {
                      source: "Network trigger security",
                      title: "Token copied",
                    }),
                  (error) =>
                    notify.error("The trigger token could not be copied.", {
                      error,
                      source: "Network trigger security",
                      title: "Could not copy token",
                    }),
                );
              }}
              variant="outline"
            >
              <Copy />
              Copy token
            </Button>
            <Button onClick={() => setGeneratedToken(null)}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
