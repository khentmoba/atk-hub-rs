# PORTING.md — ATK HUB → Rust

Port target: **ATK HUB 3.2.31** as installed at `C:\Program Files\ATK HUB`.
The port is behavior-first, not line-by-line. No original source tests exist, so
every Rust test is a characterization test derived from static analysis of the
shipped artifacts (REA MCP evidence + ASAR/main-bundle extraction).

## API mapping

| Original behavior (JS / Electron / Rust service) | Rust equivalent |
|---|---|
| ASAR 16-byte prefix, JSON manifest, payload offset | `atk_asar::Manifest::parse`, `Manifest::data_offset` |
| Manifest directory/file tree, ordered JSON keys, `size`/`offset`/`unpacked`/`executable` | `atk_asar::{Node, FileEntry, Manifest}` with `preserve_order` JSON |
| `app.asar.unpacked` entries (no size/offset in manifest) | `FileEntry` with `size = 0`, `offset = 0`, `unpacked = true` |
| `config.json` via electron-store: dot-notation `get`/`set`, `JSON.stringify(v, undef, "\t")` | `atk_config::{Config, ConfigError}` with `get`, `get_str`, `set`, `to_json` |
| Firmware/update/wallpaper/service URL literals | `atk_endpoints::{firmware_url, firmware_service_url, device_update_version_url, …}` |
| 169 literal `ipcMain.handle/on` channel names | `atk_ipc::all`, `find`, `by_category`, `Category` |
| Rust-agent allowlist `kN` | `atk_ipc::{AGENT_METHODS, is_agent_method}` |
| `^atk-hub-agent-session-\d+$` pipe discovery/validation, `\\.\pipe\…` | `atk_agent::PipeName::parse`, `PipeName::windows_path` |
| `JSON.stringify({id,method,params: p ?? null}) + "\n"`, 256 KiB cap | `atk_agent::Request::encode`, `MAX_FRAME_BYTES` |
| Response `{id, ok, result, error}` and `{event, …}` frames, newline buffering | `atk_agent::{Frame, FramedBuffer}` |
| 64 pending requests, 10 s request timeout | `atk_agent::{MAX_PENDING, REQUEST_TIMEOUT}` |
| Vue renderer / Electron windows | `atk_gui::HubApp`, eframe/egui native GUI |
| Headless inspection of the port | `atk-hub-cli` (asar, config, ipc, endpoints, agent frames) |

## Test mapping

| Characterization (from the original) | Rust test |
|---|---|
| ASAR prefix framing, manifest order, data offset | `parser::parses_manifest_order_and_data_offset` |
| String and numeric size/offset, executable and unpacked flags | `parser::reads_file_flags_and_numeric_or_string_fields` |
| Paths with `./` and leading `/`, directories are not files | `parser::lookup_handles_prefix_separators_and_missing_paths` |
| Depth-first path listing and file counts | `parser::counts_files_and_returns_depth_first_paths` |
| Unpacked entries omit size/offset | `parser::unpacked_entries_without_size_or_offset_are_accepted` |
| Short/bad prefix, out-of-bounds JSON, missing `files` | `parser::rejects_truncated_header`, `rejects_header_beyond_input`, `rejects_json_beyond_data_offset`, `rejects_missing_files_and_invalid_nodes` |
| electron-store dot notation and typed config keys | `config::reads_typed_keys_with_dot_notation`, `config::set_service_url_updates_only_that_key` |
| Tab-indented JSON, no trailing newline, unknown keys preserved | `config::serialization_matches_electron_store_formatting`, `config::round_trip_preserves_unknown_fields` |
| Exact URL templates and decimal id interpolation | `urls::*` (3 tests) |
| 169 channels, uniqueness, categories, exact allowlist | `registry::*` (4 tests) |
| Frame byte layout, response/error/event decoding, split reads | `protocol::*` (5 tests) |
| Pipe-name pattern and limits | `protocol::pipe_name_validation_matches_agent_discovery`, `protocol::protocol_limits_are_stable` |
| GUI renders all six pages without a window | `ui::every_page_renders_offscreen_without_a_window`, `ui::fresh_app_starts_on_the_overview_page` |

