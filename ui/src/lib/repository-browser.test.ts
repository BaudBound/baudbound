import { describe, expect, it } from "vitest";

import {
  compareSemanticVersions,
  meetsMinimumRunnerVersion,
  repositoryDisplayName,
  repositoryScriptState,
  repositoryUrlForDisplay,
} from "@/lib/repository-browser";

describe("repository browser state", () => {
  it("compares semantic versions without treating prereleases as newer", () => {
    expect(compareSemanticVersions("2.1.0", "2.0.9")).toBeGreaterThan(0);
    expect(compareSemanticVersions("2.0.0-beta.2", "2.0.0")).toBeLessThan(0);
    expect(compareSemanticVersions("v2.0.0", "2.0.0")).toBe(0);
  });

  it("checks the minimum runner version", () => {
    expect(meetsMinimumRunnerVersion("2.0.0", "2.0.0")).toBe(true);
    expect(meetsMinimumRunnerVersion("2.1.0", "2.0.0")).toBe(true);
    expect(meetsMinimumRunnerVersion("2.0.0", "2.1.0")).toBe(false);
  });

  it("gives security and compatibility failures priority", () => {
    expect(
      repositoryScriptState({
        compatible: true,
        informationMismatch: true,
        installed: false,
        installedFromThisRepository: false,
        updateAvailable: false,
      }),
    ).toBe("unavailable");
    expect(
      repositoryScriptState({
        compatible: false,
        informationMismatch: false,
        installed: false,
        installedFromThisRepository: false,
        updateAvailable: false,
      }),
    ).toBe("incompatible");
  });

  it("distinguishes installed and update states", () => {
    expect(
      repositoryScriptState({
        compatible: true,
        informationMismatch: false,
        installed: true,
        installedFromThisRepository: false,
        updateAvailable: false,
      }),
    ).toBe("installed_elsewhere");
    expect(
      repositoryScriptState({
        compatible: true,
        informationMismatch: false,
        installed: true,
        installedFromThisRepository: true,
        updateAvailable: true,
      }),
    ).toBe("update_available");
  });

  it("uses repository names and a stable official fallback", () => {
    expect(
      repositoryDisplayName({ name: "Example Scripts", official: false }),
    ).toBe("Example Scripts");
    expect(repositoryDisplayName({ name: "", official: true })).toBe(
      "BaudBound Official Repository",
    );
  });

  it("hides repository query values in visible URLs", () => {
    expect(
      repositoryUrlForDisplay(
        "https://example.com/repository.json?token=private-value",
      ),
    ).toBe("https://example.com/repository.json?redacted");
  });
});
