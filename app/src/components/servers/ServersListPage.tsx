import { Box, Flex, Heading, Text } from "@radix-ui/themes";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import {
  formatPercent,
  formatSnapshotAge,
  formatUsage,
  isSnapshotStale,
  summarizeServices,
  totalPlayers,
  usagePercent,
} from "../../utils/dashboard";
import { remoteServerDefaultUser, resolveServerStatus } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import EmptyState from "../ui/EmptyState";
import StatusPill from "../ui/StatusPill";

export type ServersListPageProps = {
  servers: RemoteServerRecord[];
  statuses: Record<string, RemoteServerStatus>;
  components: Record<string, RemoteServerComponent[]>;
  statusErrors: Record<string, string>;
  busyMap: Record<string, string>;
  onOpenServer: (serverId: string) => void;
  onAddServer: () => void;
  onRefreshAll: () => void;
};

export default function ServersListPage({
  servers,
  statuses,
  components,
  statusErrors,
  busyMap,
  onOpenServer,
  onAddServer,
  onRefreshAll,
}: ServersListPageProps) {
  const refreshing = Object.keys(busyMap).length > 0;
  return (
    <Box className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0" p="4">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="6" className="h-display">
              Servers
            </Heading>
            <Text as="p" size="2" mt="1" style={{ color: "var(--color-text-muted)" }}>
              Attached remote Dune battlegroups. Click a row to open its console.
            </Text>
          </Box>
          <Flex gap="2" wrap="wrap" justify="end">
            {servers.length > 0 ? (
              <ActionButton
                onClick={onRefreshAll}
                busy={refreshing}
                disabled={refreshing}
                pendingLabel="Refreshing"
              >
                Refresh all
              </ActionButton>
            ) : null}
            <ActionButton onClick={onAddServer} tone="accent">
              + Add server
            </ActionButton>
          </Flex>
        </Flex>
        <Box className="page-scroll">
          {servers.length > 0 ? (
            <div className="server-list">
              {servers.map((server, index) => {
                const status = statuses[server.id];
                const stale = isSnapshotStale(status);
                const baseResolved = resolveServerStatus(
                  statusErrors[server.id],
                  status,
                  !!busyMap[server.id],
                  server,
                );
                const resolved =
                  stale && !statusErrors[server.id]
                    ? { tone: "warn" as const, label: "Stale snapshot", pulse: false }
                    : baseResolved;
                const userName = server.user || remoteServerDefaultUser(server.type);
                const serviceSummary = summarizeServices(components[server.id] ?? []);
                const players = totalPlayers(status?.battlegroup.serverStats);
                const memory = status?.hostMetrics
                  ? usagePercent(
                      status.hostMetrics.memoryUsedBytes,
                      status.hostMetrics.memoryTotalBytes,
                    )
                  : null;
                const disk = status?.hostMetrics
                  ? usagePercent(
                      status.hostMetrics.diskUsedBytes,
                      status.hostMetrics.diskTotalBytes,
                    )
                  : null;
                return (
                  <button
                    key={server.id}
                    type="button"
                    className="server-summary-card"
                    data-tone={resolved.tone}
                    style={{ animationDelay: `${index * 30}ms` }}
                    onClick={() => onOpenServer(server.id)}
                  >
                    <span className="server-summary-rail" />
                    <span className="server-summary-content">
                      <span className="server-summary-heading">
                        <span className="server-row-content">
                          <span className="server-row-name">{server.name}</span>
                          <span className="server-row-host">
                            {userName}@{server.host}
                            {server.battlegroupName ? ` · ${server.battlegroupName}` : ""}
                          </span>
                        </span>
                        <StatusPill
                          label={resolved.label}
                          tone={resolved.tone}
                          pulse={resolved.pulse}
                        />
                      </span>
                      <span className="server-summary-stats">
                        <span>
                          <small>Players</small>
                          <strong>{players === null ? "—" : players}</strong>
                        </span>
                        <span>
                          <small>Services</small>
                          <strong>
                            {serviceSummary.totalServices > 0
                              ? `${serviceSummary.healthyServices}/${serviceSummary.totalServices}`
                              : "—"}
                          </strong>
                        </span>
                        <span
                          title={
                            status?.hostMetrics
                              ? formatUsage(
                                  status.hostMetrics.memoryUsedBytes,
                                  status.hostMetrics.memoryTotalBytes,
                                )
                              : undefined
                          }
                        >
                          <small>RAM</small>
                          <strong>{formatPercent(memory)}</strong>
                        </span>
                        <span
                          title={
                            status?.hostMetrics
                              ? formatUsage(
                                  status.hostMetrics.diskUsedBytes,
                                  status.hostMetrics.diskTotalBytes,
                                )
                              : undefined
                          }
                        >
                          <small>Disk</small>
                          <strong>{formatPercent(disk)}</strong>
                        </span>
                      </span>
                      <span className="server-summary-footer">
                        <span>{formatSnapshotAge(status?.collectedAt)}</span>
                        {statusErrors[server.id] ? (
                          <span className="server-summary-error">Latest check failed</span>
                        ) : serviceSummary.restarts > 0 ? (
                          <span className="server-summary-warning">
                            {serviceSummary.restarts} restart
                            {serviceSummary.restarts === 1 ? "" : "s"}
                          </span>
                        ) : (
                          <span>
                            {serviceSummary.totalPods > 0
                              ? `${serviceSummary.readyPods}/${serviceSummary.totalPods} pods ready`
                              : "Awaiting pod inventory"}
                          </span>
                        )}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          ) : (
            <EmptyState
              title="No remote servers attached"
              body="Add a remote Ubuntu host that already has a Dune battlegroup running."
            />
          )}
        </Box>
      </Flex>
    </Box>
  );
}
