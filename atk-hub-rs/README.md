# atk-hub-rs

![ATK HUB logo](../assets/atk-hub-logo.png)

Rust port of ATK HUB 3.2.31. A Tauri 2 desktop shell with an HTML/CSS frontend
over behavior-ported core crates, with a headless CLI for verification. See [PORTING.md](PORTING.md)
for the API/test mapping, deliberate differences and preserved quirks.

## Layout

```
crates/asar       typed Electron ASAR manifest reader
crates/config     electron-store compatible config.json access
crates/endpoints  recovered remote URL templates
crates/ipc        169 literal IPC channels + agent method allowlist
crates/agent      named-pipe protocol (frame codec, limits, pipe naming)
crates/gui        Tauri shell and bundled web frontend (binary: atk-hub-rs)
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

It opens at 1280x860 with 13 pages in three groups: Device Center, Devices,
Light Show, Macros, Audio, Background Theme, Music Score Tool, Desktop Pets,
Plugin Market, Update, Drivers, Settings and Diagnostics. The static frontend
is loaded from `crates/gui/frontend`, and Rust exposes the core crates through
Tauri commands.
To verify without opening the app window:

```
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
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

The normal installer is NSIS-based, like SlickShot's Windows setup. It has a
welcome page, a destination folder page, a finish page, a Start Menu shortcut,
and an Add or Remove Programs entry. It installs per user with no UAC prompt.

```
powershell -File installer\build-package.ps1                    # stages binaries
powershell -File installer\build-nsis.ps1 -Version 0.1.5        # builds ATK-HUB-setup.exe
ATK-HUB-setup.exe /S /D=C:\Some\Folder                          # silent install
uninstall.exe /S                                                # silent uninstall
```

A single-file `setup.exe` is also produced by `build-package.ps1`. It embeds the
package and works without the ZIP, but it is not the standard wizard.

Installation needs no license key; the optional Windows service registration
remains a separate explicit command (`atk-hub-cli service install --app <path>`).

## Verify

```
cargo fmt --all
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Devices page can connect/disconnect a HID device, write an output report,
clear rules, set bindings and read typed agent state through the live pipe.

Current result: 60 tests pass, clippy clean, and the frontend renders cleanly
at 1440, 1024, 768 and 390 pixels.
Macro key injection, verified plugin/driver installation and the THX mock-server
validation are covered; no test sends keystrokes or changes system state.
