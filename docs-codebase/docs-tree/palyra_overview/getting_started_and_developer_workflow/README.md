# Getting Started and Developer Workflow

<details>
<summary>Relevant source files</summary>

The following files were used as context for generating this wiki page:

- .github/dependabot.yml
- .gitignore
- Makefile
- SECURITY.md
- apps/web/src/console/hooks/useOverviewDomain.ts
- apps/web/src/console/sections/ConfigSection.tsx
- apps/web/src/console/sections/OperationsSection.tsx
- apps/web/src/console/sections/OverviewSection.tsx
- apps/web/src/console/sections/SupportSection.tsx
- apps/web/src/console/sections/UsageSection.tsx
- apps/web/src/console/sections/access/AccessControlWorkspace.tsx
- crates/palyra-cli/src/args/auth.rs
- crates/palyra-cli/src/commands/doctor.rs
- crates/palyra-cli/src/commands/security.rs
- crates/palyra-cli/tests/cli_parity_matrix.toml
- crates/palyra-cli/tests/cli_parity_report.md
- crates/palyra-cli/tests/help_snapshots/auth-help.txt
- infra/ci/security.yml
- justfile
- scripts/check-local-only-tracked-files.sh
- scripts/check-no-vendored-artifacts.sh
- scripts/dev/bootstrap.ps1
- scripts/dev/bootstrap.sh
- scripts/run-pre-push-checks.sh

</details>



This page describes the local development environment setup, the tools used for day-to-day engineering, and the automated gates that ensure system integrity before code is committed or pushed.

## Environment Prerequisites

The Palyra codebase is a polyglot monorepo requiring several toolchains to be present on the host system.

| Tool | Version / Requirement | Purpose |
| :--- | :--- | :--- |
| **Rust** | `1.91.0` (pinned in CI) | Core daemon, CLI, and WASM plugins. |
| **Node.js** | `>= 18.x` | Web Console and Desktop UI (React/Vite). |
| **Just** | Latest | Preferred command runner for task automation. |
| **Protoc** | Latest | Protobuf compilation for gRPC and cross-platform stubs. |

