import type {
  RemoteBattlegroupServerStat,
  RemoteHostMetrics,
  RemoteServerComponent,
  RemoteServerStatus,
} from "../types/server";
import type { StatusTone } from "../components/ui/StatusPill";

export const RESOURCE_WARNING_PERCENT = 80;
export const RESOURCE_CRITICAL_PERCENT = 90;
export const CPU_WARNING_PERCENT = 90;
export const SNAPSHOT_STALE_AFTER_MS = 5 * 60 * 1000;
const REQUIRED_SERVICE_KEYS = new Set([
  "database",
  "message-queue",
  "director",
  "gateway",
  "text-router",
]);

export type ServiceSummary = {
  groups: RemoteServerComponent[];
  healthyServices: number;
  totalServices: number;
  readyPods: number;
  totalPods: number;
  restarts: number;
};

export type DashboardAttention = {
  tone: "warn" | "err";
  message: string;
};

export function podServiceGroups(components: RemoteServerComponent[]): RemoteServerComponent[] {
  return components.filter(
    (component) => component.category === "system" && component.componentKind === "pod-group",
  );
}

export function summarizeServices(components: RemoteServerComponent[]): ServiceSummary {
  const groups = podServiceGroups(components);
  return {
    groups,
    healthyServices: groups.filter(
      (component) =>
        component.state.trim().toLowerCase() === "ready" &&
        component.readyPods === component.totalPods,
    ).length,
    totalServices: groups.length,
    readyPods: groups.reduce((sum, component) => sum + (component.readyPods ?? 0), 0),
    totalPods: groups.reduce((sum, component) => sum + (component.totalPods ?? 0), 0),
    restarts: groups.reduce((sum, component) => sum + component.restartCount, 0),
  };
}

export function isRequiredServiceUnavailable(component: RemoteServerComponent): boolean {
  return REQUIRED_SERVICE_KEYS.has(component.logKey) && (component.totalPods ?? 0) === 0;
}

export function totalPlayers(rows: RemoteBattlegroupServerStat[] | undefined): number | null {
  let observed = false;
  let total = 0;
  for (const row of rows ?? []) {
    if (!row.players.trim()) continue;
    const value = Number(row.players);
    if (!Number.isFinite(value) || value < 0) continue;
    observed = true;
    total += value;
  }
  return observed ? total : null;
}

export function usagePercent(used: number, total: number): number | null {
  if (!Number.isFinite(used) || !Number.isFinite(total) || total <= 0) return null;
  return Math.max(0, Math.min(100, (used / total) * 100));
}

export function resourceTone(percent: number | null): StatusTone {
  if (percent === null) return "gray";
  if (percent >= RESOURCE_CRITICAL_PERCENT) return "err";
  if (percent >= RESOURCE_WARNING_PERCENT) return "warn";
  return "ok";
}

export function cpuTone(percent: number | null | undefined): StatusTone {
  if (percent === null || percent === undefined) return "gray";
  return percent >= CPU_WARNING_PERCENT ? "warn" : "ok";
}

export function formatPercent(percent: number | null | undefined): string {
  return percent === null || percent === undefined ? "—" : `${Math.round(percent)}%`;
}

export function formatBytes(bytes: number | null | undefined, digits = 1): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unit;
  return `${value.toFixed(unit === 0 ? 0 : digits)} ${units[unit]}`;
}

export function formatUsage(used: number, total: number): string {
  if (!Number.isFinite(used) || !Number.isFinite(total) || total <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unit = Math.min(Math.floor(Math.log(total) / Math.log(1024)), units.length - 1);
  return `${(used / 1024 ** unit).toFixed(1)} / ${(total / 1024 ** unit).toFixed(1)} ${units[unit]}`;
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds < 0) {
    return "—";
  }
  const whole = Math.floor(seconds);
  const days = Math.floor(whole / 86400);
  const hours = Math.floor((whole % 86400) / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${whole}s`;
}

export function formatSnapshotAge(collectedAt: string | undefined, now = Date.now()): string {
  if (!collectedAt) return "Not refreshed";
  const timestamp = Date.parse(collectedAt);
  if (!Number.isFinite(timestamp)) return "Refresh time unavailable";
  const seconds = Math.max(0, Math.floor((now - timestamp) / 1000));
  if (seconds < 15) return "Updated just now";
  if (seconds < 60) return `Updated ${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `Updated ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Updated ${hours}h ago`;
  return `Updated ${Math.floor(hours / 24)}d ago`;
}

export function isSnapshotStale(
  status: RemoteServerStatus | undefined,
  now = Date.now(),
): boolean {
  if (!status?.collectedAt) return false;
  const timestamp = Date.parse(status.collectedAt);
  return Number.isFinite(timestamp) && now - timestamp > SNAPSHOT_STALE_AFTER_MS;
}

export function buildAttentionItems(
  status: RemoteServerStatus | undefined,
  components: RemoteServerComponent[],
  statusError?: string,
): DashboardAttention[] {
  const items: DashboardAttention[] = [];
  if (statusError) {
    items.push({
      tone: "err",
      message: `The latest refresh failed. Showing the last successful snapshot: ${statusError}`,
    });
  } else if (isSnapshotStale(status)) {
    items.push({
      tone: "warn",
      message: "This snapshot is more than five minutes old. Refresh before taking server actions.",
    });
  }

  const services = summarizeServices(components);
  const unavailable = services.groups.filter(
    (component) =>
      isRequiredServiceUnavailable(component) ||
      component.state.trim().toLowerCase() === "problem" ||
      (component.totalPods ?? 0) > (component.readyPods ?? 0),
  );
  if (unavailable.length > 0) {
    items.push({
      tone: "err",
      message: `${unavailable.map((component) => component.name).join(", ")} ${
        unavailable.length === 1 ? "needs" : "need"
      } attention.`,
    });
  }
  if (services.restarts > 0) {
    items.push({
      tone: "warn",
      message: `${services.restarts} container ${
        services.restarts === 1 ? "restart has" : "restarts have"
      } been recorded.`,
    });
  }

  addResourceAttention(items, status?.hostMetrics);
  return items;
}

function addResourceAttention(
  items: DashboardAttention[],
  metrics: RemoteHostMetrics | null | undefined,
): void {
  if (!metrics) return;
  const memory = usagePercent(metrics.memoryUsedBytes, metrics.memoryTotalBytes);
  const disk = usagePercent(metrics.diskUsedBytes, metrics.diskTotalBytes);
  if (memory !== null && memory >= RESOURCE_WARNING_PERCENT) {
    items.push({
      tone: memory >= RESOURCE_CRITICAL_PERCENT ? "err" : "warn",
      message: `Host memory is ${Math.round(memory)}% used.`,
    });
  }
  if (disk !== null && disk >= RESOURCE_WARNING_PERCENT) {
    items.push({
      tone: disk >= RESOURCE_CRITICAL_PERCENT ? "err" : "warn",
      message: `Dune storage is ${Math.round(disk)}% used.`,
    });
  }
}
