import { AlertTriangle, ChevronRight, ShieldCheck } from "lucide-react";

import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatCount } from "@/lib/count-format";
import type { InstalledScriptSettingStatus, ScriptStatus } from "@/lib/runner-api";
import { hasApprovalProblem, scriptProblems } from "@/lib/script-problems";
import { approvalLabel, packageHashLabel, riskVariant } from "@/lib/status-format";

export function ScriptProblemPanel({
  onApproveScript,
  scriptSettings,
  scripts,
}: {
  onApproveScript: (scriptId: string) => void;
  scriptSettings: Record<string, InstalledScriptSettingStatus[]>;
  scripts: ScriptStatus[];
}) {
  const scriptsWithProblems = scripts
    .map((script) => {
      const problems = scriptProblems(script);
      const missingRequiredSettings = (scriptSettings[script.installed.id] ?? []).filter(
        (setting) => setting.required && setting.effective_value === null,
      );
      if (script.installed.enabled && missingRequiredSettings.length > 0) {
        problems.push({
          detail: `Configure ${missingRequiredSettings
            .map((setting) => setting.name)
            .join(", ")} or disable this script.`,
          id: "required-script-settings",
          severity: "error",
          title: "Required Script Settings are missing",
        });
      }
      return {
        problems: script.installed.enabled
          ? problems
          : problems.filter((problem) => problem.id.startsWith("approval-")),
        script,
      };
    })
    .filter(({ problems }) => problems.length > 0);

  if (scriptsWithProblems.length === 0) return null;

  const errorCount = scriptsWithProblems.reduce(
    (count, item) => count + item.problems.filter((problem) => problem.severity === "error").length,
    0,
  );
  const warningCount = scriptsWithProblems.reduce(
    (count, item) => count + item.problems.filter((problem) => problem.severity === "warning").length,
    0,
  );

  return (
    <Card>
      <CardHeader className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <CardTitle>Scripts needing attention</CardTitle>
          <p className="mt-1 text-xs text-muted-foreground">
            Resolve approval, package integrity, Script Settings, and trigger registration issues before relying on
            automatic execution.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {errorCount > 0 ? <Badge variant="destructive">{formatCount(errorCount, "error")}</Badge> : null}
          {warningCount > 0 ? <Badge variant="medium">{formatCount(warningCount, "warning")}</Badge> : null}
        </div>
      </CardHeader>
      <CardContent className="grid gap-3">
        {scriptsWithProblems.map(({ problems, script }) => (
          <div
            className="grid min-w-0 gap-4 overflow-hidden rounded-md border border-border bg-background p-3 xl:grid-cols-[minmax(0,1fr)_minmax(14rem,18rem)]"
            key={script.installed.id}
          >
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium">{script.installed.name}</span>
                <Badge variant={riskVariant(script.installed.risk_level)}>{script.installed.risk_level}</Badge>
              </div>
              <div className="mt-1 break-all font-mono text-xs text-muted-foreground">{script.installed.id}</div>
              <div className="mt-3 grid gap-2">
                {problems.map((problem) => (
                  <div className="flex min-w-0 items-start gap-2 text-sm" key={problem.id}>
                    <AlertTriangle
                      className={
                        problem.severity === "error"
                          ? "mt-0.5 size-4 shrink-0 text-destructive"
                          : "mt-0.5 size-4 shrink-0 text-baud-amber"
                      }
                    />
                    <div className="min-w-0 flex-1">
                      <div className="font-medium">{problem.title}</div>
                      <p className="break-words text-muted-foreground">{problem.detail}</p>
                      {problem.advancedDetail ? (
                        <details className="group mt-2 min-w-0 max-w-full overflow-hidden">
                          <summary className="flex w-fit max-w-full cursor-pointer list-none items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
                            <ChevronRight className="size-3.5 transition-transform group-open:rotate-90" />
                            Advanced details
                          </summary>
                          <pre className="mt-2 max-h-64 min-w-0 max-w-full select-text overflow-x-hidden overflow-y-auto whitespace-pre-wrap break-all rounded-md border border-border bg-card p-3 font-mono text-xs leading-5 text-foreground">
                            {problem.advancedDetail}
                          </pre>
                        </details>
                      ) : null}
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="grid min-w-0 content-start gap-3">
              <Details
                rows={[
                  ["Approval", approvalLabel(script.approval_status)],
                  ["Hash", packageHashLabel(script.package_hash_status)],
                  ["Target runtimes", script.installed.target_runtime],
                  ["Triggers", script.triggers.length.toString()],
                ]}
              />
              {!script.package_error && hasApprovalProblem(script.approval_status) ? (
                <Button
                  className="w-full max-w-full whitespace-normal"
                  onClick={() => onApproveScript(script.installed.id)}
                >
                  <ShieldCheck />
                  Review approval
                </Button>
              ) : null}
            </div>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
