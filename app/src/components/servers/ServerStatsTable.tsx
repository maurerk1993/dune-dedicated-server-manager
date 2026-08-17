import type { RemoteBattlegroupServerStat } from "../../types/server";
import { resolveMapStatus } from "../../utils/map-status";
import StatusPill from "../ui/StatusPill";

export type ServerStatsTableProps = {
  rows: RemoteBattlegroupServerStat[];
};

/**
 * Compact per-map game-server table merged from the vendor BattleGroup and
 * ServerStats resources. The Maps tab presents the full telemetry payload.
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
        const state = resolveMapStatus(row);
        return (
          <div
            key={`${row.map}-${row.age}-${index}`}
            className="server-stats-row"
            data-tone={state.tone}
          >
            <span className="server-stats-map">{row.map}</span>
            <span className="server-stats-cell-phase">
              <StatusPill label={state.label} tone={state.tone} pulse={state.pulse} />
            </span>
            <span className="server-stats-players">{row.players || "—"}</span>
            <span className="server-stats-cell-age">{row.age || "—"}</span>
          </div>
        );
      })}
    </div>
  );
}
