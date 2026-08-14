import { useState } from "react";
import { Flex } from "@radix-ui/themes";
import { Cross2Icon } from "@radix-ui/react-icons";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import type { CustomTunnelStartRequest, ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import {
  attentionFingerprint,
  buildAttentionItems,
  cpuTone,
  formatDuration,
  formatPercent,
  formatSnapshotAge,
  formatUsage,
  isSnapshotStale,
  resourceTone,
  summarizeServices,
  totalPlayers,
  usagePercent,
} from "../../utils/dashboard";
import {
  readDashboardAttentionDismissal,
  writeDashboardAttentionDismissal,
} from "../../services/storage";
import {
  isBattlegroupStarted,
  isDirectorReadyPhase,
  phaseTone,
  remoteServerDefaultUser,
} from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import Metric from "../ui/Metric";
import ServerStatsTable from "./ServerStatsTable";
import ServerTunnelControls from "./ServerTunnelControls";
import CustomTunnelControls from "./CustomTunnelControls";
import ManagementServiceCard from "../management/ManagementServiceCard";
import type { ManagementStatusState } from "../management/useManagementStatus";
import type { LogRow } from "../../types/log";
import DashboardMetricCard from "./DashboardMetricCard";
import ServerServiceGrid from "./ServerServiceGrid";

export type ServerDashboardProps = {
  server: RemoteServerRecord;
  status?: RemoteServerStatus;
  statusError?: string;
  components: RemoteServerComponent[];
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  managementStatus: ManagementStatusState;
  onRefreshManagement: () => Promise<void>;
  appendLogRow: (row: LogRow) => void;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
  onRestartBattlegroup: () => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStartCustomTunnel: (request: CustomTunnelStartRequest, name: string) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
};

/**
 * Per-server Dashboard sub-tab: status hero metrics, per-map server-stats
 * table, lifecycle action row (start/stop/restart), and tunnel controls.
 */
export default function ServerDashboard({
  server,
  status,
  statusError,
  components,
  busyLabel,
  tunnels,
  tunnelBusy,
  managementStatus,
  onRefreshManagement,
  appendLogRow,
  onStartBattlegroup,
  onStopBattlegroup,
  onRestartBattlegroup,
  onStartTunnel,
  onStartCustomTunnel,
  onStopTunnel,
  onOpenTunnel,
}: ServerDashboardProps) {
  const stale = isSnapshotStale(status);
  const actionableStatus = statusError || stale ? undefined : status;
  const battlegroup = status?.battlegroup;
  const battlegroupStarted = actionableStatus
    ? isBattlegroupStarted(actionableStatus.battlegroup)
    : false;
  const battlegroupStartRequested = actionableStatus
    ? !actionableStatus.battlegroup.stop
    : !!battlegroup && !battlegroup.stop;
  const battlegroupStopped = actionableStatus
    ? actionableStatus.battlegroup.stop
    : !!battlegroup?.stop;
  const directorReady =
    !!actionableStatus &&
    isDirectorReadyPhase(actionableStatus.battlegroup.directorPhase);
  const busy = !!busyLabel;
  const metrics = status?.hostMetrics;
  const players = totalPlayers(battlegroup?.serverStats);
  const services = summarizeServices(components);
  const memoryPercent = metrics
    ? usagePercent(metrics.memoryUsedBytes, metrics.memoryTotalBytes)
    : null;
  const diskPercent = metrics
    ? usagePercent(metrics.diskUsedBytes, metrics.diskTotalBytes)
    : null;
  const attention = buildAttentionItems(status, components, statusError);
  const currentAttentionFingerprint = attentionFingerprint(attention);
  const [dismissedAttentionByServer, setDismissedAttentionByServer] = useState<
    Record<string, string>
  >({});
  const dismissedAttentionFingerprint =
    dismissedAttentionByServer[server.id] ?? readDashboardAttentionDismissal(server.id);
  const showAttention =
    attention.length > 0 && dismissedAttentionFingerprint !== currentAttentionFingerprint;
  const snapshotLabel = formatSnapshotAge(status?.collectedAt);

  return (
    <Flex direction="column" gap="4">
      <section className="dashboard-overview" aria-label="Server overview">
        <DashboardMetricCard
          label="Players online"
          value={players === null ? "—" : players.toString()}
          detail={
            players === null
              ? "Player counts are not available"
              : `${battlegroup?.serverStats?.length ?? 0} map ${
                  battlegroup?.serverStats?.length === 1 ? "partition" : "partitions"
                } reporting`
          }
          tone={players === null ? "gray" : "ok"}
        />
        <DashboardMetricCard
          label="Battlegroup services"
          value={
            services.totalServices > 0
              ? `${services.healthyServices} / ${services.totalServices}`
              : "—"
          }
          detail={
            services.totalPods > 0
              ? `${services.readyPods} of ${services.totalPods} pods ready`
              : "Service health is not available"
          }
          tone={
            services.totalServices === 0
              ? "gray"
              : services.healthyServices === services.totalServices
                ? "ok"
                : "err"
          }
        />
        <DashboardMetricCard
          label="Host memory"
          value={
            metrics
              ? formatUsage(metrics.memoryUsedBytes, metrics.memoryTotalBytes)
              : "Unavailable"
          }
          detail={
            metrics
              ? `${formatPercent(memoryPercent)} used${
                  metrics.swapTotalBytes > 0
                    ? ` · ${formatUsage(metrics.swapUsedBytes, metrics.swapTotalBytes)} swap`
                    : " · no swap configured"
                }`
              : "Refresh could not read Linux memory data"
          }
          tone={resourceTone(memoryPercent)}
          progress={memoryPercent}
        />
        <DashboardMetricCard
          label="CPU now"
          value={formatPercent(metrics?.cpuUsagePercent)}
          detail={
            metrics?.loadAverageOne !== null && metrics?.loadAverageOne !== undefined
              ? `1-minute load ${metrics.loadAverageOne.toFixed(2)}`
              : "Current CPU load is unavailable"
          }
          tone={cpuTone(metrics?.cpuUsagePercent)}
          progress={metrics?.cpuUsagePercent}
        />
        <DashboardMetricCard
          label="Dune storage"
          value={
            metrics
              ? formatUsage(metrics.diskUsedBytes, metrics.diskTotalBytes)
              : "Unavailable"
          }
          detail={metrics ? `${formatPercent(diskPercent)} used` : "Storage data is unavailable"}
          tone={resourceTone(diskPercent)}
          progress={diskPercent}
        />
        <DashboardMetricCard
          label="Host uptime"
          value={formatDuration(metrics?.uptimeSeconds)}
          detail={`${snapshotLabel}${stale ? " · stale" : ""}`}
          tone={stale ? "warn" : metrics ? "ok" : "gray"}
        />
      </section>

      {showAttention ? (
        <section className="dashboard-attention" aria-label="Needs attention">
          <div className="dashboard-attention-heading">
            <div className="dashboard-attention-title">Needs attention</div>
            <button
              type="button"
              className="dashboard-attention-dismiss"
              aria-label="Dismiss current attention notices"
              title="Dismiss until these notices change"
              onClick={() => {
                writeDashboardAttentionDismissal(server.id, currentAttentionFingerprint);
                setDismissedAttentionByServer((dismissed) => ({
                  ...dismissed,
                  [server.id]: currentAttentionFingerprint,
                }));
              }}
            >
              <Cross2Icon aria-hidden />
              Dismiss
            </button>
          </div>
          <div className="dashboard-attention-list">
            {attention.map((item, index) => (
              <div
                className="dashboard-attention-item"
                data-tone={item.tone}
                key={`${item.message}-${index}`}
              >
                <span aria-hidden />
                <div>{item.message}</div>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="dashboard-section">
        <div className="dashboard-section-heading">
          <div>
            <div className="dashboard-section-title">Battlegroup services</div>
            <div className="dashboard-section-subtitle">
              The Kubernetes pods that keep this Dune battlegroup running.
            </div>
          </div>
          <span className="dashboard-section-summary">
            {services.totalPods > 0
              ? `${services.readyPods}/${services.totalPods} ready`
              : "Awaiting refresh"}
          </span>
        </div>
        <ServerServiceGrid components={components} />
      </section>

      <section className="dashboard-section">
        <div className="dashboard-section-heading">
          <div>
            <div className="dashboard-section-title">Maps and players</div>
            <div className="dashboard-section-subtitle">
              Current game-server readiness and connected players by partition.
            </div>
          </div>
          <span className="dashboard-section-summary">
            {players === null ? "Player count unavailable" : `${players} online`}
          </span>
        </div>
        {battlegroup?.serverStats && battlegroup.serverStats.length > 0 ? (
          <ServerStatsTable rows={battlegroup.serverStats} />
        ) : (
          <div className="dashboard-empty">No map information has been reported yet.</div>
        )}
      </section>

      <section className="dashboard-section dashboard-control-section">
        <div>
          <div className="dashboard-section-title">Battlegroup control</div>
          <div className="dashboard-section-subtitle">
            {actionableStatus
              ? "Lifecycle actions use the latest verified snapshot."
              : "Refresh the server before lifecycle actions are available."}
          </div>
        </div>
        <div className="action-row">
          {battlegroupStopped || !actionableStatus ? (
            <ActionButton
              onClick={onStartBattlegroup}
              busy={busy && !battlegroupStarted}
              disabled={busy || !actionableStatus || !battlegroupStopped}
              tone="accent"
              pendingLabel="Starting"
            >
              Start BattleGroup
            </ActionButton>
          ) : null}
          {battlegroupStartRequested ? (
            <>
              <ActionButton
                onClick={onRestartBattlegroup}
                busy={busy}
                disabled={busy || !actionableStatus}
                tone="default"
                pendingLabel="Restarting"
              >
                Restart
              </ActionButton>
              <ActionButton
                onClick={onStopBattlegroup}
                busy={busy && battlegroupStartRequested && !battlegroupStopped}
                disabled={busy || !actionableStatus}
                tone="danger"
                pendingLabel="Stopping"
              >
                Stop BattleGroup
              </ActionButton>
            </>
          ) : null}
        </div>
      </section>

      <section className="dashboard-section dashboard-technical-section">
        <div className="dashboard-section-heading">
          <div>
            <div className="dashboard-section-title">Technical details</div>
            <div className="dashboard-section-subtitle">
              Deployment identifiers and operator-reported phases.
            </div>
          </div>
          <span className="dashboard-section-summary">{snapshotLabel}</span>
        </div>
        <div className="metric-grid">
          <Metric label="Namespace" value={server.namespace || ""} />
          <Metric label="BattleGroup" value={server.battlegroupName || ""} />
          <Metric
            label="Database"
            value={battlegroup?.databasePhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.databasePhase ?? "") : "muted"}
          />
          <Metric
            label="Gateway"
            value={battlegroup?.serverGroupPhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.serverGroupPhase) : "muted"}
          />
          <Metric
            label="Director"
            value={battlegroup?.directorPhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.directorPhase) : "muted"}
          />
          <Metric label="Battlegroup uptime" value={battlegroup?.uptime ?? ""} />
          <Metric label="Steam build" value={status?.package.installedBuildId ?? ""} />
          <Metric
            label="Running image"
            value={status?.package.liveBattlegroupVersion ?? ""}
          />
        </div>
      </section>

      <section className="dashboard-section">
        <div className="dashboard-section-title">Management service</div>
        <div className="dashboard-section-subtitle">
          Status and maintenance for the optional Dune management daemon.
        </div>
        <ManagementServiceCard
          server={server}
          status={managementStatus}
          onRefresh={onRefreshManagement}
          appendLogRow={appendLogRow}
        />
      </section>

      <section className="dashboard-section">
        <div className="dashboard-section-title">Connections</div>
        <div className="dashboard-section-subtitle">
          Secure local tunnels to battlegroup tools and custom services.
        </div>
        <ServerTunnelControls
          serverKey={server.id}
          namespace={server.namespace}
          host={server.host}
          serverKind={server.type}
          user={server.user || remoteServerDefaultUser(server.type)}
          keyPath={server.keyPath}
          port={server.port}
          canStartDirectorTunnel={
            !!actionableStatus && !actionableStatus.battlegroup.stop && directorReady
          }
          canStartFileBrowserTunnel={
            !!actionableStatus && !actionableStatus.battlegroup.stop
          }
          canStartDatabaseTunnel={
            !!actionableStatus && !actionableStatus.battlegroup.stop
          }
          canStartPgHeroTunnel={
            !!actionableStatus && !actionableStatus.battlegroup.stop
          }
          tunnels={tunnels}
          tunnelBusy={tunnelBusy}
          onStartTunnel={onStartTunnel}
          onStopTunnel={onStopTunnel}
          onOpenTunnel={onOpenTunnel}
        />
        <CustomTunnelControls
          key={server.id}
          serverKey={server.id}
          host={server.host}
          serverKind={server.type}
          user={server.user || remoteServerDefaultUser(server.type)}
          keyPath={server.keyPath}
          port={server.port}
          tunnels={tunnels}
          tunnelBusy={tunnelBusy}
          onStartCustomTunnel={onStartCustomTunnel}
          onStopTunnel={onStopTunnel}
          onOpenTunnel={onOpenTunnel}
        />
      </section>
    </Flex>
  );
}
