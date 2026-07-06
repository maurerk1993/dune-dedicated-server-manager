import { useEffect, useRef, useState } from "react";

import {
  readServerAutoRefreshPrefs,
  writeServerAutoRefreshPrefs,
  type ServerAutoRefreshPrefs,
} from "../services/storage";
import type { RemoteServerRecord } from "../types/server";
import type { ServerAutoRefreshInterval } from "../types/ui";

type UseServerAutoRefreshArgs = {
  activeServer?: RemoteServerRecord;
  busyMap: Record<string, string>;
  onRefreshServer: (server: RemoteServerRecord) => void | Promise<void>;
};

export function useServerAutoRefresh({
  activeServer,
  busyMap,
  onRefreshServer,
}: UseServerAutoRefreshArgs) {
  const [prefs, setPrefs] = useState<ServerAutoRefreshPrefs>(() =>
    readServerAutoRefreshPrefs(),
  );
  const activeServerRef = useRef(activeServer);
  const busyMapRef = useRef(busyMap);
  const refreshRef = useRef(onRefreshServer);

  useEffect(() => {
    activeServerRef.current = activeServer;
  }, [activeServer]);

  useEffect(() => {
    busyMapRef.current = busyMap;
  }, [busyMap]);

  useEffect(() => {
    refreshRef.current = onRefreshServer;
  }, [onRefreshServer]);

  const setAutoRefreshPrefs = (
    resolveNext: (current: ServerAutoRefreshPrefs) => ServerAutoRefreshPrefs,
  ) => {
    setPrefs((current) => {
      const next = resolveNext(current);
      writeServerAutoRefreshPrefs(next);
      return next;
    });
  };

  const toggleAutoRefresh = () => {
    setAutoRefreshPrefs((current) => ({ ...current, enabled: !current.enabled }));
  };

  const setAutoRefreshInterval = (intervalSeconds: ServerAutoRefreshInterval) => {
    setAutoRefreshPrefs((current) => ({ ...current, intervalSeconds }));
  };

  useEffect(() => {
    if (!prefs.enabled) return undefined;
    const handle = window.setInterval(() => {
      const server = activeServerRef.current;
      if (!server || busyMapRef.current[server.id]) return;
      void refreshRef.current(server);
    }, prefs.intervalSeconds * 1000);

    return () => window.clearInterval(handle);
  }, [prefs.enabled, prefs.intervalSeconds]);

  return {
    autoRefreshEnabled: prefs.enabled,
    autoRefreshIntervalSeconds: prefs.intervalSeconds,
    toggleAutoRefresh,
    setAutoRefreshInterval,
  };
}
