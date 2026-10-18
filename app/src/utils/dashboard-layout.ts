export const DEFAULT_DASHBOARD_ORDER = [
  "overview", "services", "maps", "control", "technical", "management", "connections",
] as const;

export type DashboardSectionId = (typeof DEFAULT_DASHBOARD_ORDER)[number];
export const DASHBOARD_LAYOUT_KEY = "dune-manager.dashboard-layout.v1";

export const DASHBOARD_SECTION_LABELS: Record<DashboardSectionId, string> = {
  overview: "Server overview",
  services: "Battlegroup services",
  maps: "Maps and players",
  control: "Battlegroup control",
  technical: "Technical details",
  management: "Management service",
  connections: "Connections",
};

/** Keep every current section exactly once, including sections added in future releases. */
export function normalizeDashboardOrder(value: unknown): DashboardSectionId[] {
  const saved = Array.isArray(value) ? value : [];
  return [...new Set([...saved.filter((id): id is DashboardSectionId =>
    typeof id === "string" && (DEFAULT_DASHBOARD_ORDER as readonly string[]).includes(id)),
    ...DEFAULT_DASHBOARD_ORDER])];
}

export function moveDashboardSection(
  order: readonly DashboardSectionId[],
  source: DashboardSectionId,
  target: DashboardSectionId,
  edge: "before" | "after",
): DashboardSectionId[] {
  const next = normalizeDashboardOrder(order);
  if (source === target || !next.includes(source) || !next.includes(target)) return next;
  next.splice(next.indexOf(source), 1);
  next.splice(next.indexOf(target) + (edge === "after" ? 1 : 0), 0, source);
  return next;
}

export function readDashboardOrder(storage: Pick<Storage, "getItem">): DashboardSectionId[] {
  try {
    return normalizeDashboardOrder(JSON.parse(storage.getItem(DASHBOARD_LAYOUT_KEY) ?? "null"));
  } catch {
    return [...DEFAULT_DASHBOARD_ORDER];
  }
}

export function saveDashboardOrder(
  storage: Pick<Storage, "setItem">,
  order: readonly DashboardSectionId[],
): boolean {
  try {
    storage.setItem(DASHBOARD_LAYOUT_KEY, JSON.stringify(normalizeDashboardOrder(order)));
    return true;
  } catch {
    return false;
  }
}
