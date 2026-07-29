import type { RemoteBattlegroupServerStat } from "../../types/server";
import StatusPill, { type StatusTone } from "../ui/StatusPill";

export type ServerStatsTableProps = {
  rows: RemoteBattlegroupServerStat[];
};

function phaseTone(phase: string): StatusTone {
  const v = phase.trim().toLowerCase();
  if (["running", "ready", "healthy", "available", "reconciling"].includes(v)) return "ok";
  if (["pending", "starting", "deploying", "scheduling", "creating"].includes(v)) return "warn";
  if (["failed", "error", "crashloop", "crashloopbackoff", "unhealthy"].includes(v)) return "err";
  return "gray";
}

function friendlyServerState(phase: string, ready: string): { label: string; tone: StatusTone } {
  const tone = phaseTone(phase);
  const normalizedReady = ready.trim().toLowerCase();
  if (tone === "err" || ["false", "0", "no"].includes(normalizedReady)) {
    return { label: "Offline", tone: "err" };
  }
  if (tone === "warn") return { label: "Starting", tone: "warn" };
  if (tone === "ok" || ["true", "1", "yes"].includes(normalizedReady)) {
    return { label: "Online", tone: "ok" };
  }
  return { label: phase || "Unknown", tone: "gray" };
}

/**
 * Compact per-map game-server table parsed from the vendor `battlegroup
 * status` output. Mirrors the wrapper's "Game Servers" section.
 */
export default function ServerStatsTable({ rows }: ServerStatsTableProps) {
  if (rows.length === 0) return null;
  return (
    <div className="server-stats">
      <div className="server-stats-header">
        <span>Map</span>
        <span>Status</span>
        <span>Players</span>
        <span className="server-stats-cell-age">Age</span>
      </div>
      {rows.map((row, index) => {
        const state = friendlyServerState(row.phase, row.ready);
        return (
          <div
            key={`${row.map}-${row.age}-${index}`}
            className="server-stats-row"
            data-tone={state.tone}
          >
            <span className="server-stats-map">{row.map}</span>
            <span className="server-stats-cell-phase">
              <StatusPill label={state.label} tone={state.tone} pulse={state.tone === "warn"} />
            </span>
            <span className="server-stats-players">{row.players || "—"}</span>
            <span className="server-stats-cell-age">{row.age || "—"}</span>
          </div>
        );
      })}
    </div>
  );
}
