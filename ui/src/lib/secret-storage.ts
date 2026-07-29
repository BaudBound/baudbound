import type { DashboardPayload } from "@/lib/runner-api";

type StartupSecretStorageState = Pick<
  DashboardPayload,
  "secret_vault" | "stored_secret_value_count"
>;

export function needsStartupSecretUnlock(
  dashboard: StartupSecretStorageState,
): boolean {
  return (
    dashboard.secret_vault.mode === "password" &&
    dashboard.secret_vault.status === "locked" &&
    dashboard.stored_secret_value_count > 0
  );
}
