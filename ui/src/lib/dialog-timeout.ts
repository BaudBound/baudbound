export function remainingDialogTimeoutMs(timeoutAtUnixMs: number, nowUnixMs: number) {
  return Math.max(0, timeoutAtUnixMs - nowUnixMs);
}

export function formatDialogTimeout(remainingMs: number) {
  const seconds = Math.max(0, Math.ceil(remainingMs / 1_000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}:${remainingMinutes.toString().padStart(2, "0")}:${remainingSeconds.toString().padStart(2, "0")}`;
}
