export type MonitorStatus = "up" | "down" | "pending" | "paused";

export interface CheckResult {
  id: number;
  monitorId: number;
  trigger: "scheduled" | "manual";
  checkedAt: string;
  ok: boolean;
  statusCode: number | null;
  latencyMs: number;
  error: string | null;
}

export interface Monitor {
  id: number;
  name: string;
  url: string;
  intervalSeconds: number;
  paused: boolean;
  nextCheckAt: string;
  createdAt: string;
  updatedAt: string;
  status: MonitorStatus;
  uptime24h: number | null;
  /** Newest first. The list endpoint returns up to 30, the detail endpoint the last 24 hours. */
  recentChecks: CheckResult[];
}

export interface WorkerInstance {
  instanceId: string;
  deploymentId: string;
  startedAt: string;
  lastSeenAt: string;
}

export interface WorkerStatus {
  online: boolean;
  instances: WorkerInstance[];
}

export interface MonitorInput {
  name?: string;
  url?: string;
  intervalSeconds?: number;
  paused?: boolean;
}

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

/**
 * Calls the Rust API through relative /api paths, so requests stay on the same domain and
 * carry the Deployment Protection cookie.
 */
export class MonitorsApi {
  constructor(private readonly baseUrl = "/api") {}

  async listMonitors(): Promise<Monitor[]> {
    const body = await this.request<{ monitors: Monitor[] }>("/monitors");
    return body.monitors;
  }

  getMonitor(id: number): Promise<Monitor> {
    return this.request(`/monitors/${id}`);
  }

  createMonitor(input: MonitorInput): Promise<Monitor> {
    return this.request("/monitors", { method: "POST", body: JSON.stringify(input) });
  }

  updateMonitor(id: number, input: MonitorInput): Promise<Monitor> {
    return this.request(`/monitors/${id}`, { method: "PATCH", body: JSON.stringify(input) });
  }

  deleteMonitor(id: number): Promise<void> {
    return this.request(`/monitors/${id}`, { method: "DELETE" });
  }

  checkNow(id: number): Promise<void> {
    return this.request(`/monitors/${id}/check`, { method: "POST" });
  }

  getWorkerStatus(): Promise<WorkerStatus> {
    return this.request("/worker");
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(this.baseUrl + path, {
      ...init,
      headers: { "Content-Type": "application/json" },
      cache: "no-store",
    });
    if (!response.ok) {
      const body = await response.json().catch(() => null);
      throw new ApiError(body?.error ?? `Request failed with status ${response.status}`, response.status);
    }
    if (response.status === 204) {
      return undefined as T;
    }
    return response.json();
  }
}

export const api = new MonitorsApi();
