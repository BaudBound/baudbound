import {
  Eye,
  EyeOff,
  HardDrive,
  KeyRound,
  LoaderCircle,
  Lock,
  LockKeyhole,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  Trash2,
  Unlock,
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { SECRET_INPUT_MAX_LENGTH } from "@/lib/input-limits";
import type { DashboardAction } from "@/lib/app-types";
import {
  STORAGE_PASSWORD_MIN_CHARACTERS,
  evaluatePasswordStrength,
  passwordCharacterCount,
} from "@/lib/password-strength";
import {
  type DashboardPayload,
  type InstalledSecretStatus,
  type SecretStorageMode,
  lockSecretStorage,
  removeScriptSecret,
  retrySecretVault,
  setScriptSecret,
  switchSecretStorage,
  unlockSecretStorage,
} from "@/lib/runner-api";

type SecretSelection = {
  scriptId: string;
  scriptName: string;
  secret: InstalledSecretStatus;
};

type StorageSwitch = {
  step: 1 | 2 | 3;
  target: SecretStorageMode;
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
  const [valueVisible, setValueVisible] = useState(false);
  const [value, setValue] = useState("");
  const [unlockOpen, setUnlockOpen] = useState(false);
  const [unlockPassword, setUnlockPassword] = useState("");
  const [storagePasswordVisible, setStoragePasswordVisible] = useState(false);
  const [storageSwitch, setStorageSwitch] = useState<StorageSwitch | null>(null);
  const [storageResetAccepted, setStorageResetAccepted] = useState(false);
  const [storagePassword, setStoragePassword] = useState("");
  const [storagePasswordConfirmation, setStoragePasswordConfirmation] = useState("");
  const scriptsWithSecrets = dashboard.runner.scripts.filter(
    (script) => (dashboard.secret_statuses[script.installed.id] ?? []).length > 0,
  );
  const secretStorageAvailable = dashboard.secret_vault.status === "available";
  const vaultRetryAction = "secret-vault-retry";
  const storageMode = dashboard.secret_vault.mode;
  const storagePasswordStrength = evaluatePasswordStrength(storagePassword);
  const storagePasswordLength = passwordCharacterCount(storagePassword);
  const storageCanChange =
    dashboard.secret_vault.status !== "initializing" &&
    !dashboard.desktop_background.running &&
    dashboard.active_runs.length === 0;
  const storageChangeReason =
    dashboard.secret_vault.status === "initializing"
      ? "Wait for the current secret storage connection to finish."
      : !storageCanChange
        ? "Stop the background runner and all running scripts first."
        : undefined;

  const close = () => {
    setSelection(null);
    setValue("");
    setValueVisible(false);
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
  const closeUnlock = () => {
    setUnlockOpen(false);
    setUnlockPassword("");
    setStoragePasswordVisible(false);
  };
  const closeStorageSwitch = () => {
    setStorageSwitch(null);
    setStorageResetAccepted(false);
    setStoragePassword("");
    setStoragePasswordConfirmation("");
    setStoragePasswordVisible(false);
  };
  const unlock = async () => {
    if (
      passwordCharacterCount(unlockPassword) <
      STORAGE_PASSWORD_MIN_CHARACTERS
    ) {
      return;
    }
    if (
      await runAction("secret-storage-unlock", () =>
        unlockSecretStorage(unlockPassword),
      )
    ) {
      closeUnlock();
    }
  };
  const applyStorageSwitch = async () => {
    if (!storageSwitch || !storageResetAccepted) return;
    if (
      storageSwitch.target === "password" &&
      (storagePasswordLength < STORAGE_PASSWORD_MIN_CHARACTERS ||
        storagePassword !== storagePasswordConfirmation)
    ) {
      return;
    }
    const target = storageSwitch.target;
    if (
      await runAction("secret-storage-switch", () =>
        switchSecretStorage(
          target,
          target === "password" ? storagePassword : undefined,
        ),
      )
    ) {
      closeStorageSwitch();
    }
  };

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="size-4 text-baud-blue" /> Secret storage
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          <div className="flex flex-wrap items-start justify-between gap-3 rounded-md border border-border bg-background p-3">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                {storageMode === "operating_system" ? (
                  <ShieldCheck className="size-4 text-baud-green" />
                ) : (
                  <Lock className="size-4 text-baud-amber" />
                )}
                <span className="font-medium">
                  {storageMode === "operating_system"
                    ? "Operating system vault"
                    : "Password protected storage"}
                </span>
                <Badge
                  variant={
                    dashboard.secret_vault.status === "available"
                      ? "good"
                      : dashboard.secret_vault.status === "unavailable"
                        ? "destructive"
                        : "muted"
                  }
                >
                  {dashboard.secret_vault.status === "available"
                    ? "Unlocked"
                    : dashboard.secret_vault.status === "initializing"
                      ? "Connecting"
                      : dashboard.secret_vault.status === "locked"
                        ? "Locked"
                        : "Unavailable"}
                </Badge>
              </div>
              <p className="mt-1 max-w-3xl text-xs text-muted-foreground">
                {storageMode === "operating_system"
                  ? "Recommended. The operating system protects the encryption key and unlocks it with your desktop session."
                  : "The encryption key is protected by your password. You must unlock it again after restarting BaudBound."}
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              {storageMode === "password" &&
              dashboard.secret_vault.status === "locked" ? (
                <Button
                  disabled={busyActions.has("secret-storage-unlock")}
                  size="sm"
                  variant="outline"
                  onClick={() => setUnlockOpen(true)}
                >
                  <Unlock /> Unlock
                </Button>
              ) : null}
              {storageMode === "password" &&
              dashboard.secret_vault.status === "available" ? (
                <Button
                  disabled={
                    !storageCanChange ||
                    busyActions.has("secret-storage-lock")
                  }
                  size="sm"
                  title={storageChangeReason}
                  variant="outline"
                  onClick={() =>
                    void runAction("secret-storage-lock", lockSecretStorage)
                  }
                >
                  <Lock /> Lock
                </Button>
              ) : null}
              <Button
                disabled={
                  !storageCanChange ||
                  busyActions.has("secret-storage-switch")
                }
                size="sm"
                title={storageChangeReason}
                variant="outline"
                onClick={() =>
                  setStorageSwitch({
                    step: 1,
                    target:
                      storageMode === "operating_system"
                        ? "password"
                        : "operating_system",
                  })
                }
              >
                Switch to{" "}
                {storageMode === "operating_system"
                  ? "password storage"
                  : "system vault"}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="size-4 text-baud-amber" /> Script secrets
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          {storageMode === "operating_system" &&
          dashboard.secret_vault.status === "initializing" ? (
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
          ) : storageMode === "operating_system" &&
            dashboard.secret_vault.status === "unavailable" ? (
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
          ) : dashboard.secret_vault.status === "locked" ? (
            <div className="flex gap-2 rounded-md border border-baud-amber/40 bg-baud-amber/5 p-3 text-sm">
              <Lock className="mt-0.5 size-4 shrink-0 text-baud-amber" />
              <div>
                <div className="font-medium">Secret storage is locked</div>
                <p className="mt-1 text-xs text-muted-foreground">
                  Unlock password protected storage before configuring or using script secrets.
                </p>
              </div>
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
                              setValueVisible(false);
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
              {selection?.scriptName}. The value is encrypted before it is written to local runner
              storage and is never returned to this interface.
            </DialogDescription>
          </DialogHeader>
          <label className="grid gap-1.5 text-sm">
            Secret value
            <div className="relative w-full">
              <Input
                autoComplete="new-password"
                autoFocus
                className="secret-value-input w-full pr-10"
                maxLength={SECRET_INPUT_MAX_LENGTH}
                type={valueVisible ? "text" : "password"}
                value={value}
                onChange={(event) => setValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void save();
                }}
              />
              <button
                aria-label={valueVisible ? "Hide secret value" : "Show secret value"}
                className="absolute inset-y-0 right-0 flex w-10 items-center justify-center text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/45"
                onClick={() => setValueVisible((visible) => !visible)}
                title={valueVisible ? "Hide secret value" : "Show secret value"}
                type="button"
              >
                {valueVisible ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </button>
            </div>
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

      <Dialog open={unlockOpen} onOpenChange={(open) => !open && closeUnlock()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Unlock secret storage</DialogTitle>
            <DialogDescription>
              Enter the password you chose for this runner. BaudBound uses it only to unlock the
              encryption key for this session.
            </DialogDescription>
          </DialogHeader>
          <label className="grid gap-1.5 text-sm">
            Storage password
            <div className="relative">
              <Input
                autoComplete="current-password"
                autoFocus
                className="secret-value-input pr-10"
                maxLength={1024}
                type={storagePasswordVisible ? "text" : "password"}
                value={unlockPassword}
                onChange={(event) => setUnlockPassword(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void unlock();
                }}
              />
              <button
                aria-label={storagePasswordVisible ? "Hide password" : "Show password"}
                className="absolute inset-y-0 right-0 flex w-10 items-center justify-center text-muted-foreground hover:text-foreground"
                onClick={() => setStoragePasswordVisible((visible) => !visible)}
                type="button"
              >
                {storagePasswordVisible ? <EyeOff /> : <Eye />}
              </button>
            </div>
          </label>
          <DialogFooter>
            <Button variant="outline" onClick={closeUnlock}>Cancel</Button>
            <Button
              disabled={
                passwordCharacterCount(unlockPassword) <
                  STORAGE_PASSWORD_MIN_CHARACTERS ||
                busyActions.has("secret-storage-unlock")
              }
              onClick={() => void unlock()}
            >
              Unlock
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={storageSwitch !== null}
        onOpenChange={(open) => !open && closeStorageSwitch()}
      >
        <DialogContent>
          {storageSwitch?.step === 1 ? (
            <>
              <DialogHeader>
                <DialogTitle>
                  {storageSwitch.target === "password"
                    ? "Use password protected storage?"
                    : "Return to the operating system vault?"}
                </DialogTitle>
                <DialogDescription>
                  {storageSwitch.target === "password"
                    ? "This optional mode is less convenient and generally less secure than the operating system vault. You must unlock it after every BaudBound restart. If you forget the password, the saved secrets cannot be recovered."
                    : "The operating system vault is the recommended storage mode. Your desktop session controls access to its encryption key."}
                </DialogDescription>
              </DialogHeader>
              <div className="rounded-md border border-baud-amber/40 bg-baud-amber/5 p-3 text-sm">
                BaudBound does not migrate saved secrets between storage modes.
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={closeStorageSwitch}>Cancel</Button>
                <Button
                  onClick={() =>
                    setStorageSwitch((current) =>
                      current ? { ...current, step: 2 } : null,
                    )
                  }
                >
                  Continue
                </Button>
              </DialogFooter>
            </>
          ) : storageSwitch?.step === 2 ? (
            <>
              <DialogHeader>
                <DialogTitle>Saved secrets will be erased</DialogTitle>
                <DialogDescription>
                  Switching storage permanently removes every configured script secret from this
                  runner. You will need to configure each value again.
                </DialogDescription>
              </DialogHeader>
              <label className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm">
                <Checkbox
                  checked={storageResetAccepted}
                  className="mt-0.5"
                  onCheckedChange={(checked) =>
                    setStorageResetAccepted(checked === true)
                  }
                />
                <span>I understand that all saved script secrets will be permanently erased.</span>
              </label>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() =>
                    setStorageSwitch((current) =>
                      current ? { ...current, step: 1 } : null,
                    )
                  }
                >
                  Back
                </Button>
                <Button
                  disabled={
                    !storageResetAccepted ||
                    busyActions.has("secret-storage-switch")
                  }
                  variant="destructive"
                  onClick={() => {
                    if (storageSwitch.target === "password") {
                      setStorageSwitch({ ...storageSwitch, step: 3 });
                    } else {
                      void applyStorageSwitch();
                    }
                  }}
                >
                  {storageSwitch.target === "password"
                    ? "Continue"
                    : "Erase secrets and switch"}
                </Button>
              </DialogFooter>
            </>
          ) : storageSwitch?.step === 3 ? (
            <>
              <DialogHeader>
                <DialogTitle>Create storage password</DialogTitle>
                <DialogDescription>
                  Use at least 8 characters. This password is not saved and cannot be recovered.
                </DialogDescription>
              </DialogHeader>
              <div className="grid gap-3">
                <label className="grid gap-1.5 text-sm">
                  Password
                  <Input
                    autoComplete="new-password"
                    autoFocus
                    className="secret-value-input"
                    maxLength={1024}
                    type="password"
                    value={storagePassword}
                    onChange={(event) => setStoragePassword(event.target.value)}
                  />
                </label>
                <div className="grid gap-1.5" aria-live="polite">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-muted-foreground">Password strength</span>
                    <span
                      className={
                        storagePasswordStrength.score <= 1
                          ? "text-destructive"
                          : storagePasswordStrength.score === 2
                            ? "text-baud-amber"
                            : "text-baud-green"
                      }
                    >
                      {storagePasswordStrength.label}
                    </span>
                  </div>
                  <div
                    aria-label="Password strength"
                    aria-valuemax={4}
                    aria-valuemin={0}
                    aria-valuenow={storagePasswordStrength.score}
                    className="h-1.5 overflow-hidden rounded-sm bg-muted"
                    role="meter"
                  >
                    <div
                      className={
                        storagePasswordStrength.score <= 1
                          ? "h-full bg-destructive transition-[width]"
                          : storagePasswordStrength.score === 2
                            ? "h-full bg-baud-amber transition-[width]"
                            : "h-full bg-baud-green transition-[width]"
                      }
                      style={{ width: `${storagePasswordStrength.score * 25}%` }}
                    />
                  </div>
                </div>
                <label className="grid gap-1.5 text-sm">
                  Confirm password
                  <Input
                    autoComplete="new-password"
                    className="secret-value-input"
                    maxLength={1024}
                    type="password"
                    value={storagePasswordConfirmation}
                    onChange={(event) =>
                      setStoragePasswordConfirmation(event.target.value)
                    }
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void applyStorageSwitch();
                    }}
                  />
                </label>
                {storagePasswordConfirmation &&
                storagePassword !== storagePasswordConfirmation ? (
                  <p className="text-xs text-destructive">The passwords do not match.</p>
                ) : null}
              </div>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() =>
                    setStorageSwitch((current) =>
                      current ? { ...current, step: 2 } : null,
                    )
                  }
                >
                  Back
                </Button>
                <Button
                  disabled={
                    storagePasswordLength < STORAGE_PASSWORD_MIN_CHARACTERS ||
                    storagePassword !== storagePasswordConfirmation ||
                    busyActions.has("secret-storage-switch")
                  }
                  variant="destructive"
                  onClick={() => void applyStorageSwitch()}
                >
                  Erase secrets and switch
                </Button>
              </DialogFooter>
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
