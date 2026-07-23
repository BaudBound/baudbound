import { describe, expect, it } from "vitest";

import type {
  DashboardPayload,
  ScriptStatus,
  TriggerAuthStatus,
} from "@/lib/runner-api";
import { networkTriggerAuthRows } from "@/views/security/network-trigger-security-panel";

describe("network trigger security rows", () => {
  it("shows declared network triggers before approval creates credentials", () => {
    const dashboard = dashboardWith(
      scriptWithNetworkTrigger({ state: "missing" }),
      [],
    );

    expect(networkTriggerAuthRows(dashboard)).toEqual([
      expect.objectContaining({
        approvalCurrent: false,
        auth: null,
        nodeId: "webhook-node",
        scriptId: "script-id",
        triggerType: "webhook",
      }),
    ]);
  });

  it("associates credentials created during approval with their trigger", () => {
    const auth: TriggerAuthStatus = {
      auth_enabled: true,
      created_at_unix: 100,
      disabled_at_unix: null,
      node_id: "webhook-node",
      rotated_at_unix: null,
      script_id: "script-id",
      token_preview: "ends in abc123",
      trigger_type: "webhook",
    };
    const dashboard = dashboardWith(
      scriptWithNetworkTrigger({ state: "current" }),
      [auth],
    );

    expect(networkTriggerAuthRows(dashboard)).toEqual([
      expect.objectContaining({ approvalCurrent: true, auth }),
    ]);
  });
});

function dashboardWith(script: ScriptStatus, auth: TriggerAuthStatus[]): DashboardPayload {
  return {
    runner: { scripts: [script] },
    trigger_auth_statuses: { [script.installed.id]: auth },
  } as DashboardPayload;
}

function scriptWithNetworkTrigger(
  approvalStatus: ScriptStatus["approval_status"],
): ScriptStatus {
  return {
    approval_status: approvalStatus,
    declared_permissions: ["webhook_public_bind"],
    installed: {
      asset_count: 0,
      enabled: true,
      id: "script-id",
      imported_at_unix: 1,
      name: "Webhook script",
      package_file_name: "webhook.bbs",
      package_format_version: 1,
      package_hash: "hash",
      package_path: "webhook.bbs",
      risk_level: "high",
      script_language_version: 1,
      target_runtime: "windows-desktop",
    },
    metadata: null,
    package_error: null,
    package_hash_status: { state: "valid" },
    triggers: [
      {
        action_type: "trigger.webhook",
        device_id: null,
        node_id: "webhook-node",
        runner_type: "webhook",
        target: "/events/example",
      },
    ],
  };
}
