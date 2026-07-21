import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ExternalLink } from "@/components/external-link";
import type { ScriptStatus } from "@/lib/runner-api";

export function ScriptAboutDialog({
  onOpenChange,
  open,
  script,
}: {
  onOpenChange: (open: boolean) => void;
  open: boolean;
  script: ScriptStatus | null;
}) {
  if (!script) return null;
  const metadata = script.metadata;

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="max-h-[min(760px,calc(100vh-2rem))] w-[min(680px,calc(100vw-2rem))] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{script.installed.name}</DialogTitle>
          <DialogDescription>
            Information provided by this installed package.
          </DialogDescription>
        </DialogHeader>

        {metadata ? (
          <div className="grid gap-5 text-sm">
            {metadata.description.trim() ? (
              <p className="select-text leading-6 text-foreground">{metadata.description}</p>
            ) : null}

            <MetadataRows
              rows={[
                ["Author", metadata.author],
                ["Created with", metadata.created_with],
                ["Created", metadata.created_at],
                ["Updated", metadata.updated_at],
                ["Minimum runner", metadata.minimum_runner_version],
                ["Target runtime", script.installed.target_runtime],
                ["Script ID", script.installed.id],
                ["Package format", script.installed.package_format_version.toString()],
                ["Runtime format", script.installed.script_language_version.toString()],
              ]}
            />

            {metadata.website.trim() || metadata.repository.trim() ? (
              <section className="grid gap-2">
                <h3 className="font-medium">Links</h3>
                {metadata.website.trim() ? (
                  <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3">
                    <span className="text-muted-foreground">Website</span>
                    <ExternalLink href={metadata.website}>{metadata.website}</ExternalLink>
                  </div>
                ) : null}
                {metadata.repository.trim() ? (
                  <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3">
                    <span className="text-muted-foreground">Repository</span>
                    <ExternalLink href={metadata.repository}>{metadata.repository}</ExternalLink>
                  </div>
                ) : null}
              </section>
            ) : null}

            {metadata.tags.length > 0 ? (
              <section className="grid gap-2">
                <h3 className="font-medium">Tags</h3>
                <div className="flex flex-wrap gap-1.5">
                  {metadata.tags.map((tag) => (
                    <Badge key={tag} variant="muted">{tag}</Badge>
                  ))}
                </div>
              </section>
            ) : null}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            Package information is unavailable because the installed package could not be read and verified.
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}

function MetadataRows({ rows }: { rows: Array<[string, string]> }) {
  const visibleRows = rows.filter(([, value]) => value.trim().length > 0);
  return (
    <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-2">
      {visibleRows.map(([label, value]) => (
        <div className="contents" key={label}>
          <dt className="text-muted-foreground">{label}</dt>
          <dd className="min-w-0 select-text break-words">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
