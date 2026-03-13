import { invoke } from "@tauri-apps/api/core";

export type RuntimeStatus = "healthy" | "degraded" | "down";

export type ActionResult = {
  ok: boolean;
  message: string;
};

export type BrowserServiceSnapshot = {
  enabled: boolean;
  healthy: boolean;
  status: string;
  uptime_seconds: number | null;
  last_error: string | null;
};

export type QuickFactsSnapshot = {
  dashboard_url: string;
  dashboard_access_mode: string;
  gateway_version: string | null;
  gateway_git_hash: string | null;
  gateway_uptime_seconds: number | null;
  browser_service: BrowserServiceSnapshot;
};

export type DiagnosticsSnapshot = {
  generated_at_unix_ms: number | null;
  errors: string[];
  dropped_log_events_total: number;
};

export type ServiceProcessSnapshot = {
  service: string;
  desired_running: boolean;
  running: boolean;
  liveness: string;
  pid: number | null;
  last_start_unix_ms: number | null;
  last_exit: string | null;
  restart_attempt: number;
  next_restart_unix_ms: number | null;
  bound_ports: number[];
};

export type ControlCenterSnapshot = {
  generated_at_unix_ms: number;
  overall_status: RuntimeStatus;
  quick_facts: QuickFactsSnapshot;
  diagnostics: DiagnosticsSnapshot;
  gateway_process: ServiceProcessSnapshot;
  browserd_process: ServiceProcessSnapshot;
  warnings: string[];
};

type DesktopGlobal = typeof globalThis & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

export const DESKTOP_PREVIEW_SNAPSHOT: ControlCenterSnapshot = {
  generated_at_unix_ms: Date.UTC(2026, 2, 13, 12, 0, 0),
  overall_status: "healthy",
  quick_facts: {
    dashboard_url: "http://127.0.0.1:7142/",
    dashboard_access_mode: "local",
    gateway_version: "preview",
    gateway_git_hash: "preview",
    gateway_uptime_seconds: 142,
    browser_service: {
      enabled: true,
      healthy: true,
      status: "running",
      uptime_seconds: 141,
      last_error: null
    }
  },
  diagnostics: {
    generated_at_unix_ms: Date.UTC(2026, 2, 13, 12, 0, 0),
    errors: [],
    dropped_log_events_total: 0
  },
  gateway_process: {
    service: "gateway",
    desired_running: true,
    running: true,
    liveness: "running",
    pid: 7142,
    last_start_unix_ms: Date.UTC(2026, 2, 13, 11, 58, 0),
    last_exit: null,
    restart_attempt: 0,
    next_restart_unix_ms: null,
    bound_ports: [7142, 7152]
  },
  browserd_process: {
    service: "browserd",
    desired_running: true,
    running: true,
    liveness: "running",
    pid: 7242,
    last_start_unix_ms: Date.UTC(2026, 2, 13, 11, 58, 4),
    last_exit: null,
    restart_attempt: 0,
    next_restart_unix_ms: null,
    bound_ports: [9222]
  },
  warnings: []
};

function desktopGlobal(): DesktopGlobal {
  return globalThis as DesktopGlobal;
}

export function isDesktopHostAvailable(): boolean {
  const host = desktopGlobal();
  return typeof host.__TAURI_INTERNALS__ !== "undefined" || typeof host.__TAURI__ !== "undefined";
}

export async function showMainWindow(): Promise<void> {
  return invoke<void>("show_main_window");
}

export async function getSnapshot(): Promise<ControlCenterSnapshot> {
  return invoke<ControlCenterSnapshot>("get_snapshot");
}

export async function startPalyra(): Promise<ActionResult> {
  return invoke<ActionResult>("start_palyra");
}

export async function stopPalyra(): Promise<ActionResult> {
  return invoke<ActionResult>("stop_palyra");
}

export async function restartPalyra(): Promise<ActionResult> {
  return invoke<ActionResult>("restart_palyra");
}

export async function openDashboard(): Promise<ActionResult> {
  return invoke<ActionResult>("open_dashboard");
}
