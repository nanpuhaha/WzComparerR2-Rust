# `wz-gui` is excluded from the Cargo workspace

The desktop GUI depends on the heavy `eframe`/`wgpu`/`rfd` stack and has its own
lockfile. It lives under `crates/wz-gui` but is listed in `workspace.exclude` so the
workspace-wide `cargo deny`, `cargo test`, and `cargo clippy` gates stay focused on
the pure-Rust library and CLI. CI builds the GUI separately with
`--manifest-path crates/wz-gui/Cargo.toml`.

**Consequences**: two lockfiles (`Cargo.lock` and `crates/wz-gui/Cargo.lock`) must be
kept in sync. The GUI is not covered by `cargo test --workspace`.
