import { AlertTriangle, ShieldCheck } from "lucide-react";

import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { ScriptStatus } from "@/lib/runner-api";
import { hasApprovalProblem, scriptProblems } from "@/lib/script-problems";
import { approvalLabel, packageHashLabel, riskVariant } from "@/lib/status-format";

export function ScriptProblemPanel({
  onApproveScript,
  scripts,
}: {
  onApproveScript: (scriptId: string) => void;
  scripts: ScriptStatus[];
}) {
  const scriptsWithProblems = scripts
    .filter((script) => script.installed.enabled)
    .map((script) => ({ problems: scriptProblems(script), script }))
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
          <CardTitle>Script attention needed</CardTitle>
          <p className="mt-1 text-xs text-muted-foreground">
            Resolve approval, package integrity, and trigger registration issues before relying on automatic execution.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {errorCount > 0 ? <Badge variant="destructive">{errorCount} errors</Badge> : null}
          {warningCount > 0 ? <Badge variant="medium">{warningCount} warnings</Badge> : null}
        </div>
      </CardHeader>
      <CardContent className="grid gap-3">
        {scriptsWithProblems.map(({ problems, script }) => (
          <div
            className="grid gap-3 rounded-md border border-border bg-background p-3 lg:grid-cols-[minmax(0,1fr)_auto]"
            key={script.installed.id}
          >
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium">{script.installed.name}</span>
                <Badge variant={riskVariant(script.installed.risk_level)}>
                  {script.installed.risk_level}
                </Badge>
              </div>
              <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
                {script.installed.id}
              </div>
              <div className="mt-3 grid gap-2">
                {problems.map((problem) => (
                  <div className="flex gap-2 text-sm" key={problem.id}>
                    <AlertTriangle
                      className={
                        problem.severity === "error"
                          ? "mt-0.5 size-4 shrink-0 text-destructive"
                          : "mt-0.5 size-4 shrink-0 text-baud-amber"
                      }
                    />
                    <div>
                      <div className="font-medium">{problem.title}</div>
                      <div className="text-muted-foreground">{problem.detail}</div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="grid gap-3 lg:min-w-72">
              <Details
                rows={[
                  ["Approval", approvalLabel(script.approval_status)],
                  ["Hash", packageHashLabel(script.package_hash_status)],
                  ["Target", script.installed.target_runtime],
                  ["Triggers", script.triggers.length.toString()],
                ]}
              />
              {hasApprovalProblem(script.approval_status) ? (
                <Button className="w-full" onClick={() => onApproveScript(script.installed.id)}>
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
