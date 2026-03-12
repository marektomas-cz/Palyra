# Palyra

Palyra is a Rust-first workspace for policy-gated agent runtime orchestration, operator workflows,
channel routing, secure tool execution, and bounded browser automation.

The repository currently centers around three runtime binaries:

- `palyrad`: daemon runtime exposing admin/console HTTP surfaces and gRPC services.
- `palyra`: operator CLI for config, auth, agents, channels, memory, approvals, skills, and
  support workflows.
- `palyra-browserd`: isolated browser service for brokered `palyra.browser.*` tool execution.

## Repository layout

- `crates/`: Rust workspace crates.
- `apps/web`: operator web console.
- `apps/desktop`: Tauri desktop control center.
- `apps/browser-extension`: local MV3 relay artifact for browser workflows.
- `schemas/`: protobuf + generated stubs.
- `docs/`: current-state architecture, operator, and development docs.

## Current implementation layout

- `crates/palyra-daemon`:
  - `src/app/*` for bootstrap/runtime/shutdown wiring,
  - `src/config/*` for config loading and schema projection,
  - `src/application/*` for run-stream, route-message, provider-event, auth, and tool-runtime
    orchestration,
  - `src/transport/http/*` and `src/transport/grpc/*` for transport surfaces,
  - `src/gateway/*` plus `src/gateway.rs` as the shared runtime/support facade.
- `crates/palyra-cli`:
  - `src/commands/*` for command dispatch and execution,
  - `src/client/*` for reusable HTTP/gRPC clients,
  - `src/output/*` for text/JSON rendering,
  - `src/lib.rs` as the shared CLI runtime/root.
- `crates/palyra-browserd`:
  - `src/app/*`, `src/transport/*`, `src/engine/*`, `src/security/*`, `src/persistence/*`,
    `src/support/*`,
  - `src/lib.rs` as the shared browserd root.
- Connector platform:
  - `crates/palyra-connector-core` holds generic connector contracts/runtime primitives,
  - `crates/palyra-connector-discord` holds Discord-specific semantics and adapter logic,
  - `crates/palyra-connectors` remains the default adapter-registry facade for daemon wiring.

## Local validation entrypoints

Core local commands:

```bash
just fmt-check
just lint
just test
just deterministic-core
just performance-smoke
just push-gate-fast
just module-budgets
```

Full web validation:

```bash
npm --prefix apps/web run lint
npm --prefix apps/web run typecheck
npm --prefix apps/web run test:run
npm --prefix apps/web run build
```

Additional docs and current-state runbooks are indexed in [docs/README.md](docs/README.md).

## License

Palyra is licensed under the Functional Source License, Version 1.1, ALv2 Future License
(`FSL-1.1-ALv2`). This is a Fair Source / source-available license and is not an OSI-approved Open
Source license. Each version converts to the Apache License, Version 2.0 two years after it is
made available. See [LICENSE](LICENSE).
