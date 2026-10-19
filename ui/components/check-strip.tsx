import type { CheckResult } from "@/lib/api";
import { formatLatency, formatTimestamp } from "@/lib/format";

export const CHECK_STRIP_SLOTS = 30;
const SLOT_WIDTH = 6;
const MARK_WIDTH = 4;
const HEIGHT = 20;

/**
 * One mark per check, oldest on the left. Failed checks are full height so they stand
 * out without relying on color.
 */
export function CheckStrip({ checks }: { checks: CheckResult[] }) {
  const shown = checks.slice(0, CHECK_STRIP_SLOTS).reverse();
  const failed = shown.filter((check) => !check.ok).length;
  const firstSlot = CHECK_STRIP_SLOTS - shown.length;
  const label =
    shown.length === 0
      ? "No checks yet"
      : `${shown.length - failed} of ${shown.length} recent checks succeeded`;

  return (
    <svg
      className="vbg-custom-check-strip"
      viewBox={`0 0 ${CHECK_STRIP_SLOTS * SLOT_WIDTH} ${HEIGHT}`}
      role="img"
      aria-label={label}
    >
      {shown.map((check, index) => {
        const height = check.ok ? HEIGHT / 2 : HEIGHT;
        return (
          <rect
            key={check.id}
            className={check.ok ? "vbg-viz-check-ok" : "vbg-viz-check-failed"}
            x={(firstSlot + index) * SLOT_WIDTH}
            y={HEIGHT - height}
            width={MARK_WIDTH}
            height={height}
          >
            <title>
              {`${formatTimestamp(check.checkedAt)}: ${check.ok ? "up" : "down"}, ${formatLatency(check.latencyMs)}`}
            </title>
          </rect>
        );
      })}
    </svg>
  );
}
