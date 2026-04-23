# phf_generator 0.8.0 patch

This is a narrow local patch for `phf_generator` 0.8.0 used by the desktop
Tauri dependency graph through `selectors` 0.24.0.

The patch keeps the crate name, version, and public API stable while changing
the internal `rand` dependency from the vulnerable 0.7 line to `rand` 0.8.6.
Remove this patch when the upstream `kuchikiki`/`selectors` path no longer
requires `phf_generator` 0.8.0.
