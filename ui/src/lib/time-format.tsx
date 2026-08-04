import { createContext, type ReactNode, useContext, useMemo } from "react";

import type { TimeFormat } from "@/lib/runner-api";

export type DesktopTimeFormatter = ReturnType<typeof createDesktopTimeFormatter>;

const defaultFormatter = createDesktopTimeFormatter("24-hour");
const DesktopTimeContext = createContext<DesktopTimeFormatter>(defaultFormatter);

export function DesktopTimeProvider({
  children,
  timeFormat,
}: {
  children: ReactNode;
  timeFormat: TimeFormat;
}) {
  const formatter = useMemo(() => createDesktopTimeFormatter(timeFormat), [timeFormat]);
  return <DesktopTimeContext.Provider value={formatter}>{children}</DesktopTimeContext.Provider>;
}

export function useDesktopTime() {
  return useContext(DesktopTimeContext);
}

export function createDesktopTimeFormatter(
  timeFormat: TimeFormat,
  options: { locale?: string; timeZone?: string } = {},
) {
  const hour12 = timeFormat === "12-hour";
  const sharedOptions = {
    hour12,
    timeZone: options.timeZone,
  } satisfies Intl.DateTimeFormatOptions;
  const dateTimeFormatter = new Intl.DateTimeFormat(options.locale, {
    ...sharedOptions,
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const timeFormatter = new Intl.DateTimeFormat(options.locale, {
    ...sharedOptions,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return {
    formatDateTime: (date: Date) => dateTimeFormatter.format(date),
    formatTime: (date: Date) => timeFormatter.format(date),
    formatUnixMilliseconds: (value: number) => dateTimeFormatter.format(new Date(value)),
    formatUnixSeconds: (value: number) => dateTimeFormatter.format(new Date(value * 1_000)),
    timeFormat,
  };
}

export function datetimeInTimeZoneToIso(value: string, timeZone: string) {
  const parts = parseDatetimeLocalValue(value);
  if (!parts) return null;
  if (timeZone === "__local__") {
    const date = new Date(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute, parts.second);
    return datePartsMatch(date, parts, true) ? date.toISOString() : null;
  }
  const requestedUtc = datePartsToUtc(parts);
  let candidate = requestedUtc;
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const displayed = parseDatetimeLocalValue(formatDateParts(new Date(candidate), timeZone));
    if (!displayed) return null;
    const correction = requestedUtc - datePartsToUtc(displayed);
    if (correction === 0) return new Date(candidate).toISOString();
    candidate += correction;
  }
  return formatDateParts(new Date(candidate), timeZone) === value ? new Date(candidate).toISOString() : null;
}

type DatetimeParts = {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
};

function parseDatetimeLocalValue(value: string): DatetimeParts | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const parts = {
    year: Number(match[1]),
    month: Number(match[2]),
    day: Number(match[3]),
    hour: Number(match[4]),
    minute: Number(match[5]),
    second: Number(match[6]),
  };
  return datePartsMatch(new Date(datePartsToUtc(parts)), parts, false) ? parts : null;
}

function datePartsMatch(date: Date, parts: DatetimeParts, local: boolean) {
  return local
    ? date.getFullYear() === parts.year &&
        date.getMonth() + 1 === parts.month &&
        date.getDate() === parts.day &&
        date.getHours() === parts.hour &&
        date.getMinutes() === parts.minute &&
        date.getSeconds() === parts.second
    : date.getUTCFullYear() === parts.year &&
        date.getUTCMonth() + 1 === parts.month &&
        date.getUTCDate() === parts.day &&
        date.getUTCHours() === parts.hour &&
        date.getUTCMinutes() === parts.minute &&
        date.getUTCSeconds() === parts.second;
}

function datePartsToUtc(parts: DatetimeParts) {
  return Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute, parts.second);
}

function formatDateParts(date: Date, timeZone: string) {
  try {
    const values = Object.fromEntries(
      new Intl.DateTimeFormat("en-CA", {
        day: "2-digit",
        hour: "2-digit",
        hourCycle: "h23",
        minute: "2-digit",
        month: "2-digit",
        second: "2-digit",
        timeZone,
        year: "numeric",
      })
        .formatToParts(date)
        .filter((part) => part.type !== "literal")
        .map((part) => [part.type, part.value]),
    );
    return values.year + "-" + values.month + "-" + values.day + "T" + values.hour + ":" + values.minute + ":" + values.second;
  } catch {
    return "";
  }
}
