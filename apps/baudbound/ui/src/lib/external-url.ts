import { openUrl } from "@tauri-apps/plugin-opener";

export function normalizeExternalUrl(value: string) {
  const url = new URL(value);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("Only HTTP and HTTPS links can be opened.");
  }
  return url.href;
}

export function openExternalUrl(value: string) {
  return openUrl(normalizeExternalUrl(value));
}