## Deliberate differences

1. **No Electron/Chromium runtime.** The GUI is native eframe/egui. There is no
   DOM, no Node integration, and no runtime `ipcRenderer` calls. The IPC list is
   a static reference, not a live message bus.
2. **No live named-pipe transport in this milestone.** The agent crate ports the
   wire format, limits and pipe naming; it does not open a pipe to the running
   `atk-hub-service`. Device I/O, HID, audio, wallpaper and driver operations
   remain future milestones.
3. **ASAR framing scope.** Only the 16-byte prefix used by this build is
   accepted; other ASAR header variants are out of scope and reported as errors.
4. **Unpacked entries** have no archive payload, so their `size`/`offset` are
   modeled as `0`; the companion `app.asar.unpacked` file is not opened.
5. **Config writes** create missing intermediate objects and reject a write that
   would traverse a non-object segment, instead of silently replacing it.
6. **serde_json `preserve_order`** is enabled so JSON object iteration order
   matches the original JavaScript object order; a plain `BTreeMap` would sort
   keys and change output order.
7. **Rust 1.93 / egui 0.35.** eframe 0.37 requires Rust 1.95, so the newest
   compatible GUI toolkit release is used. Its API differs from newer egui
   releases (root `Ui` panels and `App::ui`).

## Preserved quirks

- The misspelled channel `tary-menu-list` is kept verbatim.
- All 169 literal channels are kept in source order, including the ambiguous
  `audio-diagnostic` event and the `musicRhythm_*` underscore naming style.
- The frame terminator is `\n` (not NUL); params default to JSON `null`.
- Frame limit 256 KiB (262,144 bytes), 64 in-flight requests, 10 s timeout.
- Config serialization uses tab indentation and no trailing newline.
- IDs in firmware URLs are interpolated as decimal numbers, matching JS.
- Dynamically named `ipcMain.handle(variable, …)` registrations are intentionally
  absent: they cannot be recovered statically.

## Verification

Commands run from `C:\APPLICATIONS\AtkVhub\atk-hub-rs`:

```
cargo fmt --all                                                  # clean
cargo build --workspace                                          # finished, 0 errors
cargo test --workspace                                           # 60 passed (39 suites)
cargo clippy --workspace --all-targets --all-features -- -D warnings  # no issues
cargo run --quiet -p atk-gui --bin atk-hub-rs -- --self-check    # 16 pages rendered offscreen
```

Real-artifact smoke test with `atk-hub-cli`:

```
asar "C:\Program Files\ATK HUB\resources\app.asar"
  -> files: 3630, data offset: 0xe6d24, first path package.json
config "C:\Users\Admin\AppData\Roaming\ATK-HUB\config.json"
  -> user id, service url and UI version printed
encode 7 device.list -> {"id":7,"method":"device.list","params":null}\n
decode '{"id":3,"ok":true,"result":{"devices":[]}}' -> response id=3 ok=true …
ipc -> total: 169
discover        -> \\.\pipe\atk-hub-agent-session-1
live device.list -> live HID list from the running ATK HUB agent (VIDs/PIDs/products)
service status   -> installed
service auto-launch-status -> disabled
wallpapers -> 40 live catalogue entries from wallpaper.atkgear.com
device-state -> AgentState { connected: false, device: None, pid: 4808, … }
audio config  -> Io(NotFound): no THX endpoint file on this host (graceful error)
update        -> version 3.2.33, newer than 3.2.31, installer URL and size
driver        -> version 1.1.2, file https://bpcdn.atkgear.com/hub-cloud/file/...zip
```

## Completed in the second pass

- **Live agent transport**: `atk_agent::AgentClient` connects to the real
  `\\.\pipe\atk-hub-agent-session-N`, correlates responses by request id,
  forwards `{event, …}` frames to subscribers, enforces the method allowlist
  and the 64-request/10-second limits, and shuts the reader thread down on drop.
  Verified end-to-end against a local pipe server (`tests/live.rs`) and against
  the running ATK HUB service (`live device.list` returned real HID devices).
