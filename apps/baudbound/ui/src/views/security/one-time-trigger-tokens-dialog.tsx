import { Copy, KeyRound } from "lucide-react";
import { toast } from "sonner";

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
import type { GeneratedTriggerToken } from "@/lib/runner-api";

export function OneTimeTriggerTokensDialog({
  onDone,
  tokens,
}: {
  onDone: () => void;
  tokens: GeneratedTriggerToken[];
}) {
  async function copy(value: string, message: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(message);
    } catch (error) {
      toast.error(`Could not copy token: ${String(error)}`);
    }
  }

  const allTokens = tokens
    .map(
      ({ status, token }) =>
        `${status.script_id} ${status.node_id} ${status.trigger_type} ${token}`,
    )
    .join("\n");

  return (
    <Dialog open={tokens.length > 0}>
      <DialogContent
        className="max-h-[min(760px,calc(100vh-2rem))] w-[min(calc(100vw-2rem),680px)] grid-rows-[auto_minmax(0,1fr)_auto]"
        onEscapeKeyDown={(event) => event.preventDefault()}
        onPointerDownOutside={(event) => event.preventDefault()}
        showCloseButton={false}
      >
        <DialogHeader className="pr-0">
          <div className="flex items-center gap-2">
            <KeyRound className="size-4 text-baud-amber" />
            <DialogTitle>Save new network trigger tokens</DialogTitle>
          </div>
          <DialogDescription>
            These tokens are shown only once. Store them securely before continuing. Lost tokens
            cannot be recovered and must be replaced by generating new ones in Security.
          </DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 gap-3 overflow-y-auto pr-1">
          {tokens.map(({ status, token }) => (
            <section
              className="grid gap-2 rounded-md border border-border bg-background p-3"
              key={`${status.script_id}:${status.node_id}:${status.trigger_type}`}
            >
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="good">Protected</Badge>
                <Badge variant="muted">{triggerTypeLabel(status.trigger_type)}</Badge>
                <span className="break-all font-mono text-xs text-muted-foreground">
                  {status.script_id}:{status.node_id}
                </span>
              </div>
              <div className="flex min-w-0 items-stretch gap-2">
                <code className="min-w-0 flex-1 select-text break-all rounded-md border border-border bg-card px-3 py-2 font-mono text-sm">
                  {token}
                </code>
                <Button
                  aria-label={`Copy token for ${status.node_id}`}
                  className="shrink-0"
                  onClick={() => void copy(token, `Token for ${status.node_id} copied.`)}
                  title={`Copy token for ${status.node_id}`}
                  variant="outline"
                >
                  <Copy />
                </Button>
              </div>
            </section>
          ))}
        </div>

        <DialogFooter>
          {tokens.length > 1 ? (
            <Button
              onClick={() => void copy(allTokens, "All network trigger tokens copied.")}
              variant="outline"
            >
              <Copy />
              Copy all
            </Button>
          ) : null}
          <Button onClick={onDone}>I have saved the tokens</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function triggerTypeLabel(triggerType: GeneratedTriggerToken["status"]["trigger_type"]) {
  return triggerType === "websocket" ? "WebSocket" : "Webhook";
}
