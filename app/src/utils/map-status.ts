import type { StatusTone } from "../components/ui/StatusPill";
import type { RemoteBattlegroupServerStat } from "../types/server";

const HEALTHY_PHASES = new Set(["running", "ready", "healthy", "available"]);
const STOPPED_PHASES = new Set(["stopped", "offline", "terminated", "shutdown"]);
const FAILURE_MARKERS = ["fail", "error", "crash", "unhealthy", "fatal"];

export type ResolvedMapStatus = {
  label: string;
  tone: StatusTone;
  pulse: boolean;
  ready: boolean;
};

export function formatMapPhase(value: string | null | undefined): string {
  const normalized = (value ?? "")
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ");
  if (!normalized) return "";
  return normalized
    .split(" ")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join(" ");
}

export function readinessValue(value: string | null | undefined): boolean | null {
  const normalized = (value ?? "").trim().toLowerCase();
  if (["true", "1", "yes", "ready"].includes(normalized)) return true;
  if (["false", "0", "no", "notready", "not ready"].includes(normalized)) return false;
  return null;
}

export function isMapReady(row: RemoteBattlegroupServerStat): boolean {
  const signals = [readinessValue(row.ready), readinessValue(row.runtimeReady)].filter(
    (value): value is boolean => value !== null,
  );
  if (signals.some((value) => !value)) return false;
  return signals.some(Boolean);
}

export function resolveMapStatus(row: RemoteBattlegroupServerStat): ResolvedMapStatus {
  const gamePhase = (row.gamePhase ?? "").trim();
  const operatorPhase = row.phase.trim();
  const rawPhases = [gamePhase, operatorPhase].filter(Boolean);
  const normalizedPhases = rawPhases.map((phase) => phase.toLowerCase().replace(/[\s_-]+/g, ""));
  const failureIndex = normalizedPhases.findIndex((phase) =>
    FAILURE_MARKERS.some((marker) => phase.includes(marker)),
  );
  const stoppedIndex = normalizedPhases.findIndex((phase) => STOPPED_PHASES.has(phase));
  const ready = isMapReady(row);

  if (failureIndex >= 0) {
    return {
      label: formatMapPhase(rawPhases[failureIndex]) || "Problem",
      tone: "err",
      pulse: false,
      ready,
    };
  }
  if (stoppedIndex >= 0) {
    return {
      label: formatMapPhase(rawPhases[stoppedIndex]) || "Stopped",
      tone: "gray",
      pulse: false,
      ready,
    };
  }

  const preferredPhase = gamePhase || operatorPhase;
  const normalizedPreferred = preferredPhase.toLowerCase().replace(/[\s_-]+/g, "");
  if (ready && (HEALTHY_PHASES.has(normalizedPreferred) || !preferredPhase)) {
    return {
      label: formatMapPhase(preferredPhase) || "Ready",
      tone: "ok",
      pulse: false,
      ready,
    };
  }
  return {
    label: formatMapPhase(preferredPhase) || "Waiting For Readiness",
    tone: "warn",
    pulse: true,
    ready,
  };
}

export function formatReadiness(value: string | null | undefined): string {
  const parsed = readinessValue(value);
  if (parsed === true) return "Ready";
  if (parsed === false) return "Not ready";
  return "Not reported";
}
