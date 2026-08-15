import { useRef, useState } from "react";

import {
  detectRemoteUbuntuServers,
  getRemoteServerComponents,
  getRemoteServerStatus,
  restartRemoteBattlegroup,
  startRemoteBattlegroup,
  stopRemoteBattlegroup,
} from "../services/tauri";
import { persistRemoteServers, upsertRemoteServer } from "../services/storage";
import type { LogRow } from "../types/log";
import type {
  RemoteBattlegroupStatus,
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../types/server";
import { errorMessage } from "../utils/errors";
import { withTimeout } from "../utils/async";
import { log } from "../utils/logging";
import {
  omitKey,
  omitPrefix,
  remoteServerActionRequest,
  remoteServerDefaultUser,
} from "../utils/remote-server";

type UseRemoteServerStatusArgs = {
  appendLogRow: (row: LogRow) => void;
  setRemoteServers: React.Dispatch<React.SetStateAction<RemoteServerRecord[]>>;
};

const REMOTE_REFRESH_TIMEOUT_MS = 60_000;
const REMOTE_COMPONENT_REFRESH_TIMEOUT_MS = 45_000;
const REMOTE_STOP_TIMEOUT_MS = 3 * 60_000;
const REMOTE_START_TIMEOUT_MS = 8 * 60_000;
const REMOTE_RESTART_TIMEOUT_MS = 10 * 60_000;

export function useRemoteServerStatus({ appendLogRow, setRemoteServers }: UseRemoteServerStatusArgs) {
  const [remoteServerStatuses, setRemoteServerStatuses] = useState<Record<string, RemoteServerStatus>>({});
  const [remoteServerComponents, setRemoteServerComponents] = useState<Record<string, RemoteServerComponent[]>>({});
  const [remoteServerStatusErrors, setRemoteServerStatusErrors] = useState<Record<string, string>>({});
  const [remoteServerBusy, setRemoteServerBusy] = useState<Record<string, string>>({});
  const [remoteComponentLogs, setRemoteComponentLogs] = useState<Record<string, string>>({});
  const [remoteComponentLogBusy, setRemoteComponentLogBusy] = useState<Record<string, boolean>>({});
  const [remoteComponentRestartBusy, setRemoteComponentRestartBusy] = useState<Record<string, boolean>>({});
  const nextOperationIdRef = useRef(0);
  const activeOperationIdsRef = useRef(new Map<string, number>());

  const beginServerOperation = (serverId: string): number | null => {
    if (activeOperationIdsRef.current.has(serverId)) return null;
    const operationId = ++nextOperationIdRef.current;
    activeOperationIdsRef.current.set(serverId, operationId);
    return operationId;
  };

  const isCurrentServerOperation = (serverId: string, operationId: number): boolean =>
    activeOperationIdsRef.current.get(serverId) === operationId;

  const finishServerOperation = (serverId: string, operationId: number) => {
    if (!isCurrentServerOperation(serverId, operationId)) return;
    activeOperationIdsRef.current.delete(serverId);
    setRemoteServerBusy((busy) => omitKey(busy, serverId));
  };

  const detectRemoteServerDetails = async (server: RemoteServerRecord): Promise<RemoteServerRecord> => {
    const detected = await detectRemoteUbuntuServers({
      host: server.host,
      keyPath: server.keyPath,
      serverType: "ubuntu",
      user: server.user || remoteServerDefaultUser(server.type),
      port: server.port,
    });
    if (detected.length === 0) {
      throw new Error("No Dune battlegroups were detected on the remote server.");
    }
    return detected.find((candidate) => candidate.battlegroupName === server.battlegroupName) ?? detected[0];
  };

  const refreshRemoteServerStatus = async (server: RemoteServerRecord) => {
    if (!server.host || !server.keyPath) return;
    const operationId = beginServerOperation(server.id);
    if (operationId === null) return;
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: "Retrieving server information" }));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    try {
      const { liveServer, status, components } = await withTimeout(
        (async () => {
          const liveServer = await detectRemoteServerDetails(server);
          const request = remoteServerActionRequest(liveServer);
          const [status, components] = await Promise.all([
            getRemoteServerStatus(request),
            getRemoteServerComponents(request),
          ]);
          return { liveServer, status, components };
        })(),
        REMOTE_REFRESH_TIMEOUT_MS,
        "Refresh timed out after 60 seconds while the battlegroup was changing state. You can retry now.",
      );
      if (!isCurrentServerOperation(server.id, operationId)) return;
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id
              ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase }
              : candidate,
          ),
        ),
      );
      appendLogRow(
        log.info(
          "remote.status",
          buildStatusLogLine(liveServer.name, status.battlegroup),
          liveServer.id,
        ),
      );
    } catch (err) {
      if (!isCurrentServerOperation(server.id, operationId)) return;
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.warn("remote.status", message, server.id));
    } finally {
      finishServerOperation(server.id, operationId);
    }
  };

  const runRemoteBattlegroupAction = async (
    server: RemoteServerRecord,
    action: "start" | "stop" | "restart",
  ) => {
    if (!server.host || !server.keyPath) return;
    const operationId = beginServerOperation(server.id);
    if (operationId === null) return;
    const verbs: Record<typeof action, [busy: string, log: string]> = {
      start: ["Starting battlegroup", "Starting"],
      stop: ["Stopping battlegroup", "Stopping"],
      restart: ["Restarting battlegroup", "Restarting"],
    };
    const [busyText, verb] = verbs[action];
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: busyText }));
    appendLogRow(log.info("bg", `${verb} remote battlegroup.`, server.id));
    try {
      const timeoutMs = battlegroupActionTimeoutMs(action);
      const { liveServer, request, status } = await withTimeout(
        (async () => {
          const liveServer =
            server.namespace && server.battlegroupName
              ? server
              : await detectRemoteServerDetails(server);
          const request = remoteServerActionRequest(liveServer);
          const status =
            action === "start"
              ? await startRemoteBattlegroup(request)
              : action === "stop"
                ? await stopRemoteBattlegroup(request)
                : await restartRemoteBattlegroup(request);
          return { liveServer, request, status };
        })(),
        timeoutMs,
        `${verb} the battlegroup timed out. The server may still finish the operation; refresh to check its current state.`,
      );
      if (!isCurrentServerOperation(server.id, operationId)) return;
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id
              ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase }
              : candidate,
          ),
        ),
      );
      setRemoteServerBusy((busy) => ({ ...busy, [server.id]: "Refreshing server services" }));
      try {
        const components = await withTimeout(
          getRemoteServerComponents(request),
          REMOTE_COMPONENT_REFRESH_TIMEOUT_MS,
          "Service details timed out after the battlegroup action completed.",
        );
        if (isCurrentServerOperation(server.id, operationId)) {
          setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
        }
      } catch (err) {
        if (isCurrentServerOperation(server.id, operationId)) {
          appendLogRow(
            log.warn(
              "remote.components",
              `${errorMessage(err)} The latest battlegroup status was retained.`,
              server.id,
            ),
          );
        }
      }
    } catch (err) {
      if (!isCurrentServerOperation(server.id, operationId)) return;
      const message = errorMessage(err);
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.error("bg", message, server.id));
    } finally {
      finishServerOperation(server.id, operationId);
    }
  };

  const clearStatusForServer = (serverId: string) => {
    activeOperationIdsRef.current.delete(serverId);
    setRemoteServerStatuses((statuses) => omitKey(statuses, serverId));
    setRemoteServerComponents((components) => omitKey(components, serverId));
    setRemoteServerStatusErrors((errors) => omitKey(errors, serverId));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${serverId}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${serverId}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${serverId}:`));
    setRemoteServerBusy((busy) => omitKey(busy, serverId));
  };

  return {
    remoteServerStatuses,
    remoteServerComponents,
    setRemoteServerComponents,
    remoteServerStatusErrors,
    remoteServerBusy,
    remoteComponentLogs,
    setRemoteComponentLogs,
    remoteComponentLogBusy,
    setRemoteComponentLogBusy,
    remoteComponentRestartBusy,
    setRemoteComponentRestartBusy,
    detectRemoteServerDetails,
    refreshRemoteServerStatus,
    runRemoteBattlegroupAction,
    clearStatusForServer,
  };
}

function battlegroupActionTimeoutMs(action: "start" | "stop" | "restart"): number {
  if (action === "stop") return REMOTE_STOP_TIMEOUT_MS;
  if (action === "start") return REMOTE_START_TIMEOUT_MS;
  return REMOTE_RESTART_TIMEOUT_MS;
}

function buildStatusLogLine(name: string, bg: RemoteBattlegroupStatus): string {
  const parts: string[] = [
    `${name}: ${bg.phase || "unknown"}`,
    `server group ${bg.serverGroupPhase || "unknown"}`,
  ];
  if (bg.databasePhase) parts.push(`DB ${bg.databasePhase}`);
  parts.push(`Director ${bg.directorPhase || "unknown"}`);
  if (bg.uptime) parts.push(`up ${bg.uptime}`);
  if (bg.stop) parts.push("STOP");
  return parts.join(", ") + ".";
}
