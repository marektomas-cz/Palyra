import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { OperationsSection } from "./OperationsSection";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("OperationsSection", () => {
  it("renders diagnostics when usage insights omit optional arrays", () => {
    render(
      <MemoryRouter>
        <OperationsSection
          app={{
            auditBusy: false,
            auditFilterContains: "",
            setAuditFilterContains: vi.fn(),
            auditFilterPrincipal: "",
            setAuditFilterPrincipal: vi.fn(),
            auditEvents: [],
            refreshAudit: vi.fn(async () => {}),
            diagnosticsBusy: false,
            diagnosticsSnapshot: {
              model_provider: { state: "ok", provider: "deterministic" },
              auth_profiles: { state: "ok", profiles: [] },
              browserd: { state: "disabled", engine_mode: "headless_chrome" },
              networked_workers: {
                state: "degraded",
                mode: "preview_only",
                metrics: {
                  registered_workers: 1,
                  attested_workers: 1,
                  active_leases: 1,
                  orphaned_workers: 1,
                  failed_closed_workers: 0,
                  orphan_rate_bps: 5000,
                  lease_failures: 1,
                  transport_failures: 0,
                  fallback_to_local_rate_bps: 0,
                },
                recent_events: [
                  {
                    worker_id: "worker-test-01",
                    state: "orphaned",
                    reason_code: "worker.ttl_expired",
                    timestamp_unix_ms: 1700000000000,
                  },
                ],
                recovery: {
                  recommended_actions: ["Run force cleanup for orphaned workers."],
                  gate_criteria: ["orphaned_workers == 0"],
                },
                actions: [
                  {
                    id: "reap_expired",
                    scope: "fleet",
                    label: "Reap expired leases",
                    method: "POST",
                    api_path: "/console/v1/networked-workers/reap-expired",
                  },
                ],
              },
              observability: {
                config_ref_health: {
                  state: "degraded",
                  summary: { blocking_refs: 1, warning_refs: 1 },
                  recommendations: [
                    "Restart the daemon to refresh this config ref in the running runtime.",
                  ],
                  items: [
                    {
                      ref_id: "admin.auth_token_secret_ref:fp-1",
                      config_path: "admin.auth_token_secret_ref",
                      state: "stale",
                      severity: "warning",
                      reload_mode: "restart_required",
                      advice:
                        "Restart the daemon to refresh this config ref in the running runtime.",
                    },
                  ],
                },
              },
            } as never,
            refreshDiagnostics: vi.fn(async () => {}),
            overviewUsageInsights: {
              routing: { default_mode: "suggest" },
              budgets: {},
            } as never,
            overviewCatalog: null,
            memoryStatus: null,
            refreshMemoryStatus: vi.fn(async () => {}),
            api: {} as never,
            setError: vi.fn(),
            setNotice: vi.fn(),
            revealSensitiveValues: false,
          }}
        />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Diagnostics" })).toBeInTheDocument();
    expect(screen.getByText("0 active alerts")).toBeInTheDocument();
    expect(screen.getAllByText("Config ref health").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Networked workers").length).toBeGreaterThan(0);
    expect(screen.getByText("worker.ttl_expired")).toBeInTheDocument();
    expect(
      screen.getAllByText("Restart the daemon to refresh this config ref in the running runtime.")
        .length,
    ).toBeGreaterThan(0);
  });
});
