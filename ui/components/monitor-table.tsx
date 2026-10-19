import Link from "next/link";

import { CheckStrip, CHECK_STRIP_SLOTS } from "@/components/check-strip";
import { StatusLabel } from "@/components/status-label";
import type { CheckResult, Monitor, MonitorStatus } from "@/lib/api";
import { formatPercent, formatResponseTime } from "@/lib/format";

const STATUS_ORDER: Record<MonitorStatus, number> = { down: 0, pending: 1, up: 2, paused: 3 };

export function MonitorTable({ monitors }: { monitors: Monitor[] }) {
  const sorted = [...monitors].sort(
    (a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || a.name.localeCompare(b.name),
  );

  return (
    <div className="vbg-table-wrap">
      <table>
        <caption>
          Endpoints that are down are listed first. Uptime covers the last 24 hours. Response
          time is from the latest check.
        </caption>
        <thead>
          <tr>
            <th scope="col">Endpoint</th>
            <th scope="col">Status</th>
            <th scope="col">Last {CHECK_STRIP_SLOTS} checks</th>
            <th scope="col" className="vbg-numeric">
              Uptime
            </th>
            <th scope="col" className="vbg-numeric">
              Response time
            </th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((monitor) => {
            const latest = monitor.recentChecks[0];
            return (
              <tr key={monitor.id}>
                <th scope="row">
                  <span className="vbg-custom-endpoint">
                    <Link href={`/monitors/${monitor.id}`}>{monitor.name}</Link>
                    <span className="vbg-meta">
                      <span className="vbg-mono">{monitor.url}</span>
                    </span>
                  </span>
                </th>
                <td>
                  <StatusLabel status={monitor.status} />
                </td>
                <td>
                  <CheckStrip checks={monitor.recentChecks} />
                </td>
                <td className="vbg-numeric">{formatPercent(monitor.uptime24h)}</td>
                <td className="vbg-numeric">{responseTime(latest)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function responseTime(latest: CheckResult | undefined): string {
  return latest ? formatResponseTime(latest) : "No data";
}
