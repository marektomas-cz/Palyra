# Android Scaffold

This is the Android node application scaffold for future client work.

## Mobile convergence plan

The current Android path remains at the contract-planning stage. When it grows into a production
client, it should reuse the same high-level node model instead of forking behavior:

- Capability naming:
  - keep the same node capability namespace so desktop and Android stay inventory-compatible.
- Execution posture:
  - preserve the distinction between `automatic` and `local_mediation` capability execution.
- Pairing and trust:
  - keep mTLS + approval-bound device pairing exactly as on desktop; Android is not a shortcut.
- Capability scope:
  - start with mobile-safe equivalents of notification, URL open, and explicit handoff-style
    actions before any broader device control.
- Offline behavior:
  - mobile clients must surface offline state explicitly and must not leave pending capability
    requests appearing active after the device disappears.

## Lint baseline

- Detekt baseline config: `apps/android/config/detekt/detekt.yml`
- Optional local lint command:
  - `bash apps/android/scripts/lint.sh`

No production Android app runtime code is shipped yet; this README only captures the convergence
contract the desktop node host currently defines.
