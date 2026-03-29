# Browser Service v1 (`palyra-browserd`)

Purpose: define the runtime contract for the isolated browser boundary that backs `palyra browser`
and the daemon browser broker.

## Runtime surface

`palyra-browserd` exposes:

- `GET /healthz` for health checks.
- gRPC `palyra.browser.v1.BrowserService` with:
  - `Health`
  - `CreateSession`
  - `CloseSession`
  - `Navigate`
  - `Click`
  - `Type`
  - `Scroll`
  - `WaitFor`
  - `GetTitle`
  - `Screenshot`
  - `Observe`
  - `NetworkLog`
  - `ResetState`
  - `ListTabs`
  - `OpenTab`
  - `SwitchTab`
  - `CloseTab`
  - `GetPermissions`
  - `SetPermissions`
  - `ListProfiles`
  - `CreateProfile`
  - `RenameProfile`
  - `DeleteProfile`
  - `SetActiveProfile`
  - `RelayAction`
  - `ListDownloadArtifacts`

## Operator surfaces

- `palyra browser status` reports endpoint wiring, auth posture, timeout caps, and artifact limits.
- `palyra browser session ...` manages browser sessions with bounded budgets and structured output.
- `palyra browser profiles ...` manages persisted browser profiles when encrypted persistence is
  enabled.
- `palyra browser tabs ...` and page-action commands expose bounded navigation and DOM interaction
  primitives.
- `/console/v1/browser/*` keeps the same brokered browser contract for the web console.

## Security posture

- Download-like actions remain deny-by-default and must be enabled per session.
- Browser targets stay behind strict scheme, DNS, and private-network guardrails.
- Relay tokens remain short-lived and scoped to principal, session, and extension identity.
- Screenshot, title, observe, and action-log payloads stay byte-bounded.
- Encrypted profile persistence requires explicit opt-in and an encryption key.

## Persistence model

- Sessions are ephemeral by default.
- Profiles may persist state only when the operator enables persistence and provides
  `PALYRA_BROWSERD_STATE_ENCRYPTION_KEY`.
- Private profiles do not write persisted snapshots to disk.
- Persisted state and profile metadata use integrity checks and fail-closed restore validation.

## Release expectations

- Portable packages must bundle `palyra-browserd` next to `palyrad` and `palyra`.
- Packaged help and docs must keep `palyra browser` discoverable even when some browser subfeatures
  intentionally remain unsupported placeholders.
- Release smoke covers packaged browser help output and bundled browser-service documentation.
