import type { StatusTone } from "../ui/StatusPill";

export type DashboardMetricCardProps = {
  label: string;
  value: string;
  detail: string;
  tone?: StatusTone;
  progress?: number | null;
};

export default function DashboardMetricCard({
  label,
  value,
  detail,
  tone = "gray",
  progress,
}: DashboardMetricCardProps) {
  const normalizedProgress =
    progress === null || progress === undefined
      ? null
      : Math.max(0, Math.min(100, progress));

  return (
    <div className="dashboard-metric-card" data-tone={tone}>
      <div className="dashboard-metric-header">
        <span className="dashboard-metric-label">{label}</span>
        <span className="dashboard-metric-indicator" aria-hidden />
      </div>
      <div className="dashboard-metric-value">{value || "—"}</div>
      <div className="dashboard-metric-detail">{detail}</div>
      {normalizedProgress !== null ? (
        <div
          className="dashboard-meter"
          role="progressbar"
          aria-label={`${label} usage`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(normalizedProgress)}
        >
          <span style={{ width: `${normalizedProgress}%` }} />
        </div>
      ) : null}
    </div>
  );
}
