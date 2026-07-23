import { useState } from "react";

import { DetailDialog } from "@/components/detail-dialog";
import { EmptyState } from "@/components/empty-state";
import { Card, CardContent } from "@/components/ui/card";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type { DashboardAction } from "@/lib/app-types";
import type {
  DashboardPayload,
  ScriptStatus,
  StoredRunRecord,
} from "@/lib/runner-api";
import { useSortableRows } from "@/lib/table-sorting";
import { ScriptApprovalDialog } from "@/views/script-approval-dialog";
import { ScriptDetailPanel } from "@/views/script-detail-panel";
import { ScriptPackageToolbar } from "@/views/script-package-toolbar";
import { ScriptProblemPanel } from "@/views/script-problem-panel";
import { ScriptRow } from "@/views/script-row";
import { ScriptUpdateCheckDialog } from "@/views/script-update-check-dialog";
import { RunDetailPanel } from "@/views/run-detail-panel";

type ScriptSortColumn =
  | "approval"
  | "hash"
  | "name"
  | "risk"
  | "state"
  | "target"
  | "triggers";

const scriptSortSelectors: Record<
  ScriptSortColumn,
  (script: ScriptStatus) => boolean | number | string
> = {
  approval: (script) => script.approval_status.state,
  hash: (script) => script.package_hash_status.state,
  name: (script) => script.installed.name,
  risk: (script) => riskOrder(script.installed.risk_level),
  state: (script) => script.installed.enabled,
  target: (script) => script.installed.target_runtime,
  triggers: (script) => script.triggers.length,
};

function riskOrder(risk: string) {
  if (risk === "low") return 0;
  if (risk === "medium") return 1;
  if (risk === "high") return 2;
  return 3;
}

export function ScriptsView({
  busyActions,
  dashboard,
  onDashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  onDashboard: (dashboard: DashboardPayload) => void;
  runAction: DashboardAction;
}) {
  const [detailScriptId, setDetailScriptId] = useState<string | null>(null);
  const [detailRun, setDetailRun] = useState<StoredRunRecord | null>(null);
  const [approvalScriptId, setApprovalScriptId] = useState<string | null>(null);
  const [checkUpdatesOpen, setCheckUpdatesOpen] = useState(false);
  const { sortedRows: sortedScripts, sortState, toggleSort } = useSortableRows(
    dashboard.runner.scripts,
    scriptSortSelectors,
  );
  const approvalScript = dashboard.runner.scripts.find(
    (script) => script.installed.id === approvalScriptId,
  );
  const detailScript = dashboard.runner.scripts.find(
    (script) => script.installed.id === detailScriptId,
  ) ?? null;

  return (
    <div className="grid gap-4">
      <ScriptPackageToolbar
        busyActions={busyActions}
        canCheckUpdates={dashboard.runner.scripts.some((script) =>
          Boolean(script.metadata?.update_url.trim()),
        )}
        onCheckUpdates={() => setCheckUpdatesOpen(true)}
        runAction={runAction}
      />
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
                  <SortableTableHeader column="name" onSort={toggleSort} sortState={sortState}>
                    Name
                  </SortableTableHeader>
                  <SortableTableHeader column="state" onSort={toggleSort} sortState={sortState}>
                    State
                  </SortableTableHeader>
                  <SortableTableHeader column="risk" onSort={toggleSort} sortState={sortState}>
                    Risk
                  </SortableTableHeader>
                  <SortableTableHeader
                    className="hidden xl:table-cell"
                    column="hash"
                    onSort={toggleSort}
                    sortState={sortState}
                  >
                    Hash
                  </SortableTableHeader>
                  <SortableTableHeader column="approval" onSort={toggleSort} sortState={sortState}>
                    Approval
                  </SortableTableHeader>
                  <SortableTableHeader column="triggers" onSort={toggleSort} sortState={sortState}>
                    Triggers
                  </SortableTableHeader>
                  <SortableTableHeader
                    className="hidden xl:table-cell"
                    column="target"
                    onSort={toggleSort}
                    sortState={sortState}
                  >
                    Target
                  </SortableTableHeader>
                  <th className="px-3 py-2">Update</th>
                  <th className="px-3 py-2">
                    <span className="ml-auto block w-[11.5rem]">Actions</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {sortedScripts.map((script) => (
                  <ScriptRow
                    activeRuns={dashboard.active_runs.filter(
                      (run) => run.script_id === script.installed.id,
                    )}
                    busyActions={busyActions}
                    key={script.installed.id}
                    onReviewApproval={setApprovalScriptId}
                    onViewDetails={setDetailScriptId}
                    runAction={runAction}
                    script={script}
                    updateState={dashboard.script_updates[script.installed.id]}
                  />
                ))}
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
      <DetailDialog
        description={
          detailScript
            ? `${detailScript.installed.name} | ${detailScript.installed.id}`
            : "Script information"
        }
        onOpenChange={(open) => {
          if (!open) {
            setDetailRun(null);
            setDetailScriptId(null);
          }
        }}
        open={detailScript !== null}
        title="Script details"
      >
        {detailScript ? (
          <>
            <ScriptDetailPanel
              busyActions={busyActions}
              onViewRun={setDetailRun}
              recentRuns={dashboard.recent_runs}
              runAction={runAction}
              script={detailScript}
              updateState={dashboard.script_updates[detailScript.installed.id]}
            />
            <DetailDialog
              description={
                detailRun
                  ? `${detailScript.installed.name} | ${detailRun.run_id}`
                  : "Run information"
              }
              onOpenChange={(open) => {
                if (!open) setDetailRun(null);
              }}
              open={detailRun !== null}
              title="Run details"
            >
              {detailRun ? (
                <RunDetailPanel
                  run={detailRun}
                  scriptName={detailScript.installed.name}
                />
              ) : null}
            </DetailDialog>
          </>
        ) : null}
      </DetailDialog>
      <ScriptUpdateCheckDialog
        busyActions={busyActions}
        dashboard={dashboard}
        onDashboard={onDashboard}
        onOpenChange={setCheckUpdatesOpen}
        open={checkUpdatesOpen}
        runAction={runAction}
      />
    </div>
  );
}
