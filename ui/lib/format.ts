const percentFormat = new Intl.NumberFormat("en-US", {
  style: "percent",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const numberFormat = new Intl.NumberFormat("en-US");
const timestampFormat = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hourCycle: "h23",
  timeZone: "UTC",
});

const clockFormat = new Intl.DateTimeFormat("en-US", {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
  timeZone: "UTC",
});
const timeOfDayFormat = new Intl.DateTimeFormat("en-US", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hourCycle: "h23",
  timeZone: "UTC",
});

export function formatClock(iso: string): string {
  return `${clockFormat.format(new Date(iso))} UTC`;
}

/** Always the same length, such as "14:50:31 UTC", so text around it doesn't reflow when it updates. */
export function formatTimeOfDay(iso: string): string {
  return `${timeOfDayFormat.format(new Date(iso))} UTC`;
}

export function formatPercent(value: number | null): string {
  return value === null ? "No data" : percentFormat.format(value);
}

export function formatCount(value: number): string {
  return numberFormat.format(value);
}

export function formatLatency(milliseconds: number): string {
  return `${numberFormat.format(milliseconds)} ms`;
}

/** A check without a status code timed out or failed to connect, so its duration isn't a response time. */
export function formatResponseTime(check: { statusCode: number | null; latencyMs: number }): string {
  return check.statusCode === null ? "No response" : formatLatency(check.latencyMs);
}

export function formatTimestamp(iso: string): string {
  return `${timestampFormat.format(new Date(iso))} UTC`;
}

export function formatInterval(seconds: number): string {
  if (seconds % 60 === 0 && seconds >= 120) {
    return `every ${seconds / 60} minutes`;
  }
  return `every ${seconds} seconds`;
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${formatCount(count)} ${count === 1 ? singular : pluralForm}`;
}

/** Nearest-rank percentile of the given values, with p between 0 and 1. */
export function percentile(values: number[], p: number): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil(p * sorted.length));
  return sorted[rank - 1];
}
