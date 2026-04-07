# Palyra Overview

<details>
<summary>Relevant source files</summary>

The following files were used as context for generating this wiki page:

- Cargo.lock
- Cargo.toml
- LICENSE
- README.md
- apps/desktop/README.md
- apps/web/README.md
- crates/palyra-a2ui/Cargo.toml
- crates/palyra-auth/Cargo.toml
- crates/palyra-cli/Cargo.toml
- crates/palyra-cli/build.rs
- crates/palyra-connectors/Cargo.toml
- crates/palyra-daemon/Cargo.toml
- crates/palyra-daemon/build.rs
- crates/palyra-plugins/runtime/Cargo.toml
- deny.toml
- osv-scanner.toml

</details>



Palyra is a high-level AI agent orchestration platform designed for secure, auditable, and extensible interactions between Large Language Models (LLMs) and local or remote environments. The system is built as a modular monorepo, prioritizing a "fail-closed" security model, human-in-the-loop approvals, and a robust plugin architecture.

The platform's primary goal is to provide a unified gateway where agents can execute tools (skills), manage long-term memory, and interact with users across multiple channels (Web, Discord, CLI) while maintaining strict governance over secrets and system resources.

### System Architecture

The Palyra ecosystem consists of several specialized daemons and client applications that communicate via gRPC and REST APIs. At the center is `palyrad`, which orchestrates the lifecycle of agent "runs" and enforces security policies.

#### High-Level Component Interaction
The following diagram illustrates how the core daemons and user interfaces interact.

**Diagram: System Component Map**
```mermaid
graph TD
    subgraph "User Interface Space"
        [WebConsole] -->|"/console/v1"| [palyrad]
        [DesktopApp] -->|"Supervises"| [palyrad]
        [CLI] -->|"gRPC"| [palyrad]
    end

    subgraph "Core Daemon Space"
        [palyrad] -->|"gRPC"| [palyra-browserd]
        [palyrad] -->|"WASM"| [PluginRuntime]
        [palyrad] -->|"SQLite"| [JournalStore]
    end

    subgraph "External Space"
        [palyrad] -->|"HTTPS"| [LLM_Providers]
        [palyrad] -->|"Websockets"| [Discord/Slack]
    end
```
Sources: `crates/palyra-daemon/Cargo.toml:1-55`(), `apps/desktop/README.md:17-39`(), `apps/web/README.md:6-26`()

### Major Components

| Component | Code Entity / Binary | Description |
| :--- | :--- | :--- |
| **Core Daemon** | `palyrad` | The central orchestrator. Manages sessions, tool execution, and the security policy engine. [crates/palyra-daemon/src/bin/palyrad.rs#9-10](http://crates/palyra-daemon/src/bin/palyrad.rs#9-10) |
| **Browser Daemon** | `palyra-browserd` | A headless browser controller providing automation capabilities (Click, Type, Screenshot) to agents. [crates/palyra-browserd/Cargo.toml#2](http://crates/palyra-browserd/Cargo.toml#2) |
| **CLI Tool** | `palyra` | The primary operator interface for configuration, manual overrides, and TUI-based interaction. [crates/palyra-cli/src/bin/palyra.rs#9-10](http://crates/palyra-cli/src/bin/palyra.rs#9-10) |
| **Desktop App** | `palyra-desktop` | A Tauri-based supervisor that manages the lifecycle of `palyrad` and `palyra-browserd`. [apps/desktop/README.md#1-5](http://apps/desktop/README.md#1-5) |
| **Web Console** | `apps/web` | A React dashboard for monitoring agent runs, managing memory, and granting approvals. [apps/web/README.md#1-4](http://apps/web/README.md#1-4) |

### Code-to-System Mapping

Palyra bridges high-level agent concepts (like "Skills" or "Sessions") with specific Rust crates and Protobuf definitions.

**Diagram: Entity Mapping**
```mermaid
graph LR
    subgraph "Natural Language Space"
        [Agent_Skill]
        [Security_Policy]
        [Secret_Vault]
        [Agent_Memory]
    end

    subgraph "Code Entity Space"
        [Agent_Skill] --- [palyra-skills]
        [Security_Policy] --- [palyra-policy]
        [Secret_Vault] --- [palyra-vault]
        [Agent_Memory] --- [JournalStore]
    end

    subgraph "Protocol Space (schemas/proto/palyra/v1/)"
        [palyra-skills] --- [gateway.proto]
        [palyra-policy] --- [common.proto]
        [JournalStore] --- [memory.proto]
    end
```
Sources: `Cargo.toml:1-21`(), `crates/palyra-daemon/build.rs:7-14`(), `crates/palyra-cli/build.rs:7-14`()

### Key Subsystems

*   **Gateway and Session Orchestration**: Handles the `RunStateMachine`, processing inbound messages from channels and routing them through the LLM. For details, see [Gateway and Session Orchestration](../core_daemon_palyrad/gateway_and_session_orchestration/README.md).
*   **Security and Policy Engine**: Uses the Cedar policy language to evaluate whether a tool call or secret access is permitted. For details, see [Security Architecture](../security_architecture/README.md).
*   **Skills and Plugin System**: Executes tools in isolated WASM sandboxes using `wasmtime`. For details, see [Skills and Plugin System](../skills_and_plugin_system/README.md).
*   **Channel Connectors**: Adapters for external platforms like Discord and Slack. For details, see [Channel Connectors](../channel_connectors/README.md).
*   **Browser Automation**: A dedicated service for agent-driven web navigation. For details, see [Browser Automation (palyra-browserd)](../browser_automation_palyra-browserd/README.md).

### Repository and Workspace

The Palyra monorepo is organized as a Cargo workspace containing 18 internal crates and several application directories.

*   **Crates**: Found in `crates/`, these provide modular logic for auth, identity, transport, and more. [Cargo.toml#2-21](http://Cargo.toml#2-21)
*   **Apps**: Found in `apps/`, containing the Web, Desktop (Tauri), and Browser Extension frontends. [apps/desktop/README.md#1-5](http://apps/desktop/README.md#1-5)
*   **Schemas**: Found in `schemas/proto/`, defining the gRPC and Protobuf contracts that ensure type safety across Rust, Kotlin, and Swift. [crates/palyra-daemon/build.rs#7-14](http://crates/palyra-daemon/build.rs#7-14)

For a deep dive into how these files are organized, see [Repository Structure and Workspace Layout](repository_structure_and_workspace_layout/README.md).

### Development and Setup

Palyra uses a custom toolchain and bootstrap process to ensure consistent environments across Linux, macOS, and Windows. Developers primarily interact with the system via `just` or `make` targets and the `palyra doctor` diagnostic command.

For instructions on setting up your local environment, see [Getting Started and Developer Workflow](getting_started_and_developer_workflow/README.md).

Sources: `Cargo.toml:1-84`(), `apps/desktop/README.md:102-158`(), `apps/web/README.md:43-59`()

## Child Pages

- [Repository Structure and Workspace Layout](repository_structure_and_workspace_layout/README.md)
- [Getting Started and Developer Workflow](getting_started_and_developer_workflow/README.md)
