"use client";

import { useEffect, useRef, useState } from "react";

import type { CheckResult } from "@/lib/api";
import { formatClock, formatLatency, formatTimestamp, plural } from "@/lib/format";

const HEIGHT = 280;
const MARGIN = { top: 16, right: 104, bottom: 48, left: 72 };
const Y_TICKS = 4;
const NICE_STEPS = [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10];

interface ResponseTimeChartProps {
  /** Newest first, as the API returns them. */
  checks: CheckResult[];
  p95: number | null;
}

export function ResponseTimeChart({ checks, p95 }: ResponseTimeChartProps) {
  const [viewportRef, width] = useContainerWidth();

  return (
    <figure className="vbg-chart">
      <div className="vbg-chart-header">
        <h2 className="vbg-heading-20">Response time over the last 24 hours</h2>
      </div>
      <div className="vbg-chart-viewport" tabIndex={0} ref={viewportRef}>
        {checks.length === 0 ? (
          <p>The chart appears after the first check.</p>
        ) : (
          <ChartSvg checks={checks} p95={p95} width={width} />
        )}
      </div>
      <figcaption className="vbg-caption">
        The line connects successful checks. Marks below the axis are failed checks, and the
        dashed line is the 95th percentile. Times are in UTC.
      </figcaption>
    </figure>
  );
}

function ChartSvg({ checks, p95, width }: ResponseTimeChartProps & { width: number }) {
  const chronological = [...checks].reverse();
  const successful = chronological.filter((check) => check.ok);
  const failed = chronological.filter((check) => !check.ok);
  const plotWidth = width - MARGIN.left - MARGIN.right;
  const plotHeight = HEIGHT - MARGIN.top - MARGIN.bottom;
  const baseline = MARGIN.top + plotHeight;
  const first = chronological[0];
  const last = chronological.at(-1)!;

  const start = new Date(first.checkedAt).getTime();
  const span = Math.max(new Date(last.checkedAt).getTime() - start, 1);
  const x = (iso: string) => MARGIN.left + ((new Date(iso).getTime() - start) / span) * plotWidth;

  const yMax = niceCeil(Math.max(...successful.map((check) => check.latencyMs), p95 ?? 0, 1) * 1.05);
  const step = yMax / Y_TICKS;
  const y = (latencyMs: number) => baseline - (latencyMs / yMax) * plotHeight;

  const linePath = successful
    .map((check, index) => `${index === 0 ? "M" : "L"}${x(check.checkedAt).toFixed(1)},${y(check.latencyMs).toFixed(1)}`)
    .join(" ");

  const description =
    `Response time for ${plural(successful.length, "successful check")} between ` +
    `${formatTimestamp(first.checkedAt)} and ${formatTimestamp(last.checkedAt)}.` +
    (p95 !== null ? ` The 95th percentile is ${formatLatency(p95)}.` : "") +
    ` ${plural(failed.length, "check")} failed.`;

  return (
    <svg className="vbg-custom-response-chart" width={width} height={HEIGHT} viewBox={`0 0 ${width} ${HEIGHT}`} role="img" aria-label={description}>
      <g className="vbg-chart-axis">
        {Array.from({ length: Y_TICKS + 1 }, (_, tick) => (
          <g key={tick}>
            <line className="vbg-chart-gridline" x1={MARGIN.left} x2={MARGIN.left + plotWidth} y1={y(tick * step)} y2={y(tick * step)} />
            <text x={MARGIN.left - 8} y={y(tick * step) + 4} textAnchor="end">
              {formatLatency(tick * step)}
            </text>
          </g>
        ))}
        <text x={MARGIN.left} y={HEIGHT - 12}>
          {formatClock(first.checkedAt)}
        </text>
        <text x={MARGIN.left + plotWidth} y={HEIGHT - 12} textAnchor="end">
          {formatClock(last.checkedAt)}
        </text>
      </g>

      {successful.length > 1 && <path className="vbg-series-stroke vbg-series-1" d={linePath} fill="none" />}
      {successful.length === 1 && (
        <circle className="vbg-data-point vbg-series-1" cx={x(successful[0].checkedAt)} cy={y(successful[0].latencyMs)} r={3} />
      )}

      {p95 !== null && (
        <g>
          <line className="vbg-chart-annotation-line" x1={MARGIN.left} x2={MARGIN.left + plotWidth} y1={y(p95)} y2={y(p95)} />
          <text className="vbg-chart-direct-label" x={MARGIN.left + plotWidth + 8} y={y(p95) + 4}>
            {`p95 ${formatLatency(p95)}`}
          </text>
        </g>
      )}

      {failed.map((check) => (
        <rect key={check.id} className="vbg-viz-failure" x={x(check.checkedAt) - 1.5} y={baseline + 6} width={3} height={10}>
          <title>{`${formatTimestamp(check.checkedAt)}: ${check.error ?? "failed"}`}</title>
        </rect>
      ))}
    </svg>
  );
}

/** Tracks the element's width so the SVG draws at 1:1 scale and its labels keep their type size. */
function useContainerWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(720);

  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    const observer = new ResizeObserver(([entry]) => setWidth(Math.max(320, Math.floor(entry.contentRect.width))));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return [ref, width] as const;
}

/** Rounds up to a readable axis maximum such as 500, 600, or 800. */
function niceCeil(value: number): number {
  const magnitude = 10 ** Math.floor(Math.log10(value));
  const normalized = value / magnitude;
  return NICE_STEPS.find((step) => step >= normalized)! * magnitude;
}
