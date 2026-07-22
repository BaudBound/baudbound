import { describe, expect, it } from "vitest";
import { BundleType } from "@tauri-apps/api/app";

import {
  availableUpdateDescription,
  canInstallUpdateInApp,
  classifyDownloadFailure,
  installationTypeFromBundle,
  isNativeLinuxPackage,
} from "@/lib/update-policy";

describe("update policy", () => {
  it.each([
    [BundleType.AppImage, "appimage"],
    [BundleType.Deb, "deb"],
    [BundleType.Msi, "msi"],
    [BundleType.Nsis, "nsis"],
    [BundleType.Rpm, "rpm"],
  ] as const)("maps %s bundles", (bundle, expected) => {
    expect(installationTypeFromBundle(bundle)).toBe(expected);
  });

  it("allows only self-updating bundle types to install in the app", () => {
    expect(canInstallUpdateInApp("appimage")).toBe(true);
    expect(canInstallUpdateInApp("nsis")).toBe(true);
    expect(canInstallUpdateInApp("msi")).toBe(true);
    expect(canInstallUpdateInApp("deb")).toBe(false);
    expect(canInstallUpdateInApp("rpm")).toBe(false);
    expect(canInstallUpdateInApp("unknown")).toBe(false);
  });

  it("identifies native Linux packages", () => {
    expect(isNativeLinuxPackage("deb")).toBe(true);
    expect(isNativeLinuxPackage("rpm")).toBe(true);
    expect(isNativeLinuxPackage("appimage")).toBe(false);
  });

  it("uses package-manager instructions for native packages", () => {
    expect(availableUpdateDescription("deb")).toContain("Linux package manager");
    expect(availableUpdateDescription("rpm")).toContain("Linux package manager");
    expect(availableUpdateDescription("appimage")).toContain("download the signed update");
  });

  it("distinguishes verification failures from transport failures", () => {
    expect(classifyDownloadFailure(new Error("signature verification failed"))).toBe("verify");
    expect(classifyDownloadFailure(new Error("network connection closed"))).toBe("download");
  });
});
