# ATK HUB reverse engineering + Rust port

![ATK HUB logo](assets/atk-hub-logo.png)

This workspace contains the reverse-engineering report for the installed
ATK HUB 3.2.31 application and a behavior-ported Rust implementation with a
Tauri desktop shell and an HTML/CSS frontend.

- [ATK_HUB_reverse_engineering_report.md](ATK_HUB_reverse_engineering_report.md)
  — findings from static analysis of the installed Electron app.
- [re_artifacts/](re_artifacts/) — extracted entry files used during analysis.
- [atk-hub-rs/](atk-hub-rs/) — the Rust workspace: GUI, core crates, CLI,
  license-free installer, tests and [porting notes](atk-hub-rs/PORTING.md).

## Quick start

```
cd atk-hub-rs
cargo test --workspace
cargo run -p atk-gui --bin atk-hub-rs
```

See [atk-hub-rs/README.md](atk-hub-rs/README.md) for details and installer usage.
