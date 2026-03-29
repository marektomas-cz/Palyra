# Release Engineering v1

Purpose: define the current release contract for portable desktop and headless artifacts, including
the migration-safe CLI parity handoff.

## Shipping targets

- Desktop portable bundles for Windows, macOS, and Linux.
- Supported desktop runtime targets in v1: Windows and macOS.
- Linux desktop bundle remains a packaging and regression artifact while the Linux desktop runtime
  path stays disabled because of the Tauri `glib` dependency constraint.
- Headless portable package remains the primary Linux shipping target.

## Artifact contents

### Desktop portable bundle

Archive contents must include:

- `palyra-desktop-control-center`
- `palyrad`
- `palyra-browserd`
- `palyra`
- `LICENSE.txt`
- `README.txt`
- `ROLLBACK.txt`
- `RELEASE_NOTES.txt`
- `MIGRATION_NOTES.txt`
- `release-manifest.json`
- `checksums.txt`
- `docs/`
- `docs/help_snapshots/`

### Headless portable package

Archive contents must include:

- `palyrad`
- `palyra-browserd`
- `palyra`
- `LICENSE.txt`
- `README.txt`
- `ROLLBACK.txt`
- `RELEASE_NOTES.txt`
- `MIGRATION_NOTES.txt`
- `release-manifest.json`
- `checksums.txt`
- `docs/`
- `docs/help_snapshots/`

## Lifecycle contract

- `install-desktop-package.ps1` and `install-headless-package.ps1` expose `palyra` through a
  user-scoped command root and record install metadata for uninstall cleanup.
- `palyra update --archive <zip> --dry-run` and `palyra uninstall --dry-run` are part of the
  release-ready package lifecycle surface.
- Headless upgrades require `palyra config migrate --path <config>` after unpacking a new archive.
- Packaged docs must remain available offline through `palyra docs`.

## CLI parity release coverage

Installed package smoke must verify:

- canonical lifecycle surfaces: `setup`, `onboarding wizard`, `gateway`
- migration-safe aliases: `init`, `onboard`, `daemon`
- broader packaged operator surface: `browser`, `node`, `nodes`, `docs`, `update`, `uninstall`,
  `support-bundle`, and `doctor`
- packaged docs lookups for migration guidance, ACP bridge notes, and browser-service docs

## Release automation

- `.github/workflows/ci.yml` must keep release packaging smoke on Windows, macOS, and Linux.
- `.github/workflows/release.yml` must keep archive SHA256 sidecars, release manifests, provenance
  sidecars, and GitHub build attestations.
- Main CI must stay green before final publication.

## Packaging boundaries

Portable archives must exclude:

- runtime databases and journal files
- logs
- support-bundle exports
- browser profiles and download sandboxes
- `node_modules`
- transient web build leftovers outside the packaged dashboard payload
