"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useState } from "react";

import { CheckHistory } from "@/components/check-history";
import { ResponseTimeChart } from "@/components/response-time-chart";
import { ApiError, api, type Monitor } from "@/lib/api";
import { formatInterval, formatLatency, formatPercent, formatTimestamp, percentile, plural } from "@/lib/format";
import { usePolling } from "@/lib/use-polling";

export function MonitorDetail({ id }: { id: number }) {
  const load = useCallback(() => api.getMonitor(id), [id]);
  const { data: monitor, error, refresh } = usePolling(load);
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [actionStatus, setActionStatus] = useState("");

  if (error instanceof ApiError && error.status === 404) {
    return (
      <section className="vbg-opening">
        <div className="vbg-opening-claim">
          <h1 className="vbg-title">This endpoint isn&apos;t monitored</h1>
          <p className="vbg-lede">
            It may have been deleted. <Link href="/">See all endpoints</Link>.
          </p>
        </div>
      </section>
    );
  }

  if (!monitor) {
    return (
      <section className="vbg-opening">
        <div className="vbg-opening-claim">
          <h1 className="vbg-title">{error ? "The API can't be reached" : "Loading endpoint"}</h1>
        </div>
      </section>
    );
  }

  const checks = monitor.recentChecks;
  const latest = checks[0];
  const latencies = checks.filter((check) => check.ok).map((check) => check.latencyMs);
  const p50 = percentile(latencies, 0.5);
  const p95 = percentile(latencies, 0.95);

  async function runAction(action: () => Promise<unknown>, message: string) {
    setBusy(true);
    try {
      await action();
      setActionStatus(message);
      await refresh();
    } catch (err) {
      setActionStatus(err instanceof Error ? err.message : "The request failed.");
    } finally {
      setBusy(false);
    }
  }

  async function deleteMonitor(name: string) {
    if (!window.confirm(`Delete ${name} and its check history?`)) {
      return;
    }
    setBusy(true);
    try {
      await api.deleteMonitor(id);
      router.push("/");
    } catch (err) {
      setActionStatus(err instanceof Error ? err.message : "The request failed.");
      setBusy(false);
    }
  }

  return (
    <>
      <section className="vbg-opening">
        <div className="vbg-opening-claim">
          <p className="vbg-meta">
            <Link href="/">All endpoints</Link>
          </p>
          <h1 className="vbg-title">{headline(monitor)}</h1>
        </div>
        <div className="vbg-opening-context">
          <div className="vbg-flow">
            <p className="vbg-lede">
              <span className="vbg-mono">{monitor.url}</span> is checked {formatInterval(monitor.intervalSeconds)}.{" "}
              {latest ? (
                <>
                  The latest check ran at <span className="vbg-mono">{formatTimestamp(latest.checkedAt)}</span>.
                </>
              ) : (
                "It hasn't been checked yet."
              )}
            </p>
            <div className="vbg-cluster">
              <button
                className="vbg-button"
                type="button"
                disabled={busy}
                onClick={() => runAction(() => api.checkNow(id), "Check queued. The result appears here after the worker runs it.")}
              >
                Check now
              </button>
              <button
                className="vbg-button"
                type="button"
                disabled={busy}
                onClick={() =>
                  runAction(
                    () => api.updateMonitor(id, { paused: !monitor.paused }),
                    monitor.paused ? "Scheduled checks resumed." : "Scheduled checks paused.",
                  )
                }
              >
                {monitor.paused ? "Resume checks" : "Pause checks"}
              </button>
              <button className="vbg-button" type="button" disabled={busy} onClick={() => deleteMonitor(monitor.name)}>
                Delete
              </button>
            </div>
            <p className="vbg-meta" role="status">
              {actionStatus}
            </p>
          </div>
        </div>
      </section>

      <section className="vbg-section" aria-label="Last 24 hours">
        <div className="vbg-stat-strip">
          <div className="vbg-stat">
            <p className="vbg-stat-label">Uptime</p>
            <p className="vbg-stat-value">{formatPercent(monitor.uptime24h)}</p>
            <p className="vbg-stat-detail">{plural(checks.length, "check")} in the last 24 hours</p>
          </div>
          <div className="vbg-stat">
            <p className="vbg-stat-label">Median response time</p>
            <p className="vbg-stat-value">{p50 === null ? "No data" : formatLatency(p50)}</p>
            <p className="vbg-stat-detail">Successful checks only</p>
          </div>
          <div className="vbg-stat">
            <p className="vbg-stat-label">95th percentile</p>
            <p className="vbg-stat-value">{p95 === null ? "No data" : formatLatency(p95)}</p>
            <p className="vbg-stat-detail">Successful checks only</p>
          </div>
        </div>
      </section>

      <section className="vbg-section">
        <ResponseTimeChart checks={checks} p95={p95} />
      </section>

      <section className="vbg-section" aria-labelledby="history-heading">
        <h2 className="vbg-heading-24" id="history-heading">
          Check history
        </h2>
        <CheckHistory checks={checks} />
      </section>
    </>
  );
}

function headline(monitor: Monitor): string {
  switch (monitor.status) {
    case "paused":
      return `${monitor.name} is paused`;
    case "pending":
      return `${monitor.name} hasn't been checked yet`;
    case "up":
      return `${monitor.name} is up`;
    case "down": {
      const firstSuccess = monitor.recentChecks.findIndex((check) => check.ok);
      const failures = firstSuccess === -1 ? monitor.recentChecks : monitor.recentChecks.slice(0, firstSuccess);
      return `${monitor.name} has been down since ${formatTimestamp(failures.at(-1)!.checkedAt)}`;
    }
  }
}
