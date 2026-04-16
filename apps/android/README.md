# Android Scaffold

This is the Android mobile companion scaffold for the current release work.

## Mobile convergence plan

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

## Scaffold files

- `apps/android/app/src/main/kotlin/io/palyra/mobile/companion/CompanionContracts.kt`
- `apps/android/app/src/main/kotlin/io/palyra/mobile/companion/CompanionStore.kt`
- `apps/android/app/src/main/kotlin/io/palyra/mobile/companion/CompanionShell.kt`

These files define the first mobile-safe shell state, local store contract, and release scope for:

- approvals inbox
- polling notifications
- recent sessions and handoff
- safe URL open
- voice notes with transcript review

## Lint baseline

- Detekt baseline config: `apps/android/config/detekt/detekt.yml`
- Optional local lint command:
  - `bash apps/android/scripts/lint.sh`

No production Android runtime ships yet. This scaffold exists to keep the mobile companion aligned
with the same auth, audit, and revoke/recovery semantics as web and desktop from the first commit.
