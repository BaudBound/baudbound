import { BundleType } from "@tauri-apps/api/app";

export const LATEST_RELEASE_URL =
  "https://github.com/NATroutter/BaudBound/releases/latest";
export const LINUX_INSTALL_COMMAND =
  "curl -fsSL https://get.baudbound.app/linux | sh";

export type AppInstallationType =
  | "appimage"
  | "deb"
  | "development"
  | "msi"
  | "nsis"
  | "rpm"
  | "unknown";

export type UpdateFailureOperation = "check" | "download" | "install" | "verify";

export function installationTypeFromBundle(bundleType: BundleType): AppInstallationType {
  switch (bundleType) {
    case BundleType.AppImage:
      return "appimage";
    case BundleType.Deb:
      return "deb";
    case BundleType.Msi:
      return "msi";
    case BundleType.Nsis:
      return "nsis";
    case BundleType.Rpm:
      return "rpm";
    default:
      return "unknown";
  }
}

export function isNativeLinuxPackage(type: AppInstallationType) {
  return type === "deb" || type === "rpm";
}

export function canInstallUpdateInApp(type: AppInstallationType) {
  return type === "appimage" || type === "msi" || type === "nsis";
}

export function installationTypeLabel(type: AppInstallationType) {
  switch (type) {
    case "appimage":
      return "Linux AppImage";
    case "deb":
      return "Debian package";
    case "msi":
      return "Windows MSI";
    case "nsis":
      return "Windows installer";
    case "rpm":
      return "RPM package";
    case "development":
      return "Development build";
    default:
      return "Unknown installation";
  }
}

export function classifyDownloadFailure(error: unknown): UpdateFailureOperation {
  const message = String(error).toLowerCase();
  if (
    message.includes("signature") ||
    message.includes("checksum") ||
    message.includes("verification") ||
    message.includes("verify")
  ) {
    return "verify";
  }
  return "download";
}

export function availableUpdateDescription(type: AppInstallationType) {
  if (isNativeLinuxPackage(type)) {
    return "Review the release notes, then update BaudBound with your Linux package manager.";
  }
  if (canInstallUpdateInApp(type)) {
    return "Review the release notes, then download the signed update.";
  }
  return "Review the release notes and open the latest GitHub Release for installation options.";
}
