import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
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
  STORAGE_PASSWORD_MIN_CHARACTERS,
  passwordCharacterCount,
} from "@/lib/password-strength";
import { unlockSecretStorage } from "@/lib/runner-api";

export function StartupSecretUnlockDialog({
  busy,
  onDismiss,
  open,
  runAction,
  storedValueCount,
}: {
  busy: boolean;
  onDismiss: () => void;
  open: boolean;
  runAction: DashboardAction;
  storedValueCount: number;
}) {
  const [password, setPassword] = useState("");
  const [passwordVisible, setPasswordVisible] = useState(false);

  const close = () => {
    setPassword("");
    setPasswordVisible(false);
    onDismiss();
  };
  const unlock = async () => {
    if (passwordCharacterCount(password) < STORAGE_PASSWORD_MIN_CHARACTERS) {
      return;
    }
    if (
      await runAction("startup-secret-storage-unlock", () =>
        unlockSecretStorage(password),
      )
    ) {
      close();
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Unlock secret storage</DialogTitle>
          <DialogDescription>
            BaudBound found {storedValueCount} saved encrypted{" "}
            {storedValueCount === 1 ? "secret value" : "secret values"}. Enter
            your storage password to make them available for this session.
          </DialogDescription>
        </DialogHeader>
        <label className="grid gap-1.5 text-sm">
          Storage password
          <div className="relative w-full">
            <Input
              autoFocus
              className="secret-value-input w-full pr-10"
              maxLength={1024}
              type={passwordVisible ? "text" : "password"}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void unlock();
              }}
            />
            <button
              aria-label={passwordVisible ? "Hide password" : "Show password"}
              className="absolute inset-y-0 right-0 flex w-10 items-center justify-center text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/45"
              onClick={() => setPasswordVisible((visible) => !visible)}
              type="button"
            >
              {passwordVisible ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
            </button>
          </div>
        </label>
        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Cancel
          </Button>
          <Button
            disabled={
              busy ||
              passwordCharacterCount(password) <
                STORAGE_PASSWORD_MIN_CHARACTERS
            }
            onClick={() => void unlock()}
          >
            Unlock
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
