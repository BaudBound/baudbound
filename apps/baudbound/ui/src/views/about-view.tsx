import { CheckCircle2, Download, RefreshCw, RotateCw, TriangleAlert } from "lucide-react";

import { ExternalLink } from "@/components/external-link";
import { LazyMarkdownContent } from "@/components/lazy-markdown-content";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { AppUpdaterController, AppUpdateState } from "@/hooks/use-app-updater";
import { updateProgressPercent } from "@/lib/update-progress";
import { useDesktopTime } from "@/lib/time-format";

export function AboutView({ updater }: { updater: AppUpdaterController }) {
  const { formatDateTime } = useDesktopTime();
  const { checkForUpdate, download, installAndRestart, state } = updater;
  const progress = updateProgressPercent(state.progress);

  return (
    <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(22rem,0.8fr)]">
      <div className="grid content-start gap-4">
        <Card>
          <CardContent className="flex flex-wrap items-start gap-4">
            <img alt="" className="size-16 rounded-md" draggable={false} src="/logo-notext.svg" />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-lg font-semibold">BaudBound</h2>
                <Badge variant="muted">{state.currentVersion ?? "Version unavailable"}</Badge>
              </div>
              <p className="mt-2 max-w-3xl text-sm leading-6 text-muted-foreground">
                A local automation runner for executing, scheduling, and monitoring BaudBound scripts on Windows and Linux.
              </p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader><CardTitle>Project links</CardTitle></CardHeader>
          <CardContent className="grid gap-3 sm:grid-cols-2">
            <ProjectLink href="https://baudbound.app" label="Website" />
            <ProjectLink href="https://wiki.baudbound.app" label="Documentation" />
            <ProjectLink href="https://github.com/NATroutter/BaudBound" label="Source repository" />
            <ProjectLink href="https://github.com/NATroutter/BaudBound/issues" label="Report an issue" />
          </CardContent>
        </Card>

        <Card>
          <CardHeader><CardTitle>Credits and licensing</CardTitle></CardHeader>
          <CardContent className="grid gap-3 text-sm leading-6 text-muted-foreground">
            <p>Created and maintained by NATroutter.</p>
            <p>
              BaudBound software is provided under the PolyForm Noncommercial License 1.0.0. Documentation and original creative content use CC BY-NC-SA 4.0. BaudBound names and marks remain the property of NATroutter.
            </p>
            <ExternalLink href="https://wiki.baudbound.app/licensing">
              Read licensing and attribution details
            </ExternalLink>
            <p className="text-xs">Copyright (c) 2026 NATroutter.</p>
          </CardContent>
        </Card>
      </div>

      <Card className="self-start">
        <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-3">
          <div>
            <CardTitle>Application updates</CardTitle>
            {state.lastCheckedAt ? (
              <p className="mt-1 text-xs text-muted-foreground">
                Last checked {formatDateTime(new Date(state.lastCheckedAt))}
              </p>
            ) : null}
          </div>
          <UpdateBadge state={state} />
        </CardHeader>
        <CardContent className="grid gap-4">
          <UpdateSummary state={state} />

          {state.phase === "downloading" ? (
            <div className="grid gap-2" aria-live="polite">
              <div className="flex justify-between gap-3 text-xs text-muted-foreground">
                <span>Downloading and verifying</span>
                <span>{progress === null ? "In progress" : `${progress}%`}</span>
              </div>
              <div className="h-2 overflow-hidden rounded-sm bg-muted">
                <div
                  className={progress === null ? "h-full w-1/3 animate-pulse bg-primary" : "h-full bg-primary"}
                  style={progress === null ? undefined : { width: `${progress}%` }}
                />
              </div>
            </div>
          ) : null}

          {state.releaseNotes ? (
            <section className="grid gap-2">
              <h3 className="text-sm font-medium">Release notes</h3>
              <div className="max-h-80 overflow-y-auto rounded-md border border-border bg-background p-3">
                <LazyMarkdownContent source={state.releaseNotes} />
              </div>
            </section>
          ) : null}

          <div className="flex flex-wrap gap-2">
            <Button
              disabled={state.phase === "checking" || state.phase === "downloading" || state.phase === "ready"}
              onClick={() => void checkForUpdate()}
              variant="outline"
            >
              <RefreshCw className={state.phase === "checking" ? "animate-spin" : undefined} />
              Check for updates
            </Button>
            {state.phase === "available" ? (
              <Button onClick={() => void download()}>
                <Download />
                Download update
              </Button>
            ) : null}
            {state.phase === "ready" ? (
              <Button onClick={() => void installAndRestart()}>
                <RotateCw />
                Restart and install
              </Button>
            ) : null}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function ProjectLink({ href, label }: { href: string; label: string }) {
  return (
    <div className="min-w-0 py-1 text-sm">
      <ExternalLink href={href}>{label}</ExternalLink>
    </div>
  );
}

function UpdateBadge({ state }: { state: AppUpdateState }) {
  if (state.phase === "available" || state.phase === "downloading" || state.phase === "ready") {
    return <Badge variant="medium">Update available</Badge>;
  }
  if (state.phase === "error") return <Badge variant="destructive">Check failed</Badge>;
  if (state.phase === "checking") return <Badge variant="muted">Checking</Badge>;
  if (state.phase === "up_to_date") return <Badge variant="good">Up to date</Badge>;
  return <Badge variant="muted">Not checked</Badge>;
}

function UpdateSummary({ state }: { state: AppUpdateState }) {
  if (state.phase === "error") {
    return (
      <div className="flex items-start gap-2 rounded-md border border-destructive/25 bg-destructive/10 p-3 text-sm text-destructive">
        <TriangleAlert className="mt-0.5 size-4 shrink-0" />
        <span className="min-w-0 select-text break-words">{state.error}</span>
      </div>
    );
  }
  if (state.phase === "up_to_date") {
    return (
      <div className="flex items-start gap-2 text-sm text-baud-green">
        <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
        <span>BaudBound {state.currentVersion ?? ""} is the newest available version.</span>
      </div>
    );
  }
  if (state.version) {
    return (
      <p className="text-sm text-muted-foreground">
        Version <span className="text-foreground">{state.version}</span> is available. The update is signed and verified before installation.
      </p>
    );
  }
  return (
    <p className="text-sm text-muted-foreground">
      Check the official signed release feed for a newer BaudBound version.
    </p>
  );
}
