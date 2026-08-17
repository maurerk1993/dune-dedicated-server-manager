import type { RemoteServerStatus } from "../../types/server";
import { isSnapshotStale } from "../../utils/dashboard";
import { isBattlegroupStarted } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";

export type BattlegroupControlProps = {
  status?: RemoteServerStatus;
  statusError?: string;
  busyLabel?: string;
  includeRestart?: boolean;
  onStart: () => void;
  onStop: () => void;
  onRestart?: () => void;
};

/** Shared lifecycle controls so every tab applies the same stale-snapshot guard. */
export default function BattlegroupControl({
  status,
  statusError,
  busyLabel,
  includeRestart = false,
  onStart,
  onStop,
  onRestart,
}: BattlegroupControlProps) {
  const actionableStatus = statusError || isSnapshotStale(status) ? undefined : status;
  const battlegroup = status?.battlegroup;
  const battlegroupStarted = actionableStatus
    ? isBattlegroupStarted(actionableStatus.battlegroup)
    : false;
  const startRequested = actionableStatus
    ? !actionableStatus.battlegroup.stop
    : !!battlegroup && !battlegroup.stop;
  const stopped = actionableStatus ? actionableStatus.battlegroup.stop : !!battlegroup?.stop;
  const busy = !!busyLabel;

  return (
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
        {stopped || !actionableStatus ? (
          <ActionButton
            onClick={onStart}
            busy={busy && !battlegroupStarted}
            disabled={busy || !actionableStatus || !stopped}
            tone="accent"
            pendingLabel="Starting"
          >
            Start BattleGroup
          </ActionButton>
        ) : null}
        {startRequested ? (
          <>
            {includeRestart && onRestart ? (
              <ActionButton
                onClick={onRestart}
                busy={busy}
                disabled={busy || !actionableStatus}
                pendingLabel="Restarting"
              >
                Restart
              </ActionButton>
            ) : null}
            <ActionButton
              onClick={onStop}
              busy={busy && startRequested && !stopped}
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
  );
}
