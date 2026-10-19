"use client";

import { api } from "@/lib/api";
import { formatTimeOfDay, plural } from "@/lib/format";
import { usePolling } from "@/lib/use-polling";

const loadWorkerStatus = () => api.getWorkerStatus();

/**
 * Shows whether the worker daemon is sending heartbeats. The heartbeat time is a
 * fixed-width timestamp so the sentence doesn't rewrap, and shift the page, on each poll.
 */
export function WorkerSummary() {
  const { data: worker } = usePolling(loadWorkerStatus);

  if (!worker) {
    return <p className="vbg-lede">Checking whether the worker is running.</p>;
  }

  if (!worker.online) {
    return (
      <p className="vbg-lede">
        No worker is running, so scheduled checks are paused. The worker runs as a Vercel Daemon,
        and daemons only run in production deployments.
      </p>
    );
  }

  const count = worker.instances.length;
  const lastSeen = worker.instances
    .map((instance) => instance.lastSeenAt)
    .sort()
    .at(-1)!;
  return (
    <p className="vbg-lede">
      {plural(count, "worker instance")} {count === 1 ? "is" : "are"} running as a Vercel Daemon.
      The latest heartbeat arrived at <span className="vbg-mono">{formatTimeOfDay(lastSeen)}</span>.
    </p>
  );
}
