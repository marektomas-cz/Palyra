# glib Dependency Graph (desktop `src-tauri`)

Captured on 2026-03-03 after enabling downstream patch override for `glib 0.18.5`.

## Commands

```bash
cargo tree --manifest-path apps/desktop/src-tauri/Cargo.toml --target all -i glib
cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --format-version 1 --locked
```

## `cargo tree` evidence

```text
glib v0.18.5 (<repo-root>/apps/desktop/src-tauri/third_party/glib-0.18.5-patched)
├── atk v0.18.2
│   └── gtk v0.18.2
│       └── tauri v2.10.2
│           └── palyra-desktop-control-center v0.1.0
...
```

## `cargo metadata` evidence

```text
glib.id=path+file:///<repo-root>/apps/desktop/src-tauri/third_party/glib-0.18.5-patched#glib@0.18.5
glib.source=
glib.manifest_path=<repo-root>/apps/desktop/src-tauri/third_party/glib-0.18.5-patched/Cargo.toml
```

`<repo-root>` is intentionally redacted to avoid publishing machine-local filesystem details.

The `path+file://` package id confirms that the desktop crate resolves `glib` from the local patched source, not from crates.io.
