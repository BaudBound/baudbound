import { describe, expect, it } from "vitest";

import { needsStartupSecretUnlock } from "@/lib/secret-storage";

describe("startup secret storage prompt", () => {
  it("requires an unlock for a locked password vault containing values", () => {
    expect(
      needsStartupSecretUnlock({
        secret_vault: {
          error: null,
          mode: "password",
          status: "locked",
        },
        stored_secret_value_count: 2,
      }),
    ).toBe(true);
  });

  it("does not prompt for an empty, available, or operating system vault", () => {
    for (const state of [
      {
        secret_vault: {
          error: null,
          mode: "password" as const,
          status: "locked" as const,
        },
        stored_secret_value_count: 0,
      },
      {
        secret_vault: {
          error: null,
          mode: "password" as const,
          status: "available" as const,
        },
        stored_secret_value_count: 1,
      },
      {
        secret_vault: {
          error: null,
          mode: "operating_system" as const,
          status: "locked" as const,
        },
        stored_secret_value_count: 1,
      },
    ]) {
      expect(needsStartupSecretUnlock(state)).toBe(false);
    }
  });
});
