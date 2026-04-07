# Repository Structure and Workspace Layout

<details>
<summary>Relevant source files</summary>

The following files were used as context for generating this wiki page:

- Cargo.lock
- Cargo.toml
- LICENSE
- Makefile
- README.md
- crates/palyra-a2ui/Cargo.toml
- crates/palyra-auth/Cargo.toml
- crates/palyra-cli/Cargo.toml
- crates/palyra-cli/build.rs
- crates/palyra-connectors/Cargo.toml
- crates/palyra-daemon/Cargo.toml
- crates/palyra-daemon/build.rs
- crates/palyra-plugins/runtime/Cargo.toml
- deny.toml
- infra/ci/security.yml
- justfile
- osv-scanner.toml

</details>



This page describes the organization of the Palyra monorepo, detailing the Cargo workspace, the internal crate ecosystem, application directories, and supporting infrastructure. The repository is designed to support a multi-process AI agent architecture with strict security boundaries and cross-platform compatibility.

## Cargo Workspace Overview

Palyra is managed as a single Cargo workspace containing 18 internal Rust crates. This structure allows for shared dependency management, atomic cross-crate refactoring, and a unified build pipeline.

The workspace configuration is defined in the root `Cargo.toml`, which specifies the member crates and shared workspace-level dependencies [Cargo.toml#1-21](http://Cargo.toml#1-21). It uses the Rust 2021 edition and targets Rust version 1.91 [Cargo.toml#25-28](http://Cargo.toml#25-28).

### Internal Crates (crates/)

The core logic is partitioned into specialized crates to enforce modularity and reduce compile times for individual components.

| Crate Name | Purpose | Key Dependencies |
|:---|:---|:---|
| `palyra-daemon` | The central orchestrator (`palyrad`). Manages sessions, LLM flow, and persistence. | `tonic`, `axum`, `rusqlite`, `quinn` |
| `palyra-cli` | The primary operator interface (`palyra`). Includes the TUI and ACP bridge. | `ratatui`, `clap`, `agent-client-protocol` |
| `palyra-browserd` | Headless browser automation daemon using Chromium. | `headless_chrome` |
| `palyra-sandbox` | Execution isolation for tools (WASM, rlimit, bwrap). | `wasmtime`, `libc` |
| `palyra-policy` | Cedar-based policy evaluation engine. | `cedar-policy` (implied by context) |
| `palyra-vault` | Platform-specific secret management (Keychain, Secret Service, DPAPI). | `ring`, `aes` |
| `palyra-identity` | mTLS and device identity management. | `rcgen`, `ed25519-dalek` |
| `palyra-plugins-runtime` | WASM execution environment for agent skills. | `wasmtime` |
| `palyra-common` | Shared types, utilities, and error definitions. | `serde`, `ulid` |
| `palyra-auth` | Authentication providers and OAuth bootstrap logic. | `reqwest` |
| `palyra-skills` | Skill manifest parsing and lifecycle management. | `toml`, `zip` |
| `palyra-connector-core` | Trait definitions for external messaging adapters. | `async-trait` |
| `palyra-connector-discord` | Discord-specific bot implementation. | `tokio-tungstenite` |

**Sources:** [Cargo.toml#1-21](http://Cargo.toml#1-21), [crates/palyra-daemon/Cargo.toml#1-55](http://crates/palyra-daemon/Cargo.toml#1-55), [crates/palyra-cli/Cargo.toml#1-45](http://crates/palyra-cli/Cargo.toml#1-45)

## Application Directory (apps/)

The `apps/` directory contains the various user-facing interfaces and platform-specific stubs.

*   **Web Console:** Located in `apps/web`, this is a React-based dashboard for monitoring and interacting with the daemon.
*   **Desktop App:** Located in `apps/desktop`, a Tauri-based application that acts as a process supervisor for `palyrad` and `palyra-browserd`.
*   **Browser Extension:** Located in `apps/browser-extension`, providing integration between the user's browser and the automation daemon.
*   **Mobile Stubs:** `apps/android` and `apps/ios` (implied by linting scripts) contain platform-specific logic for mobile integration [justfile#27](http://justfile#27).

## Protocol Schemas (schemas/)

The communication between all components (CLI, Daemon, Browser, Web) is governed by Protobuf definitions located in `schemas/proto/palyra/v1/`.

The build process for crates like `palyra-daemon` and `palyra-cli` uses `tonic-build` to generate Rust stubs from these schemas during compilation [crates/palyra-daemon/build.rs#3-35](http://crates/palyra-daemon/build.rs#3-35), [crates/palyra-cli/build.rs#3-35](http://crates/palyra-cli/build.rs#3-35).

**Sources:** [crates/palyra-daemon/build.rs#7-14](http://crates/palyra-daemon/build.rs#7-14), [crates/palyra-cli/build.rs#7-14](http://crates/palyra-cli/build.rs#7-14)

## Dependency Graph and Data Flow

The following diagram illustrates how the primary binaries interact and which internal crates they depend on to fulfill their roles.

### System Architecture and Crate Association

```mermaid
graph TD
    subgraph "External Clients"
        [Discord_API]
        [Web_Browser]
    end

    subgraph "Binaries (Code Entities)"
        [palyrad] --> [palyra-daemon]
        [palyra] --> [palyra-cli]
        [palyra-browserd] --> [palyra-browserd-crate]
    end

    subgraph "Core Shared Logic (Crates)"
        [palyra-daemon] --> [palyra-auth]
        [palyra-daemon] --> [palyra-vault]
        [palyra-daemon] --> [palyra-identity]
        [palyra-daemon] --> [palyra-policy]
        [palyra-daemon] --> [palyra-sandbox]
        
        [palyra-cli] --> [palyra-control-plane]
        [palyra-cli] --> [palyra-identity]
        
        [palyra-connector-discord] --> [palyra-connector-core]
        [palyra-daemon] --> [palyra-connectors]
    end

    [palyrad] -- "gRPC (gateway.proto)" --> [palyra]
    [palyrad] -- "HTTP/JSON" --> [Web_Browser]
    [palyra-connector-discord] -- "WebSocket" --> [Discord_API]
    [palyra-daemon] -- "gRPC (browser.proto)" --> [palyra-browserd]

    style [palyrad] stroke-width:2px
    style [palyra] stroke-width:2px
    style [palyra-browserd] stroke-width:2px
```
**Sources:** [crates/palyra-daemon/Cargo.toml#19-32](http://crates/palyra-daemon/Cargo.toml#19-32), [crates/palyra-cli/Cargo.toml#22-29](http://crates/palyra-cli/Cargo.toml#22-29), [crates/palyra-daemon/build.rs#8-14](http://crates/palyra-daemon/build.rs#8-14)

## Infrastructure and Tooling

The repository includes several top-level directories for maintenance and quality assurance:

*   **scripts/:** Contains shell and PowerShell scripts for protocol generation, release packaging, and security hygiene [justfile#97-106](http://justfile#97-106), [justfile#133-141](http://justfile#133-141).
*   **fuzz/:** Contains `cargo-fuzz` targets for stress-testing parsers, including `a2ui_json_parser`, `webhook_payload_parser`, and `auth_profile_registry_parser` [justfile#154-167](http://justfile#154-167).
*   **infra/:** Configuration for CI/CD pipelines and security scanning.

### Developer Workflow Commands

Project tasks are orchestrated via a `justfile` (and a mirrored `Makefile`). Key targets include:

*   `just doctor`: Runs environment checks to ensure all prerequisites are met [justfile#7-11](http://justfile#7-11).
*   `just dev`: Bootstraps the local development environment, including building the workspace and checking UI readiness [justfile#16-21](http://justfile#16-21).
*   `just protocol`: Validates `.proto` files and regenerates Rust stubs [justfile#103-107](http://justfile#103-107).
*   `just security`: Aggregates `cargo-audit`, `cargo-deny`, and custom pattern scans [justfile#130-136](http://justfile#130-136).

### Crate Dependency Mapping (Natural Language to Code)

This table bridges the conceptual subsystems with their physical implementation in the codebase.

| Subsystem | Code Implementation (Crate/File) | Role |
|:---|:---|:---|
| **Identity Management** | `crates/palyra-identity` | Manages Ed25519 keys and mTLS certs [Cargo.toml#18](http://Cargo.toml#18) |
| **Tool Sandbox** | `crates/palyra-sandbox` | Implements resource-constrained execution [Cargo.toml#11](http://Cargo.toml#11) |
| **Secret Storage** | `crates/palyra-vault` | Interfaces with OS-level secret stores [Cargo.toml#19](http://Cargo.toml#19) |
| **Policy Engine** | `crates/palyra-policy` | Evaluates Cedar policies for tool access [Cargo.toml#14](http://Cargo.toml#14) |
| **Agent UI** | `crates/palyra-a2ui` | Handles JSON-patch based UI updates [Cargo.toml#15](http://Cargo.toml#15) |
| **WASM Runtime** | `crates/palyra-plugins/runtime` | Host for skill execution via `wasmtime` [Cargo.toml#16](http://Cargo.toml#16) |

**Sources:** [Cargo.toml#1-21](http://Cargo.toml#1-21), [justfile#1-171](http://justfile#1-171), [Makefile#1-164](http://Makefile#1-164)
