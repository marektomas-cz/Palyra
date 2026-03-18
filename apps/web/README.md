# Web Dashboard

This package hosts the Palyra operator dashboard for the `palyrad` `/console/v1/*`
surface.

## Dashboard surface

- `Overview` summarizes deployment posture plus the control-plane capability catalog.
- `Chat and Sessions` hosts the operator chat/session workflows.
- `Approvals` exposes pending approval decisions and audit-ready outcomes.
- `Cron` covers job create/update/enable-disable/run-now and run log inspection.
- `Channels and Router` covers channel inventory, Discord onboarding, router preview, and
  pairing-code handoff flows.
- `Browser` covers browser profiles, relay actions, and download artifact inspection.
- `Memory` covers scoped ingest/search/purge workflows.
- `Skills` covers install/update/remove/verify/audit/quarantine/enable workflows.
- `OpenAI and Auth Profiles` covers model-provider API-key and OAuth-backed auth profile
  operations.
- `Config and Secrets` covers config inspection, mutation, migration, backup recovery, and
  explicit secret reveal/store/delete flows.
- `Pairing and Gateway Access` covers dashboard access posture, remote verification
  handoff, SSH tunnel handoff, and pairing status visibility.
- `Diagnostics and Audit` covers diagnostics snapshots, audit event browsing, and
  operator-facing internal-only capability notes.
- `Support and Recovery` covers support bundle jobs, recovery-oriented diagnostics, and
  deployment posture summaries.

## Capability catalog contract

- The dashboard consumes `/console/v1/control-plane/capabilities` and renders each
  capability with a dashboard section, execution mode, exposure mode, and optional CLI
  handoff commands.
- `dashboard_exposure` is explicit:
  - `direct_action` means the dashboard exposes the operation directly.
  - `cli_handoff` means the dashboard points the operator to an exact CLI command.
  - `internal_only` means the capability remains visible for parity/audit purposes but is
    not directly executable from the UI.
- Sensitive config and secret values stay redacted by default. Reveal flows require
  explicit operator intent.
- Support, recovery, and remote-access flows are guarded to surface the capability without
  weakening the repository's fail-closed defaults.

## Local commands

- Canonical install:
  - `vp install`
- Format and lint the JS workspace:
  - `vp check apps/web apps/desktop/ui apps/browser-extension scripts`
- Lint the web dashboard only:
  - `vp run web:lint`
- Typecheck:
  - `vp run web:typecheck`
- Tests:
  - `vp run web:test`
- Build:
  - `vp run web:build`
- Full web CI task:
  - `vp run web:ci`

## Notes

- The dashboard is designed for same-origin deployment with `palyrad` `/console/v1` HTTP
  routes.
- API calls always use `credentials: include` to bind requests to the session cookie.
- Mutating requests require CSRF protection.
- The dashboard now installs from the root workspace lockfile and reuses the root `node_modules`
  tree. `apps/web/node_modules` is no longer part of the supported workflow.
- Root toolchain baseline is Node `22.18.0` with `packageManager: npm@10.9.3`.
- `vite`, `vitest`, linting, and formatting now run through the root Vite+ entrypoint.