**Sources:** [justfile#19](http://justfile#19), [Makefile#40](http://Makefile#40), [scripts/run-pre-push-checks.sh#34](http://scripts/run-pre-push-checks.sh#34)

---

## Local Setup and Bootstrapping

The repository provides automated scripts to prepare the environment. The primary entry point for new developers is the `doctor` command, followed by the `dev` target.

### 1. The 'palyra doctor' Command
The `doctor` command, implemented in `crates/palyra-cli/src/commands/doctor.rs`, performs a comprehensive audit of the local environment [crates/palyra-cli/src/commands/doctor.rs#4-5](http://crates/palyra-cli/src/commands/doctor.rs#4-5). It checks for:
*   **Config Validity:** Ensures `palyra.toml` exists and is parsable [crates/palyra-cli/src/commands/doctor.rs#34-38](http://crates/palyra-cli/src/commands/doctor.rs#34-38).
*   **Identity Store:** Verifies the local identity root is writable [crates/palyra-cli/src/commands/doctor.rs#40-44](http://crates/palyra-cli/src/commands/doctor.rs#40-44).
*   **Connectivity:** Probes the local daemon's HTTP and gRPC endpoints [crates/palyra-cli/src/commands/doctor.rs#46-51](http://crates/palyra-cli/src/commands/doctor.rs#46-51).
*   **Sandbox Readiness:** Validates if the host supports Tier B (rlimit) and Tier C (bwrap/sandbox-exec) isolation [crates/palyra-cli/src/commands/doctor.rs#117-121](http://crates/palyra-cli/src/commands/doctor.rs#117-121).

### 2. Just / Make Targets
Palyra supports both `just` and `make` for common tasks.

*   `just dev`: Runs the doctor, ensures the desktop UI is built, and compiles the entire workspace [justfile#16-21](http://justfile#16-21).
*   `just web-bootstrap`: Initializes the JS workspace using `vp install` [justfile#30-31](http://justfile#30-31).
*   `just protocol`: Validates `.proto` definitions and regenerates Rust/Kotlin/Swift stubs [justfile#103-107](http://justfile#103-107).

### Local Setup Flow
The following diagram illustrates the bootstrap sequence from a clean clone to a running system.

**Title: Developer Bootstrap Flow**
```mermaid
graph TD
    "Clone"["Git Clone"] --> "Bootstrap"["scripts/dev/bootstrap.sh"]
    "Bootstrap" --> "Doctor"["palyra doctor --strict"]
    "Doctor" -- "Success" --> "Build"["just build"]
    "Doctor" -- "Failure" --> "Remediation"["Doctor Hints/Remediation"]
    "Remediation" --> "Doctor"
    "Build" --> "Web"["just web-bootstrap"]
    "Web" --> "Ready"["Local Environment Ready"]

    subgraph "Code Entities"
        "Doctor" -.-> "run_doctor()"["crates/palyra-cli/src/commands/doctor.rs:run_doctor"]
        "Build" -.-> "cargo_build"["justfile:112"]
        "Web" -.-> "vp_install"["justfile:30"]
    end
```
**Sources:** [justfile#7-22](http://justfile#7-22), [crates/palyra-cli/src/commands/doctor.rs#3-10](http://crates/palyra-cli/src/commands/doctor.rs#3-10)

---

## Developer Workflow

The workflow is centered around high-frequency testing and strict hygiene checks.

### Building and Testing
*   **Workspace Build:** `cargo build --workspace --locked` [justfile#114](http://justfile#114).
*   **Workspace Test:** `cargo test --workspace --locked` [justfile#110](http://justfile#110).
*   **CLI Regression:** `bash scripts/test/run-workflow-regression.sh` [justfile#62-63](http://justfile#62-63).
*   **Fuzzing:** Fuzz targets for parsers (JSON, Webhooks, Config) can be built via `just fuzz-build` [justfile#154-167](http://justfile#154-167).

### Protocol Management
Palyra uses a "Schema First" approach. When modifying gRPC or message structures:
1.  Edit files in `schemas/proto/palyra/v1/`.
2.  Run `just protocol-validate` to ensure no breaking changes [justfile#97-98](http://justfile#97-98).
3.  Run `just protocol-generate` to update generated code in all languages [justfile#100-101](http://justfile#100-101).

---

## Pre-Push Gate

Before pushing to the remote, developers are expected to run the pre-push suite. This is orchestrated by `scripts/run-pre-push-checks.sh` and supports two profiles [scripts/run-pre-push-checks.sh#5](http://scripts/run-pre-push-checks.sh#5):

### 1. Fast Profile (`fast`)
Optimized for speed, focusing on formatting and deterministic core tests:
*   `rustfmt` and `clippy` (basic) [scripts/run-pre-push-checks.sh#52-53](http://scripts/run-pre-push-checks.sh#52-53).
*   **Artifact Hygiene:** Checks that no temporary runtime artifacts (logs, SQLite DBs) are tracked in git [scripts/run-pre-push-checks.sh#55-56](http://scripts/run-pre-push-checks.sh#55-56).
*   **Deterministic Core:** Runs `scripts/test/run-deterministic-core.sh` to verify the state machine without external I/O [scripts/run-pre-push-checks.sh#64-65](http://scripts/run-pre-push-checks.sh#64-65).

### 2. Full Profile (`full`)
Required for significant changes or before opening a PR:
*   Full workspace `cargo test` [scripts/run-pre-push-checks.sh#89-90](http://scripts/run-pre-push-checks.sh#89-90).
*   Workflow regression matrix [scripts/run-pre-push-checks.sh#92-93](http://scripts/run-pre-push-checks.sh#92-93).
*   Protocol stub validation [scripts/run-pre-push-checks.sh#95-98](http://scripts/run-pre-push-checks.sh#95-98).
*   High-risk pattern scanning (e.g., searching for unredacted secrets or unsafe blocks) [scripts/run-pre-push-checks.sh#100-101](http://scripts/run-pre-push-checks.sh#100-101).

**Title: Pre-Push Validation Pipeline**
```mermaid
flowchart LR
    "Start"["just push-gate-fast"] --> "JS"["run_js_workspace_checks()"]
    "JS" --> "Fmt"["cargo fmt --check"]
    "Fmt" --> "Hygiene"["check-runtime-artifacts.sh"]
    "Hygiene" --> "Deterministic"["run-deterministic-core.sh"]
    "Deterministic" --> "Risk"["check-high-risk-patterns.sh"]
    "Risk" --> "Success"["Gate Passed"]

    subgraph "Hygiene Checks"
        "Hygiene" -.-> "GitIgnore"["Check .gitignore patterns"]
        "Risk" -.-> "Scanner"["Gitleaks / Pattern Matcher"]
    end
```
**Sources:** [scripts/run-pre-push-checks.sh#49-69](http://scripts/run-pre-push-checks.sh#49-69), [.gitignore#49-56](http://.gitignore#49-56)

---

## Security Auditing

Developers can run local security audits using `palyra security audit`. This command leverages the same logic as the `doctor` but focuses on deployment risks [crates/palyra-cli/src/commands/security.rs#68-70](http://crates/palyra-cli/src/commands/security.rs#68-70).

The audit identifies:
*   **Admin Auth:** Whether `admin.require_auth` is disabled [crates/palyra-cli/src/commands/security.rs#152-160](http://crates/palyra-cli/src/commands/security.rs#152-160).
*   **TLS Risks:** Remote binds without gateway TLS [crates/palyra-cli/src/commands/security.rs#169-177](http://crates/palyra-cli/src/commands/security.rs#169-177).
*   **Secret Exposure:** Inline API keys in config files instead of `VaultRef` [crates/palyra-cli/src/commands/security.rs#63](http://crates/palyra-cli/src/commands/security.rs#63).

**Sources:** [crates/palyra-cli/src/commands/security.rs#134-185](http://crates/palyra-cli/src/commands/security.rs#134-185), [justfile#130-136](http://justfile#130-136)
