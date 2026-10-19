"use client";

import { useState } from "react";

import { StatusLabel } from "@/components/status-label";
import type { CheckResult } from "@/lib/api";
import { formatCount, formatResponseTime, formatTimestamp, plural } from "@/lib/format";

const MAX_ROWS = 200;

/** Lists failed checks by default, with a control to show every check. */
export function CheckHistory({ checks }: { checks: CheckResult[] }) {
  const [showAll, setShowAll] = useState(false);
  const failed = checks.filter((check) => !check.ok);
  const rows = (showAll ? checks : failed).slice(0, MAX_ROWS);

  return (
    <div className="vbg-flow">
      <div className="vbg-cluster">
        <p>{summary(checks.length, failed.length, showAll)}</p>
        {checks.length > 0 && (
          <button className="vbg-button" type="button" onClick={() => setShowAll(!showAll)}>
            {showAll ? "Show failed checks only" : "Show all checks"}
          </button>
        )}
      </div>

      {rows.length > 0 && (
        <div className="vbg-table-wrap">
          <table>
            <caption>{showAll ? "Every check, newest first" : "Failed checks, newest first"}</caption>
            <thead>
              <tr>
                <th scope="col">Time</th>
                <th scope="col">Result</th>
                <th scope="col" className="vbg-numeric">
                  Response time
                </th>
                <th scope="col" className="vbg-numeric">
                  HTTP status
                </th>
                <th scope="col">Trigger</th>
                <th scope="col">Error</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((check) => (
                <tr key={check.id}>
                  <th scope="row">
                    <span className="vbg-mono vbg-custom-nowrap">{formatTimestamp(check.checkedAt)}</span>
                  </th>
                  <td>
                    <StatusLabel status={check.ok ? "up" : "down"} />
                  </td>
                  <td className="vbg-numeric">{formatResponseTime(check)}</td>
                  <td className="vbg-numeric">
                    {check.statusCode === null ? "None" : <span className="vbg-mono">{check.statusCode}</span>}
                  </td>
                  <td>{check.trigger === "manual" ? "Check now" : "Scheduled"}</td>
                  <td>{check.error ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function summary(total: number, failed: number, showAll: boolean): string {
  if (total === 0) {
    return "No checks have run in the last 24 hours.";
  }
  if (showAll) {
    return total > MAX_ROWS
      ? `Showing the newest ${MAX_ROWS} of ${formatCount(total)} checks from the last 24 hours.`
      : `Showing all ${plural(total, "check")} from the last 24 hours.`;
  }
  if (failed === 0) {
    return `None of the ${plural(total, "check")} from the last 24 hours failed.`;
  }
  return `Showing ${plural(Math.min(failed, MAX_ROWS), "failed check")} of ${formatCount(total)} from the last 24 hours.`;
}
