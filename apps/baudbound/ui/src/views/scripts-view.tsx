import { Fragment, useState } from "react";

import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import type { DashboardPayload } from "@/lib/runner-api";
import { ScriptApprovalDialog } from "@/views/script-approval-dialog";
import { ScriptDetailPanel } from "@/views/script-detail-panel";
import { ScriptPackageToolbar } from "@/views/script-package-toolbar";
import { ScriptProblemPanel } from "@/views/script-problem-panel";
import { ScriptRow } from "@/views/script-row";

export function ScriptsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [expandedScriptIds, setExpandedScriptIds] = useState<Set<string>>(new Set());
  const [approvalScriptId, setApprovalScriptId] = useState<string | null>(null);
  const approvalScript = dashboard.runner.scripts.find(
    (script) => script.installed.id === approvalScriptId,
  );

  function toggleScriptDetails(scriptId: string) {
    setExpandedScriptIds((current) => {
      const next = new Set(current);
      if (next.has(scriptId)) {
        next.delete(scriptId);
      } else {
        next.add(scriptId);
      }
      return next;
    });
  }

  return (
    <div className="grid gap-4">
      <ScriptPackageToolbar busyActions={busyActions} runAction={runAction} />
      <ScriptProblemPanel
        onApproveScript={setApprovalScriptId}
        scripts={dashboard.runner.scripts}
      />
      {dashboard.runner.scripts.length === 0 ? (
        <EmptyState>No scripts are installed.</EmptyState>
      ) : (
        <Card>
          <CardContent className="overflow-x-auto p-0 max-[1280px]:p-3">
            <table className="responsive-table w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <th className="px-3 py-2">Name</th>
                  <th className="px-3 py-2">State</th>
                  <th className="px-3 py-2">Risk</th>
                  <th className="hidden px-3 py-2 xl:table-cell">Hash</th>
                  <th className="px-3 py-2">Approval</th>
                  <th className="px-3 py-2">Triggers</th>
                  <th className="hidden px-3 py-2 xl:table-cell">Target</th>
                  <th className="px-3 py-2">Actions</th>
                </tr>
              </thead>
              <tbody>
                {dashboard.runner.scripts.map((script) => {
                  const expanded = expandedScriptIds.has(script.installed.id);
                  return (
                    <Fragment key={script.installed.id}>
                      <ScriptRow
                        activeRuns={dashboard.active_runs.filter(
                          (run) => run.script_id === script.installed.id,
                        )}
                        busyActions={busyActions}
                        expanded={expanded}
                        onReviewApproval={setApprovalScriptId}
                        onToggleDetails={toggleScriptDetails}
                        runAction={runAction}
                        script={script}
                      />
                      {expanded ? (
                        <tr className="border-b border-border bg-background/40">
                          <td className="p-3" colSpan={8} data-label="">
                            <ScriptDetailPanel
                              recentRuns={dashboard.recent_runs}
                              script={script}
                            />
                          </td>
                        </tr>
                      ) : null}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          </CardContent>
        </Card>
      )}
      {approvalScript ? (
        <ScriptApprovalDialog
          approveBusy={busyActions.has(`approve:${approvalScript.installed.id}`)}
          onOpenChange={(open) => {
            if (!open) setApprovalScriptId(null);
          }}
          open={approvalScriptId !== null}
          runAction={runAction}
          script={approvalScript}
          revokeBusy={busyActions.has(`revoke-approval:${approvalScript.installed.id}`)}
        />
      ) : null}
    </div>
  );
}
