# iOS Scaffold

This is the iOS node application scaffold for future client work.

## Mobile convergence plan

The desktop companion currently defines the first production first-party node contract. iOS should
follow the same contract shape rather than inventing a separate one:

- Capability namespace:
  - reuse the same node capability names so inventory, audits, and future policy stay aligned.
- Execution posture:
  - preserve `automatic` vs `local_mediation` semantics for every capability.
- Trust and approvals:
  - keep certificate-bound pairing, approval-backed enrollment, and explicit revoke/recovery flows.
- Product scope:
  - favor a narrow handoff-oriented capability pack first, such as safe URL open or notification
    surfaces, before considering broader device APIs.
- Offline discipline:
  - stale mobile devices must degrade visibly and pending requests must time out or clean up
    instead of hanging indefinitely.

## Lint baseline

- `SwiftLint` configuration lives in `apps/ios/.swiftlint.yml`.
- Recommended local command:
  - `swiftlint lint --config apps/ios/.swiftlint.yml`

No production iOS runtime ships yet; this README documents the compatibility target the desktop
node host now defines.
