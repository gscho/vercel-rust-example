import type { MonitorStatus } from "@/lib/api";

const LABELS: Record<MonitorStatus, string> = {
  up: "Up",
  down: "Down",
  pending: "Not checked yet",
  paused: "Paused",
};

export function StatusLabel({ status }: { status: MonitorStatus }) {
  return (
    <span className="vbg-custom-status" data-status={status}>
      <span className="vbg-custom-status-mark" aria-hidden="true" />
      {LABELS[status]}
    </span>
  );
}
