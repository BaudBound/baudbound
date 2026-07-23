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
