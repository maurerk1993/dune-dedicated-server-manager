import { Flex } from "@radix-ui/themes";

import type { RemoteBattlegroupServerStat, RemoteServerStatus } from "../../types/server";
import { formatSnapshotAge, isSnapshotStale, totalPlayers } from "../../utils/dashboard";
import {
  formatMapPhase,
  formatReadiness,
  isMapReady,
  resolveMapStatus,
} from "../../utils/map-status";
import StatusPill from "../ui/StatusPill";
import BattlegroupControl from "./BattlegroupControl";
import DashboardMetricCard from "./DashboardMetricCard";

export type ServerMapsProps = {
  status?: RemoteServerStatus;
  statusError?: string;
  busyLabel?: string;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
};

export default function ServerMaps({
  status,
  statusError,
  busyLabel,
  onStartBattlegroup,
  onStopBattlegroup,
}: ServerMapsProps) {
  const rows = status?.battlegroup.serverStats ?? [];
  const readyMaps = rows.filter(isMapReady).length;
  const players = totalPlayers(rows);
  const stale = isSnapshotStale(status);
  const snapshotLabel = formatSnapshotAge(status?.collectedAt);

  return (
    <Flex direction="column" gap="4">
      <section className="dashboard-overview maps-overview" aria-label="Map overview">
        <DashboardMetricCard
          label="Loaded map servers"
          value={status ? rows.length.toString() : "—"}
          detail={
            !status
              ? "Awaiting server refresh"
              : rows.length === 1
                ? "One partition reporting"
                : `${rows.length} partitions reporting`
          }
          tone={status && rows.length > 0 ? "ok" : "gray"}
        />
        <DashboardMetricCard
          label="Ready maps"
          value={rows.length > 0 ? `${readyMaps} / ${rows.length}` : "—"}
          detail={rows.length > 0 ? "Operator and game runtime readiness" : "Awaiting map telemetry"}
          tone={rows.length === 0 ? "gray" : readyMaps === rows.length ? "ok" : "warn"}
        />
        <DashboardMetricCard
          label="Players online"
          value={players === null ? "—" : players.toString()}
          detail={players === null ? "Player counts are unavailable" : "Across all loaded partitions"}
          tone={players === null ? "gray" : "ok"}
        />
        <DashboardMetricCard
          label="Battlegroup uptime"
          value={status?.battlegroup.uptime || "—"}
          detail={`${snapshotLabel}${stale ? " · stale" : ""}`}
          tone={stale ? "warn" : status ? "ok" : "gray"}
        />
      </section>

      {statusError || stale ? (
        <section className="maps-snapshot-warning" data-tone={statusError ? "err" : "warn"}>
          <strong>{statusError ? "Latest refresh failed." : "This snapshot is stale."}</strong>{" "}
          {statusError
            ? "The last successful map data remains visible, but controls stay disabled until a refresh succeeds."
            : "Refresh before using lifecycle controls or relying on current map readiness."}
        </section>
      ) : null}

      <BattlegroupControl
        status={status}
        statusError={statusError}
        busyLabel={busyLabel}
        onStart={onStartBattlegroup}
        onStop={onStopBattlegroup}
      />

      <section className="dashboard-section">
        <div className="dashboard-section-heading">
          <div>
            <div className="dashboard-section-title">Active loaded maps</div>
            <div className="dashboard-section-subtitle">
              Live Funcom operator and game-runtime telemetry for each partition.
            </div>
          </div>
          <span className="dashboard-section-summary">{snapshotLabel}</span>
        </div>

        {rows.length > 0 ? (
          <div className="map-card-grid">
            {rows.map((row, index) => (
              <MapCard
                key={row.serverName || `${row.rawMap || row.map}-${row.partitionIndex ?? index}`}
                row={row}
              />
            ))}
          </div>
        ) : (
          <div className="dashboard-empty">
            No loaded map servers have reported yet. During a start, they will appear here as the
            Funcom operator creates each partition.
          </div>
        )}
      </section>

      <section className="dashboard-section maps-guide">
        <div className="dashboard-section-title">How to read map status</div>
        <div className="dashboard-section-subtitle">
          The main badge favors the game runtime&apos;s exact phase, so startup stages such as
          Initializing or Post Landscape Physics remain visible instead of being collapsed into
          Online or Offline. Operator phase and both readiness signals are shown separately below.
          Values update with the normal server refresh; this is a current snapshot, not history.
        </div>
      </section>
    </Flex>
  );
}

function MapCard({ row }: { row: RemoteBattlegroupServerStat }) {
  const state = resolveMapStatus(row);
  const identity = [row.rawMap, row.sietch].filter(Boolean).join(" · ");
  const ports = [
    row.gamePort !== null && row.gamePort !== undefined ? `Game ${row.gamePort}` : "",
    row.igwPort !== null && row.igwPort !== undefined ? `IGW ${row.igwPort}` : "",
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <article className="map-card" data-tone={state.tone}>
      <div className="map-card-rail" aria-hidden />
      <div className="map-card-content">
        <div className="map-card-heading">
          <div className="map-card-title-wrap">
            <h3 className="map-card-title">{row.map || "Game server"}</h3>
            <div className="map-card-identity">{identity || "Map identity not reported"}</div>
          </div>
          <StatusPill label={state.label} tone={state.tone} pulse={state.pulse} />
        </div>

        <div className="map-card-highlights">
          <MapDatum label="Players" value={row.players || "—"} prominent />
          <MapDatum
            label="Server FPS"
            value={
              row.simulationFps === null || row.simulationFps === undefined
                ? "—"
                : row.simulationFps.toFixed(1)
            }
            title="Server simulation frames per second (SFPS), as reported by the game runtime"
          />
          <MapDatum
            label="Partition"
            value={row.partitionIndex === null || row.partitionIndex === undefined ? "—" : row.partitionIndex.toString()}
          />
          <MapDatum label="Approx. age" value={row.age || "—"} title="Best available age; currently derived from battlegroup start time" />
        </div>

        <dl className="map-card-telemetry">
          <TelemetryTerm label="Game phase" value={formatMapPhase(row.gamePhase) || "Not reported"} />
          <TelemetryTerm label="Operator phase" value={formatMapPhase(row.phase) || "Not reported"} />
          <TelemetryTerm label="Runtime readiness" value={formatReadiness(row.runtimeReady)} />
          <TelemetryTerm label="Operator readiness" value={formatReadiness(row.ready)} />
          <TelemetryTerm
            label="Battlegroup role"
            value={
              row.battlegroupLeader === true
                ? "Leader"
                : row.battlegroupLeader === false
                  ? "Worker"
                  : "Not reported"
            }
          />
          <TelemetryTerm
            label="Map server restarts"
            value={row.restarts === null || row.restarts === undefined ? "Not reported" : row.restarts.toString()}
          />
          <TelemetryTerm
            label="Dimension"
            value={row.dimension === null || row.dimension === undefined ? "Not reported" : row.dimension.toString()}
          />
          <TelemetryTerm label="Ports" value={ports || "Not reported"} />
        </dl>

        {row.serverName ? (
          <div className="map-card-reporter" title={row.serverName}>
            Reporter: {row.serverName}
          </div>
        ) : null}
      </div>
    </article>
  );
}

function MapDatum({
  label,
  value,
  prominent = false,
  title,
}: {
  label: string;
  value: string;
  prominent?: boolean;
  title?: string;
}) {
  return (
    <div className="map-card-datum" data-prominent={prominent ? "true" : "false"} title={title}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function TelemetryTerm({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}
