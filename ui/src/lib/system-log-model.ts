import type {
  NewSystemLog,
  SystemLogDetail,
  SystemLogSeverity,
} from "@/lib/runner-api";

const MAX_DETAIL_COUNT = 32;
const MAX_DETAILS_VALUE_BYTES = 120 * 1024;
const MAX_DETAIL_LABEL_BYTES = 128;
const MAX_DETAIL_VALUE_BYTES = 32 * 1024;
const MAX_MESSAGE_BYTES = 8 * 1024;
const MAX_SOURCE_BYTES = 128;
const MAX_TITLE_BYTES = 256;

export type SystemLogOptions = {
  details?: SystemLogDetail[];
  error?: unknown;
  source: string;
  title?: string;
};

export function createSystemLog(
  severity: SystemLogSeverity,
  message: string,
  options: SystemLogOptions,
): NewSystemLog {
  const source = boundedRequiredText(options.source, MAX_SOURCE_BYTES, "Runner");
  const title = boundedRequiredText(options.title ?? source, MAX_TITLE_BYTES, source);
  const details = boundDetails([
    ...(options.details ?? []),
    ...errorDetails(options.error),
  ]);

  return {
    details,
    message: boundedRequiredText(message, MAX_MESSAGE_BYTES, title),
    severity,
    source,
    title,
  };
}

function boundDetails(details: SystemLogDetail[]) {
  let remainingValueBytes = MAX_DETAILS_VALUE_BYTES;
  return details.slice(0, MAX_DETAIL_COUNT).map((detail, index) => {
    const label = boundedRequiredText(
      detail.label,
      MAX_DETAIL_LABEL_BYTES,
      `Detail ${index + 1}`,
    );
    const valueLimit = Math.min(MAX_DETAIL_VALUE_BYTES, remainingValueBytes);
    const value = truncateUtf8(detail.value, valueLimit);
    remainingValueBytes = Math.max(
      0,
      remainingValueBytes - new TextEncoder().encode(value).length,
    );
    return {
      label,
      value,
    };
  });
}

export function formatSystemLogDetails(log: NewSystemLog & { timestamp_unix_ms?: number }) {
  const sections = [
    `Severity: ${log.severity}`,
    `Source: ${log.source}`,
    `Title: ${log.title}`,
    `Message: ${log.message}`,
  ];
  if (log.timestamp_unix_ms !== undefined) {
    sections.unshift(`Timestamp: ${new Date(log.timestamp_unix_ms).toISOString()}`);
  }
  for (const detail of log.details) {
    sections.push(`${detail.label}: ${detail.value}`);
  }
  return sections.join("\n");
}

function errorDetails(error: unknown): SystemLogDetail[] {
  if (error === undefined || error === null) return [];
  if (error instanceof Error) {
    const details: SystemLogDetail[] = [
      { label: "Error type", value: error.name || "Error" },
      { label: "Error message", value: error.message },
    ];
    if (error.cause !== undefined) {
      details.push({ label: "Error cause", value: serializeUnknown(error.cause) });
    }
    if (error.stack) {
      details.push({ label: "Stack trace", value: error.stack });
    }
    return details;
  }
  return [{ label: "Original error", value: serializeUnknown(error) }];
}

function serializeUnknown(value: unknown): string {
  if (typeof value === "string") return value;
  if (value instanceof Error) {
    const cause: string =
      value.cause === undefined ? "" : `\nCaused by: ${serializeUnknown(value.cause)}`;
    return `${value.name || "Error"}: ${value.message}${cause}`;
  }
  try {
    const serialized = JSON.stringify(value, null, 2);
    return serialized === undefined ? String(value) : serialized;
  } catch {
    return String(value);
  }
}

function boundedRequiredText(value: string, maxBytes: number, fallback: string) {
  const trimmed = value.trim();
  return truncateUtf8(trimmed || fallback, maxBytes);
}

function truncateUtf8(value: string, maxBytes: number) {
  const encoded = new TextEncoder().encode(value.replaceAll("\0", ""));
  if (encoded.length <= maxBytes) return value.replaceAll("\0", "");
  const marker = "\n[truncated]";
  const markerBytes = new TextEncoder().encode(marker);
  if (maxBytes <= markerBytes.length) {
    return new TextDecoder("utf-8", { fatal: false })
      .decode(encoded.slice(0, maxBytes))
      .replace(/\uFFFD$/u, "");
  }
  const available = Math.max(0, maxBytes - markerBytes.length);
  const truncated = new TextDecoder("utf-8", { fatal: false }).decode(
    encoded.slice(0, available),
  );
  return `${truncated.replace(/\uFFFD$/u, "")}${marker}`;
}
