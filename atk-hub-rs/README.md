# atk-hub-rs

![ATK HUB logo](../assets/atk-hub-logo.png)

Rust port of ATK HUB 3.2.31. A native eframe/egui GUI over five behavior-ported
core crates, with a headless CLI for verification. See [PORTING.md](PORTING.md)
for the API/test mapping, deliberate differences and preserved quirks.

## Layout

```
crates/asar       typed Electron ASAR manifest reader
crates/config     electron-store compatible config.json access
crates/endpoints  recovered remote URL templates
crates/ipc        169 literal IPC channels + agent method allowlist
crates/agent      named-pipe protocol (frame codec, limits, pipe naming)
crates/gui        full eframe/egui GUI (binary: atk-hub-rs)
crates/wallpaper  live wallpaper catalogue client
crates/audio      THX gRPC client generated from the shipped proto
crates/features   logic for lighting, macros, music, pets, plugins, updater, drivers
crates/input      typed Win32 macro key injection
crates/cli        headless CLI (binary: atk-hub-cli)
```

## Run the GUI

```
cargo run -p atk-gui --bin atk-hub-rs
```

It opens at 1280x860 with 16 pages: Overview, ASAR, Config, Endpoints, IPC,
Agent, Devices, Wallpapers, Audio, Lighting, Macros, Music, Pets, Plugins,
Updater and Drivers.
To verify without opening a window:

```
cargo run -p atk-gui --bin atk-hub-rs -- --self-check
```

## Run the CLI

```
cargo run -p atk-cli --bin atk-hub-cli -- asar "C:\Program Files\ATK HUB\resources\app.asar"
cargo run -p atk-cli --bin atk-hub-cli -- config "C:\Users\Admin\AppData\Roaming\ATK-HUB\config.json"
cargo run -p atk-cli --bin atk-hub-cli -- ipc
cargo run -p atk-cli --bin atk-hub-cli -- encode 7 device.list
cargo run -p atk-cli --bin atk-hub-cli -- decode '{"id":3,"ok":true,"result":[]}'
```

## Installer (no license or third-party tooling required)

`setup.exe` is a small Rust installer built from the standard library only.

```
powershell -File installer\build-package.ps1        # builds installer-package\ and atk-hub-rs-portable.zip
installer-package\setup.exe                         # installs to %LOCALAPPDATA%\AtkVhub with a Start Menu shortcut
installer-package\setup.exe --no-shortcut --target C:\Some\Folder
installer-package\setup.exe --uninstall
```

The portable ZIP contains `setup.exe`, `atk-hub-rs.exe`, `atk-hub-cli.exe` and
the docs. Installation is per-user, so no elevation or product key is needed;
the optional Windows service registration remains a separate explicit command
(`atk-hub-cli service install --app <path>`).

## Verify

```
cargo fmt --all
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Devices page can connect/disconnect a HID device, write an output report,
clear rules, set bindings and read typed agent state through the live pipe.

Current result: 60 tests pass, clippy clean, GUI self-check renders all 16 pages.
Macro key injection, verified plugin/driver installation and the THX mock-server
validation are covered; no test sends keystrokes or changes system state.
