# CLI Parity Migration v1

Purpose: give existing operators a clear migration path from older or compatibility command names
to the preferred Palyra CLI surface used in packaging, release notes, and current examples.

## Migration posture

- Preferred names are the ones used by packaged install guidance, release smoke, and current help
  examples.
- Compatibility aliases remain supported where implemented and remain covered by the CLI parity
  matrix.
- Alias support exists for migration safety. It is not a second product surface.

## Primary naming shifts

- Bootstrap and first-touch setup:
  - Prefer `palyra setup`.
  - `palyra init` remains a compatibility alias for older scripts.
- Runtime and admin workflows:
  - Prefer `palyra gateway`.
  - `palyra daemon` remains a compatibility alias.
- Guided onboarding:
  - Prefer `palyra onboarding wizard`.
  - `palyra onboard` remains a shorthand compatibility alias.

## Other compatibility aliases worth preserving

- `palyra channels discord verify` keeps `test-send` as a compatibility alias.
- `palyra cron update` keeps `edit` as a compatibility alias.
- `palyra cron delete` keeps `rm` as a compatibility alias.
- `palyra cron logs` keeps `runs` as a compatibility alias.
- `palyra memory index` keeps `reindex` as a compatibility alias.
- `palyra skills` keeps `skill` as a compatibility alias.

## Release-ready examples

- Headless bootstrap:
  - `palyra setup --mode remote --path ./palyra.toml --force`
- Guided remote onboarding:
  - `palyra onboarding wizard --flow remote --path ./palyra.toml`
- Runtime status after install:
  - `palyra gateway status`
- Offline migration lookup:
  - `palyra docs search migration`
- Packaged update review:
  - `palyra update --install-root <install-root> --archive <zip> --dry-run`
- Packaged uninstall review:
  - `palyra uninstall --install-root <install-root> --dry-run`

## Packaging guidance

- Portable desktop and headless packages bundle these migration notes plus CLI help snapshots, so
  operators can inspect migration guidance without a source checkout.
- Existing automation that still uses `init` or `daemon` does not need an emergency rewrite for
  this release.
- New automation and new docs should move to `setup`, `gateway`, and `onboarding wizard`.

## Intentional parity transparency

- Browser placeholder subfeatures such as `browser console`, `browser pdf`, `browser select`, and
  `browser highlight` remain intentionally discoverable placeholders until their implementations
  land.
- Windows and macOS remain the supported v1 desktop runtime targets.
- The Linux desktop bundle remains a packaging and regression artifact while the desktop runtime
  path stays disabled there.
