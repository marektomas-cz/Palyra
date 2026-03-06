# Desktop Onboarding Milestone Plan

Status: approved closure-debt implementation plan for the `M50` exit criterion in `roadmap/phase_0-7_fixed.md`.

## Why this exists

The desktop control center is already real and useful. It starts and stops sidecars, surfaces health, opens the dashboard, and exports support bundles. The remaining gap is first-run product flow and backend modularity, not basic validity.

`apps/desktop/src-tauri/src/lib.rs` still owns sidecar supervision, dashboard access resolution, diagnostics snapshot assembly, support-bundle export, login/bootstrap helpers, Tauri command wiring, and runtime startup in one file. That is sufficient for v1 delivery, but it is the wrong shape for onboarding work.

## Current pressure points

- `apps/desktop/src-tauri/src/lib.rs`
  - mixes service lifecycle, snapshot assembly, auth bootstrap, dashboard URL logic, support-bundle export, and Tauri command handlers
  - makes onboarding work riskier because product flow and runtime wiring live in the same module
- `run()` in `apps/desktop/src-tauri/src/lib.rs`
  - registers every Tauri command directly from the monolith
  - should become thin composition over dedicated modules
- Current UX
  - can open the dashboard and show quick facts
  - does not yet lead a new install through a deliberate first-run path from local runtime readiness to dashboard handoff

## Architectural outcome

The desktop onboarding milestone should leave the control center as the obvious local entry point for a fresh install while keeping the dashboard as the canonical full operator surface.

That requires two concrete outcomes:

- backend modularity so onboarding logic can evolve safely
- a first-run state machine that guides the user from install to an operational dashboard handoff

## Delivery slices

### Slice 1: backend module split

Split the Rust backend into focused modules with clear ownership boundaries:

- service manager
  - sidecar process lifecycle
  - health polling
  - supervisor loop concerns
- auth bootstrap
  - local admin/bootstrap session acquisition
  - any onboarding session preflight that must remain desktop-owned
- diagnostics
  - snapshot capture
  - redaction-safe parsing
  - quick-facts shaping
- dashboard access
  - dashboard URL resolution
  - remote-versus-local access mode evaluation
  - browser open handoff
- support bundle
  - export planning and CLI invocation
- command wiring
  - thin Tauri invoke handlers over the module surfaces

Module names can change, but this responsibility split should stay fixed.

### Slice 2: first-run onboarding state machine

Add a guided first-run path that covers the minimum path from install to usable operator surface:

1. runtime check
   - validate required binaries and state-root prerequisites
   - surface actionable failure reasons before starting services
2. gateway init
   - start `palyrad`
   - wait for loopback health and dashboard access readiness
3. OpenAI connect
   - guide the user into the provider-auth step without storing secrets in desktop-local state
   - confirm readiness via diagnostics or dashboard callback state
4. Discord verify
   - guide the user into the Discord verification step
   - confirm connector readiness through the existing diagnostics or channel status surfaces
5. dashboard handoff
   - open the full dashboard once the onboarding minimum is satisfied
   - make it clear that advanced operations belong in the dashboard, not the desktop shell

### Slice 3: focused desktop UX

- Keep the desktop UI focused on lifecycle, health, onboarding state, and handoff.
- Do not turn the desktop control center into a second full dashboard.
- Use lightweight onboarding checkpoints and status cards instead of duplicating the full web console feature set.

### Slice 4: persistence and recovery

- Persist only minimal onboarding progress needed for resume/retry.
- Do not persist provider tokens, connector tokens, or other raw secrets in desktop-local state.
- Preserve loopback-only control-plane behavior and shared redaction helpers.

## Proposed file map

Exact filenames may change, but the boundary plan should remain stable:

- `apps/desktop/src-tauri/src/service_manager.rs`
- `apps/desktop/src-tauri/src/auth_bootstrap.rs`
- `apps/desktop/src-tauri/src/diagnostics.rs`
- `apps/desktop/src-tauri/src/dashboard_access.rs`
- `apps/desktop/src-tauri/src/support_bundle.rs`
- `apps/desktop/src-tauri/src/commands.rs`
- `apps/desktop/src-tauri/src/lib.rs`
  - reduced to app composition, state wiring, and runtime bootstrap

## Security and product constraints

- Keep control-plane calls loopback-only.
- Keep sensitive diagnostics and logs redacted by default.
- Keep desktop-local state free of provider or connector secrets.
- Keep the dashboard as the canonical full operator surface for advanced workflows.
- Preserve existing support-bundle and diagnostics exports as operator-safe artifacts.

## Acceptance criteria

This milestone is complete when all of the following are true:

- `apps/desktop/src-tauri/src/lib.rs` is reduced to a thin composition layer.
- Desktop backend responsibilities are split into focused modules.
- A new install can be guided through runtime check, gateway init, OpenAI connect, Discord verify, and dashboard handoff.
- The desktop app remains lifecycle and onboarding focused rather than becoming a second dashboard.
- Sensitive values remain redacted and secret persistence boundaries stay unchanged.

## Validation plan

- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --release --locked`
- Focused regression coverage for:
  - snapshot diagnostics redaction
  - dashboard access resolution
  - support-bundle export
  - onboarding resume/failure states once introduced

## Scope boundaries

- This milestone does not replace the web dashboard.
- This milestone does not introduce general provider-management UX beyond the focused first-run path.
- This milestone does not weaken loopback, auth, or redaction invariants.
