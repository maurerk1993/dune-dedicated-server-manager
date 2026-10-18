// Isolated browser fixture: synthetic data, no live profiles or backend actions.
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { Theme } from "@radix-ui/themes";
import "@radix-ui/themes/styles.css";
import "../src/styles.css";
import ServerDashboard from "../src/components/servers/ServerDashboard";

function Fixture() {
  const [refreshes, setRefreshes] = useState(0);
  const [server, setServer] = useState("example-one");
  const [actions, setActions] = useState(0);
  const action = () => setActions((count) => count + 1);
  return <Theme appearance="dark"><main style={{ padding: 24, height: "100vh", overflow: "auto" }}>
    <button onClick={() => setRefreshes((count) => count + 1)}>Simulate refresh</button>
    <button onClick={() => setServer((id) => id === "example-one" ? "example-two" : "example-one")}>Switch server</button>
    <output aria-label="Action count">{actions}</output>
    <output aria-label="Refresh count">{refreshes}</output>
    <ServerDashboard
      server={{ type: "ubuntu", id: server, name: "Example", host: "127.0.0.1", user: "fixture", keyPath: "",
        namespace: "example", battlegroupName: "example", worldUniqueName: "example", phase: "Healthy" }}
      status={{ collectedAt: new Date().toISOString(), package: { installedBuildId: "example" },
        battlegroup: { stop: false, phase: "Healthy", databasePhase: "Ready", directorPhase: "Healthy", serverGroupPhase: "Running",
          serverStats: ["Hagga Basin", "Overmap", "Deep Desert"].map((map) => ({ map, phase: "Running", ready: "true", players: "2", age: "2h" })) },
        hostMetrics: { memoryUsedBytes: 16e9, memoryTotalBytes: 32e9, swapUsedBytes: 0, swapTotalBytes: 0,
          cpuUsagePercent: 28, loadAverageOne: 1.2, diskUsedBytes: 30e9, diskTotalBytes: 100e9, uptimeSeconds: 7200 } }}
      components={[{ name: "Database", logKey: "database", category: "system", componentKind: "pod-group", state: "Ready", tone: "ok",
        summary: "All pods ready", details: [], readyPods: 1, totalPods: 1, restartCount: 2 }]}
      tunnels={{}} tunnelBusy={{}} managementStatus={{ kind: "idle" }}
      onRefreshManagement={async () => { action(); }} appendLogRow={() => {}}
      onStartBattlegroup={action} onStopBattlegroup={action} onRestartBattlegroup={action}
      onStartTunnel={action} onStartCustomTunnel={action} onStopTunnel={action} onOpenTunnel={action}
    />
  </main></Theme>;
}

createRoot(document.getElementById("root")!).render(<Fixture />);
