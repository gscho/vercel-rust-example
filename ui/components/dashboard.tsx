"use client";

import { AddMonitorForm } from "@/components/add-monitor-form";
import { MonitorTable } from "@/components/monitor-table";
import { WorkerSummary } from "@/components/worker-summary";
import { api, type Monitor } from "@/lib/api";
import { plural } from "@/lib/format";
import { usePolling } from "@/lib/use-polling";

const loadMonitors = () => api.listMonitors();

export function Dashboard() {
  const { data: monitors, error, refresh } = usePolling(loadMonitors);

  return (
    <>
      <section className="vbg-opening">
        <div className="vbg-opening-claim">
          <h1 className="vbg-title">{headline(monitors, error)}</h1>
        </div>
        <div className="vbg-opening-context">
          <WorkerSummary />
        </div>
      </section>

      {monitors && monitors.length > 0 && (
        <section className="vbg-section" aria-label="Monitored endpoints">
          <MonitorTable monitors={monitors} />
        </section>
      )}

      <section className="vbg-section" aria-labelledby="add-heading">
        <div className="vbg-flow">
          <h2 className="vbg-heading-24" id="add-heading">
            Monitor another endpoint
          </h2>
          <p>The worker requests the URL on the interval you choose and treats any status below 400 as up.</p>
        </div>
        <AddMonitorForm onCreated={refresh} />
      </section>
    </>
  );
}

function headline(monitors: Monitor[] | undefined, error: Error | undefined): string {
  if (!monitors) {
    return error ? "The API can't be reached" : "Loading endpoint status";
  }

  const active = monitors.filter((monitor) => monitor.status !== "paused");
  const down = active.filter((monitor) => monitor.status === "down").length;
  const pending = active.filter((monitor) => monitor.status === "pending").length;

  if (monitors.length === 0) {
    return "No endpoints are monitored yet";
  }
  if (active.length === 0) {
    return "Every endpoint is paused";
  }
  if (down > 0) {
    return `${down} of ${plural(active.length, "endpoint")} ${down === 1 ? "is" : "are"} down`;
  }
  if (pending > 0) {
    return `Waiting for the first check of ${plural(pending, "endpoint")}`;
  }
  return active.length === 1 ? "The endpoint is up" : `All ${active.length} endpoints are up`;
}
