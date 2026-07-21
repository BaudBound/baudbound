import {
  KeyRound,
  LoaderCircle,
  LockKeyhole,
  RefreshCw,
  TriangleAlert,
  Trash2,
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { DashboardAction } from "@/lib/app-types";
import {
  type DashboardPayload,
  type InstalledSecretStatus,
  removeScriptSecret,
  retrySecretVault,
  setScriptSecret,
} from "@/lib/runner-api";

type SecretSelection = {
  scriptId: string;
  scriptName: string;
  secret: InstalledSecretStatus;
};

export function SecretManagementPanel({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [selection, setSelection] = useState<SecretSelection | null>(null);
  const [value, setValue] = useState("");
  const scriptsWithSecrets = dashboard.runner.scripts.filter(
    (script) => (dashboard.secret_statuses[script.installed.id] ?? []).length > 0,
  );
  const secretStorageAvailable = dashboard.secret_vault.status === "available";
  const vaultRetryAction = "secret-vault-retry";

  const close = () => {
    setSelection(null);
    setValue("");
  };
  const save = async () => {
    if (!selection || value === "") return;
    const actionId = `secret-set:${selection.scriptId}:${selection.secret.name}`;
    if (
      await runAction(actionId, () =>
        setScriptSecret(selection.scriptId, selection.secret.name, value),
      )
    ) {
      close();
    }
  };

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="size-4 text-baud-amber" /> Script secrets
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          {dashboard.secret_vault.status === "initializing" ? (
            <div className="flex gap-2 rounded-md border border-border bg-background p-3 text-sm">
              <LoaderCircle className="mt-0.5 size-4 shrink-0 animate-spin" />
              <div>
                <div className="font-medium">Connecting to encrypted secret storage</div>
                <p className="mt-1 text-xs text-muted-foreground">
                  The runner remains available while BaudBound connects to the operating system
                  credential vault. Secret actions become available after the connection succeeds.
                </p>
              </div>
            </div>
          ) : dashboard.secret_vault.status === "unavailable" ? (
            <div className="flex flex-wrap items-start gap-3 rounded-md border border-baud-amber/40 bg-baud-amber/5 p-3 text-sm text-baud-amber">
              <TriangleAlert className="mt-0.5 size-4 shrink-0" />
              <div className="min-w-0 flex-1">
                <div className="font-medium">Encrypted secret storage is unavailable</div>
                <p className="mt-1 text-xs text-muted-foreground">
                  Other runner features remain available, but scripts cannot read or save secrets
                  until the operating system credential vault is available.
                </p>
                {dashboard.secret_vault.error ? (
                  <p className="mt-2 select-text break-words font-mono text-xs text-muted-foreground">
                    {dashboard.secret_vault.error}
                  </p>
                ) : null}
              </div>
              <Button
                disabled={busyActions.has(vaultRetryAction)}
                size="sm"
                variant="outline"
                onClick={() => void runAction(vaultRetryAction, retrySecretVault)}
              >
                <RefreshCw /> Retry
              </Button>
            </div>
          ) : null}
          {scriptsWithSecrets.length === 0 ? (
            <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground">
              Installed scripts do not declare any secret references.
            </div>
          ) : (
            scriptsWithSecrets.map((script) => (
              <section className="rounded-md border border-border bg-background" key={script.installed.id}>
                <div className="border-b border-border px-3 py-2">
                  <div className="font-medium">{script.installed.name}</div>
                  <div className="font-mono text-xs text-muted-foreground">{script.installed.id}</div>
                </div>
                <div className="divide-y divide-border">
                  {(dashboard.secret_statuses[script.installed.id] ?? []).map((secret) => {
                    const setActionId = `secret-set:${script.installed.id}:${secret.name}`;
                    const removeActionId = `secret-remove:${script.installed.id}:${secret.name}`;
                    return (
                      <div
                        className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-3 py-2 max-sm:grid-cols-1"
                        key={secret.name}
                      >
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="break-all font-mono text-sm">{secret.name}</span>
                            <Badge variant={secret.configured ? "good" : secret.required ? "destructive" : "muted"}>
                              {secret.configured ? "Configured" : secret.required ? "Required" : "Optional"}
                            </Badge>
                            <Badge variant="muted">{secret.value_type}</Badge>
                          </div>
                          {secret.description ? (
                            <p className="mt-1 text-xs text-muted-foreground">{secret.description}</p>
                          ) : null}
                        </div>
                        <div className="flex flex-wrap justify-end gap-2 max-sm:justify-start">
                          <Button
                            disabled={
                              !secretStorageAvailable || busyActions.has(setActionId)
                            }
                            size="sm"
                            variant="outline"
                            onClick={() => {
                              setSelection({
                                scriptId: script.installed.id,
                                scriptName: script.installed.name,
                                secret,
                              });
                              setValue("");
                            }}
                          >
                            <LockKeyhole /> {secret.configured ? "Replace" : "Configure"}
                          </Button>
                          {secret.configured ? (
                            <Button
                              disabled={
                                !secretStorageAvailable ||
                                busyActions.has(removeActionId)
                              }
                              size="sm"
                              variant="destructive"
                              onClick={() =>
                                void runAction(removeActionId, () =>
                                  removeScriptSecret(script.installed.id, secret.name),
                                )
                              }
                            >
                              <Trash2 /> Remove
                            </Button>
                          ) : null}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            ))
          )}
        </CardContent>
      </Card>

      <Dialog open={selection !== null} onOpenChange={(open) => !open && close()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Configure {selection?.secret.name}</DialogTitle>
            <DialogDescription>
              {selection?.scriptName}. The value is encrypted before it is written to runner storage and is never returned to this UI.
            </DialogDescription>
          </DialogHeader>
          <label className="grid gap-1.5 text-sm">
            Secret value
            <Input
              autoComplete="new-password"
              autoFocus
              type="password"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void save();
              }}
            />
          </label>
          <p className="text-xs text-muted-foreground">
            Expected type: {selection?.secret.value_type}. Objects and lists use JSON syntax.
          </p>
          <DialogFooter>
            <Button variant="outline" onClick={close}>Cancel</Button>
            <Button disabled={!selection || value === "" || busyActions.has(`secret-set:${selection.scriptId}:${selection.secret.name}`)} onClick={() => void save()}>
              Save encrypted value
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