- **Service CLI**: `atk-hub-cli service <action>` wraps install/uninstall/
  start/stop/status and the three auto-launch actions of the shipped
  `atk-hub-service.exe`; read-only actions were exercised successfully.
- **Live Devices page**: the eframe GUI now has eight pages; Devices shows the
  live HID interface list from `device.list`, and the Agent page can connect,
  disconnect and send live requests.
- **Live wallpaper client**: `atk-wallpaper` downloads and parses
  `list-v2.json` (40 entries at verification time), tolerating the mixed
  string/number `id` and `version` scalars found in real data; the GUI has a
  Wallpapers page with search and catalogue refresh.
- **Typed HID control**: `atk_agent::{DeviceId, Rule, RuleAction, AgentState}`
  and `AgentClient::{connect_device, disconnect_device, write_report, set_rules,
  set_bindings, state}`. Parameter shapes came from the agent's own validation
  errors (probed safely: `vid`/`pid` required, report is a byte sequence, rules
  are a sequence of `{name, packet, action:{type,target}}`, bindings need
  `{device, scope, rules}`). Local validation rejects empty/oversized reports
  and malformed scopes before any pipe traffic. The GUI Devices page exposes
  all six operations; `DeviceState` was verified against the live agent.
  Probe-induced rules and device selection were cleared afterwards, restoring
  `state.get` to its original `{connected:false, device:null, rules:[]}`.

- **THX audio client**: `atk-audio` generates a typed gRPC client from the
  shipped `thx_audio_properties.proto` (tonic 0.13 + vendored protoc), reads
  `C:\ProgramData\thx\<product>\thx_properties.tcp`, validates the loopback
  address and PEM certificate as the original does, and exposes `version`,
  `endpoints`, `properties`, `property`, `set_bool`, `presets`,
  `selected_preset`, `select_preset` and aggregated `status`. The GUI has an
  Audio page with status read and spatial on/off actions; `atk-hub-cli audio`
  exposes the same operations. This host has no THX endpoint file, so the live
  RPC path could not be exercised here; generated messages, connection
  validation and pre-connect rejection are covered by four tests.

- **Feature pages**: `atk-features` backs seven new eframe pages (16 total).
  Lighting lists real keymaps from `resources/main/app.asar` and sends reports
  through `device.write`; Macros persists macro JSON; Music probes and stops
  the loopback rhythm service on port 5577; Pets reads and writes
  `wooden-fish.json`; Plugins scans `%APPDATA%\ATK-HUB\plugins`; Updater parses
  the production `latest.yml` and compares versions; Drivers fetches firmware
  metadata from the cloud API. `AgentClient::execute_action` opens installer
  and download URLs through the live agent.
- **Gap closure**: `atk-input` injects typed macro keys through the Win32
  `keybd_event` API from an explicit user action; tests cover parsing only, so
  no keystrokes are ever injected by the test suite. `atk-features` now
  downloads plugin bundles, verifies SHA-256, rejects unsafe ZIP paths and
  extracts into `plugins/<id>/<version>`; the same verified download/extract
  path stages driver archives before use. The THX client is validated
  end-to-end against an in-process generated gRPC mock server
  (`tests/mock_server.rs`, all 31 RPCs implemented), which exercises
  `status()` through the real tonic client stack over a local channel.
  TLS-specific address/certificate validation stays covered by config tests.

## Status

All milestones and all previously documented gaps are closed. The application
is a 16-page native eframe GUI over the ported core crates, with live agent,
service, HID, wallpaper, THX, macro, plugin, updater and driver functionality.
No system state is changed by tests; installation and key injection require an
explicit GUI action. A license-free per-user `setup.exe` installer is provided
(`installer/build-package.ps1` builds `installer-package/` and
`atk-hub-rs-portable.zip`); install/uninstall were verified against a temporary
target directory.
