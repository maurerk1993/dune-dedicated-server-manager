export type ServerSubPage =
  | "dashboard"
  | "update"
  | "pods"
  | "users"
  | "admin"
  | "welcome"
  | "tasks";

export type ActivePage =
  | { kind: "servers" }
  | { kind: "server"; serverId: string; sub: ServerSubPage };

export const SERVER_AUTO_REFRESH_INTERVALS = [15, 30, 60, 180] as const;

export type ServerAutoRefreshInterval = (typeof SERVER_AUTO_REFRESH_INTERVALS)[number];

export const SERVER_SUB_PAGES: readonly ServerSubPage[] = [
  "dashboard",
  "update",
  "pods",
  "users",
  "admin",
  "welcome",
  "tasks",
] as const;

export const MANAGEMENT_SUB_PAGES: readonly ServerSubPage[] = [
  "users",
  "admin",
  "welcome",
  "tasks",
] as const;

export function isManagementSubPage(sub: ServerSubPage): boolean {
  return MANAGEMENT_SUB_PAGES.includes(sub);
}

export type DetectionState = "idle" | "detecting" | "ready" | "failed";

export type BadgeTone = "green" | "amber" | "red" | "gray" | "bronze";

export type RemoteAttachForm = {
  host: string;
  user: string;
  keyPath: string;
  port: number;
};
