import type { RemoteServerComponent } from "../../types/server";
import {
  formatBytes,
  isRequiredServiceUnavailable,
  podServiceGroups,
} from "../../utils/dashboard";
import { phaseTone } from "../../utils/remote-server";
import StatusPill from "../ui/StatusPill";

export type ServerServiceGridProps = {
  components: RemoteServerComponent[];
};

export default function ServerServiceGrid({ components }: ServerServiceGridProps) {
  const groups = podServiceGroups(components);
  if (groups.length === 0) {
    return (
      <div className="dashboard-empty">
        Service information will appear after the server has been refreshed.
      </div>
    );
  }
  const resourceUsageAvailable = groups.some(
    (component) =>
      component.cpuMillicores !== null && component.cpuMillicores !== undefined ||
      component.memoryBytes !== null && component.memoryBytes !== undefined,
  );

  return (
    <>
      <div className="service-health-grid">
        {groups.map((component) => {
          const ready = component.readyPods ?? 0;
          const total = component.totalPods ?? 0;
          const tone = isRequiredServiceUnavailable(component)
            ? "err"
            : phaseTone(component.state);
          return (
            <article
              className="service-health-card"
              data-tone={tone}
              key={component.logKey}
            >
              <div className="service-health-heading">
                <span className="service-health-name">{component.name}</span>
                <StatusPill
                  label={component.state}
                  tone={tone}
                  pulse={component.state.trim().toLowerCase() === "starting"}
                />
              </div>
              <div className="service-health-pods">
                <strong>
                  {ready}/{total}
                </strong>{" "}
                pods ready
              </div>
              <div className="service-health-meta">
                {component.restartCount > 0 ? (
                  <span className="service-health-restarts">
                    {component.restartCount} restart{component.restartCount === 1 ? "" : "s"}
                  </span>
                ) : (
                  <span>No restarts</span>
                )}
                {component.memoryBytes !== null && component.memoryBytes !== undefined ? (
                  <span>{formatBytes(component.memoryBytes)} RAM</span>
                ) : null}
                {component.cpuMillicores !== null && component.cpuMillicores !== undefined ? (
                  <span>{Math.round(component.cpuMillicores)}m CPU</span>
                ) : null}
              </div>
            </article>
          );
        })}
      </div>
      {!resourceUsageAvailable ? (
        <div className="dashboard-section-note">
          Per-service CPU and RAM are not published by this host. Pod readiness and restart
          monitoring remain available.
        </div>
      ) : null}
    </>
  );
}
