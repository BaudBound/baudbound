import { openUrl } from "@tauri-apps/plugin-opener";

export function normalizeExternalUrl(value: string) {
  const url = new URL(value.trim());
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("Only HTTP and HTTPS links can be opened.");
  }
  return url.href;
}

export function tryNormalizeExternalUrl(value: string) {
  try {
    return normalizeExternalUrl(value);
  } catch {
    return null;
  }
}

export function openExternalUrl(value: string) {
  return openUrl(normalizeExternalUrl(value));
}
