# ATK HUB — Reverse Engineering Report

**Date:** 2026-11-18
**Target:** ATK HUB 3.2.31, installed at `C:\Program Files\ATK HUB`
**Primary method:** REA MCP (static, execution-free analysis) + local static parsing where REA's 50 s client deadline or native-provider availability prevented completion.

---

## 1. Scope and method

REA MCP calls actually executed:

| REA tool | Purpose | Key evidence ID |
|---|---|---|
| `inspect_desktop_application` | Classify install tree (Electron + .NET candidates) | `ev_1fad99e5…` |
| `open_binary` / `inspect_artifact` | Inventory `resources\app.asar` (4 001 nodes, 3 631 files) | `ev_265b8d8f…` |
| `inspect_analysis_view` (inventory/summary) | Page ASAR entries, locate entrypoints | `ev_6c9ede1a…` |
| `analyze_javascript_application` (bounded) | Static JS graph for manifest + 4 preload/overlay scripts: 32 findings, 17 IPC operations, 15 literal IPC channels, 1 `contextBridge` API | `ev_49aaf010…` |
| `inspect_managed_artifact` | Classify `KeyLinkerlib.dll` as mixed C++/CLI .NET Framework assembly | `ev_36c8121b…` |
| `inspect_managed_native_boundaries` | 3 ModuleRefs, 12 P/Invoke declarations, 14 native implementations | `ev_7fe05813…` |
| `inspect_pe_imports` / `inspect_native_exports` / `inspect_pe_certificates` / `inspect_pe_load_config` | Service binary PE structure | `ev_23394429…`, `ev_43842d89…`, `ev_8efc567a…`, `ev_002a2432…` |

Limits encountered (explicitly recorded):
- `analyze_javascript_application` over the full 46 MB ASAR, and over `dist-electron/main/index.js` alone, exceeded the MCP client's **50 s** deadline. REA itself never completed those runs; the bounded run above is the authoritative REA JS result.
- `extract_artifact` also exceeded 50 s. Small files were therefore read by parsing the ASAR header locally; the 10 extracted entry files are in [`re_artifacts`](re_artifacts/).
- `search_strings`/disassembly reported `provider_unavailable` (no configured Ghidra/Hopper/IDA/Kuna). PE metadata uses REA's portable parsers; service strings were extracted locally from the binary.
- `inspect_artifact` failed integrity by default and was rerun with `integrity_policy=record-and-continue` (see §7).

No target code was executed and no live debugging/instrumentation was performed.

---

## 2. What is installed and what is running right now

**Install root:** `C:\Program Files\ATK HUB`

```
ATK HUB.exe                      186,674,272 B   Electron  (resources.pak, v8_context_snapshot.bin, …)
resources\app.asar                46,043,631 B   Electron main + preload + node_modules
resources\app.asar.unpacked\…                      native addons (.node, .dll, .exe)
resources\main\app.asar           48,596,678 B   renderer UI (Vue bundles, keymaps, skins, demos)
resources\service\atk-hub-service.exe 552,448 B   Rust Windows service / agent
resources\thx\thx_audio_properties.proto          THX gRPC contract
resources\elevate.exe                              elevation helper
```

**Identity files**
- `resources\app-env.json`: `APP_ENV=production`, `APP_VERSION=26`, `CUR_ASAR_VERSION=81`
- `app.asar\package.json`: name `ATK-HUB`, **version 3.2.31**, author `ATK <kaifa@vxe.cn>`, main `dist-electron/main/index.js`
- Updater config: `https://bpcdn.atkgear.com/device/update-v3/win32`, channel `latest`

**Live state (observed with shell, not REA):**
- Windows service `AtkHubAgentService` ("ATK HUB Agent Service"), `StartName=LocalSystem`, Auto, Running.
- Service control: `…\resources\service\atk-hub-service.exe run --app "C:\Program Files\ATK HUB\ATK HUB.exe"`
- Two `atk-hub-service.exe` processes: PID 4896 (Session 0, service) and PID 4808 (Session 1, interactive agent).
- Live named pipe: `\\.\pipe\atk-hub-agent-session-1`
- The Electron UI (`ATK HUB.exe`) was **not** running at analysis time; `service.log` shows repeated "desktop agent started in session 1".
- `C:\ProgramData\ATK HUB\service.log` records service starts, "explorer detected in session 1, waiting 5s before launch", agent starts.
- `C:\Users\Admin\AppData\Roaming\ATK-HUB\config.json` stores a `USERINFO.userId`, `atk_service_url` (a zip on `bpcdn.atkgear.com`), and an acknowledged UI version.
- `C:\Users\Admin\AppData\Roaming\ATK-HUB\DevToolsActivePort` contains `9222` and a browser WebSocket path — the app enables remote debugging on the fixed loopback port 9222 when running (port was closed at analysis time).
- Updater has `installer.exe` and `pending\atk-hub-3.2.31.exe` staged locally.

---

## 3. Architecture

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │ AtkHubAgentService (Rust, SYSTEM, session 0)                         │
 │  • install/uninstall/start/stop via SCM                              │
 │  • launches app in interactive session via WTSQueryUserToken +        │
 │    DuplicateTokenEx + CreateEnvironmentBlock + CreateProcessAsUserW  │
 │    (winsta0\default) inside a Job Object                              │
 │  • agent: named pipe \\.\pipe\atk-hub-agent-session-N, JSON ≤256 KiB  │
 │  • subcommands: action (ShellExecuteW), hid list/check/listen,        │
 │    open target, auto-launch, agent, agent-pipe                        │
 └───────────────┬──────────────────────────────────────────────────────┘
                 │ --app …  / named pipe JSON
 ┌───────────────▼──────────────────────────────────────────────────────┐
 │ Electron main — resources\app.asar\dist-electron\main\index.js (1.1 MB)│
 │  • 169 literal ipcMain handlers                                       │
 │  • discovers pipe via `atk-hub-service agent-pipe` → validates name    │
 │    /^atk-hub-agent-session-\d+$/, connects with net.connect           │
 │  • tooltips/tray/notifications/wallpaper/pet/music/plugin/Lua/Diagnostics│
 │  • updater, firmware/audio driver management, PowerShell scripts      │
 └───────┬──────────────────────────────────────────┬───────────────────┘
         │ preload bridges                          │ native addons / helpers
 ┌───────▼───────────────────────────┐   ┌──────────▼───────────────────────┐
 │ preload/index.js                  │   │ node-hid, uiohook-napi, koffi,    │
 │  window.musicPracticeInput        │   │ key-linker-lib, sound-tools,      │
 │  window.nodeHid, window.shell,    │   │ atk-game-helper.exe (22 MB),      │
 │  window.loudness                  │   │ atk-mouse-system-api, usb,        │
 │ rust-agent-preload.js             │   │ music-rhythm-node, @napi-rs/canvas│
 │  contextBridge → window.rustAgent │   │ electron-as-wallpaper-atk, Fengari│
 │ wallpaper-engine-bridge.js        │   │ gRPC @grpc/grpc-js + proto-loader │
 └───────┬───────────────────────────┘   └──────────────────────────────────┘
         │
 ┌───────▼──────────────────────────────────────────────────────────────┐
 │ Renderer UI — resources\main\app.asar (400 files)                     │
 │  Vue 3 bundles, keymaps for ATK/VXE keyboards, skins, pet/demo assets, │
 │  plugin/floating-toolbar/PC-script pages                              │
 └──────────────────────────────────────────────────────────────────────┘
```

---

## 4. Startup, persistence and privilege model

- Installer registers `AtkHubAgentService`; the service binary's own CLI:
  `atk-hub-service install --app <path> [--start] | uninstall | start | stop | status | auto-launch enable|disable|status | agent [--pipe name] [--config path] | agent-pipe | action [--delay-ms 0..60000] <action> | hid list|check|listen <config.json>`.
- Main-process auto-launch wrapper calls `service(["auto-launch","enable"|"disable"|"status"])` and deletes legacy scheduled task / `HKCU|HKLM\Software\Microsoft\Windows\CurrentVersion\Run` entries via `schtasks.exe` and `reg.exe`.
- Service imports (REA PE parser) include `CreateServiceW/DeleteService/ChangeServiceConfigW/StartServiceW/ControlService`, job objects, `WTSGetActiveConsoleSessionId`, `WTSQueryUserToken`-related session APIs, named pipes, `ShellExecuteW`, `HidD_GetIndexedString`.
- Service PE: PE32+ x64, **no export table**, **no embedded Authenticode directory**, load config has `IMAGE_GUARD_CF_INSTRUMENTED`, one TLS callback at RVA `0x45D90`, security cookie present. PDB name `atk_hub_service.pdb` (Rust build).
- Main process appends Chromium switches: `disable-gpu-sandbox`, `disable-renderer-backgrounding`, `disable-background-timer-throttling`, `disable-backgrounding-occluded-windows`.
- Production main window loads `resources\main\app.asar`; if `resources\apks\<N>\app.asar` exists with `N > APP_VERSION (26)`, it loads that instead — a **hot-swappable UI bundle** mechanism. `apks` was absent at analysis time.

---

## 5. IPC / preload surface (REA-confirmed + static extraction)

REA bounded analysis reported: 17 IPC operations, 15 literal channels, 9 renderer transmissions, 8 renderer listeners, 1 `contextBridge` API, 0 main handlers *within the selected files* (the handlers live in the unselected 1.1 MB main bundle, which was analyzed locally).

**Preload bridges (exact contents in `re_artifacts/dist-electron/preload/`):**
- `rustAgent` (contextBridge when isolated, otherwise `window.rustAgent`): `pickOpenTarget`, `listInstalledApplications`, `request(method, params)`, `onEvent(cb)` → channels `atk-open-target:pick`, `atk-open-target:applications`, `atk-rust-agent:request`, `atk-rust-agent:event`.
- `window.musicPracticeInput`: `setEnabled`, `onKey` → `music-practice:input` / `music-practice:key`.
- `window.nodeHid = require("node-hid")`, `window.shell = electron.shell`, `window.loudness = require("loudness")` (get/set volume, mute).

**Rust agent method allowlist (main process):**
`device.list`, `device.connect`, `device.disconnect`, `rules.set`, `action.execute`, `device.write`, `state.get`, `trace.set`, `bindings.set`. Requests are rejected unless the sender is the main window's primary frame; methods outside the allowlist throw "Unsupported Rust Agent method". `device.write` / `action.execute` are the strongest capabilities exposed through this bridge.

**169 literal `ipcMain` handlers**, grouped:
- HID/device/music: `musicRhythm_*` (9), `dotScreen:*` (3), `scanDevice`, `getPointerSpeed`/`setPointerSpeed`, `getRollerSpeed`/`setRollerSpeed`, `audioSystemManage`, `getHeadsetAudioOperation`, `headsetDriverLog`, `runMusic*`, `runBykb*`, `check/start/stopBykbService`
- Pet / widgets: `woodenFish:*` (9), `hoopsPet:*` (12), `capsLockHint:show`, `lockHint:*`, `notification:show`, `changeNotificationSize`
- Floating selection / toolbar / clipboard / global hotkeys: 20+ `floating-*`, `globalHubHotkey:tryRegister`, `floating-toolbar-read-clipboard`
- Plugins & Lua PC scripts: `plugin-install-bundle`, `plugin-*-renderer-path`, `plugin-remove-installed-files`, `pc-script:*` (14, including `save-and-apply`, `validate-lua-script`, `probe-keys`, Lua import/export)
- Wallpaper: 15 `wallpaper:*` handlers (list/download/apply/clear/displays/brightness/style/audio listeners)
- Updates/files/system: `startUpdate`, `installUpdate`, `downloadFile`, `fileExist`, `open-win`, `close-win`, `hide-win`, `max-win`, `open-dev-tools`, `restartApp`, `exit`, `dialog`, `selectFile`, `showSaveDialog`, `generatedData`, `getSetting`/`setSetting`, `appVersion`, `get-temp-path`, `updateTheme`
- Driver service lifecycle: `driverRestartPrompt:*`, `background-service-exit:*`, `app-exit:guard-state`, `quit-app`
- Tray: `tary-menu-list` (sic), `tray-menu:action`, `tray-menu:perform`, `tray-tooltip:update`
- Diagnostics: `sls-log`, `atk-main-process-message`, `audio-diagnostic`

`plugin-install-bundle` downloads a caller-supplied `zipUrl` to `%TEMP%\atk-plugin-…zip`, optionally verifies a caller-supplied SHA-256, and extracts into `%APPDATA%\ATK-HUB\plugins\<id>\<version>` — remote/plugin code can land in the UI plugin directory.

---

## 6. Network endpoints and download flows

Observed literals in the main bundle:

| Endpoint | Use (from surrounding code) |
|---|---|
| `https://bpcdn.atkgear.com/device/update-v3/win32` | electron-updater channel `latest` (`app-update.yml`) |
| `https://<host>/v1/device/update/<platform>/version` | updater version check |
| `https://api-hub-cloud.atk.pro/firmware/0/2/service` | BYKB music service download URL |
| `https://api-hub-cloud.atk.pro/firmware/0/0/service` | game-power/atkservice download URL |
| `https://api-hub-cloud.atk.pro/firmware/0/2/service` | firmware service metadata |
| `https://api-hub-cloud.atk.pro/firmware/<vid>/<pid>/<tag>` | device firmware / model-pack metadata + download |
| `https://api-hub-cloud.atk.pro/firmware/<vendorId>/<productId>/<tag>` | THX audio driver/firmware fetch |
| `https://wallpaper.atkgear.com/list-v2.json` | wallpaper catalogue |
| `https://desktop.vxe.com/v3hub` | "download from official site" fallback |
| `http://127.0.0.1:5577/exit` | shutdown of the local music service on port 5577 |
| `http://localhost:5175`, `http://localhost:8888` | dev servers (packaged app uses local files) |
| `https://api.keygen.sh/v1/accounts/<account>/artifacts?product=…` | from bundled keygen/licensing client code; main bundle shows a "许可证验证失败" dialog |
| `https://s3*.amazonaws.com/...`, DigitalOcean Spaces | bundled update/download provider code |

Downloads are written to `TEMP\game-power\`, `TEMP\game-power\atkservice`, or `TEMP\atk-plugin-*`, and the UI can hot-load a newer `app.asar` from `resources\apks\<N>`.

**THX gRPC:** `thx\thx_audio_properties.proto` defines `service ThxAudioProperties` with ~28 RPCs (endpoint/property/preset/pose/application management and change subscriptions). The client loads the proto at runtime, reads a per-device endpoint JSON file, and connects to that local gRPC service (with an endpoint-format check).

---

## 7. Notable observations (facts, with context)

These are characteristics of the build, not claims of malicious intent — most map directly to the product's keyboard/mouse/audio/wallpaper feature set.

1. **Very broad renderer privileges.** The main window is created with `nodeIntegration:true, contextIsolation:false, webSecurity:false, experimentalFeatures:true, webviewTag:true`; several overlay windows also use `nodeIntegration:true, contextIsolation:false` (some with `webSecurity:false`). The preload directly exposes `node-hid`, `shell`, and volume-control APIs. Any script executing in those pages effectively has full Node/system capability.
2. **Production remote debugging.** `DevToolsActivePort` points to loopback port 9222 with a browser WebSocket path — standard Electron remote debugging, but enabled in a shipped production build.
3. **SYSTEM service + interactive-session launch.** The Rust service runs as LocalSystem and launches the GUI into the user session via token duplication and `CreateProcessAsUserW`, constrained by a Job Object. It also exposes an agent pipe with `action.execute`/`device.write` methods.
4. **Named-pipe local control channel.** `\\.\pipe\atk-hub-agent-session-1` is live; JSON frames are capped at 256 KiB; the main process validates the pipe name pattern but pipe ACLs were not inspected.
5. **Dynamic code/asset delivery.** Firmware, model packs, services, wallpapers, plugins and a versioned `app.asar` are downloaded from HTTPS endpoints; plugin bundles accept a caller-supplied URL and optional SHA-256. UI version gating uses a local env file (`APP_VERSION=26`, `CUR_ASAR_VERSION=81`) versus the installed package version 3.2.31.
6. **Extensive host reach.** `PowerShell -EncodedCommand` (driver/service cleanup), `reg.exe`, `schtasks.exe`, `pnputil.exe`, `sc.exe`, `wscript.exe`, `shutdown.exe`, `tasklist.exe`, `elevate.exe`, `game-power-service.exe`, `BykbLightingService.exe`, `service_manager.exe`, `apo_auth_service.exe`; global keyboard/mouse hooks (`uiohook-napi`), clipboard, HID, USB, raw mouse API, audio-device/driver management, Lua PC scripts via Fengari.
7. **Post-packaging binary changes.** REA integrity comparison flagged two unpacked executables whose on-disk hashes differ from the ASAR manifest: `node_modules/atk-game-helper/bin/atk-game-helper.exe` and `node_modules/loudness/impl/windows/adjust_get_current_system_volume_vista_plus.exe`. REA treated the observed bytes as untrusted. This is common when binaries are signed/stripped after ASAR creation, but it means the shipped bytes are not the bytes the ASAR manifest attests.
8. **Native addon surface.** Large native components ship inside the ASAR: `skia.win32-x64-msvc.node` (26 MB), `atk-game-helper.exe` (22 MB), `music-rhythm-node/index.node` (14 MB), `koffi`, `@napi-rs/canvas`, `key-linker-lib`, `sound-tools`, `usb`, `node-hid`, `atk-mouse-system-api`, `system-activate-windows`, `electron-as-wallpaper-atk`.
9. **Mixed-mode .NET bridge.** `KeyLinkerlib.dll` is a C++/CLI mixed-mode assembly (native entry point, IL-only clear) targeting .NET Framework 4.0, with 12 P/Invokes (CRT + KERNEL32 `EncodePointer`/`DecodePointer`). Static inspection did not decompile its C#/native bodies.
10. **No code signature on the service binary** (embedded security directory absent) and **no PE export table**; CFG instrumented flag set, one TLS callback. This is not proof of an unsigned catalog signature, only of the embedded table.

---

## 8. Artifacts and repro hints

- Report: [ATK_HUB_reverse_engineering_report.md](ATK_HUB_reverse_engineering_report.md)
- Extracted entry files: [`re_artifacts/`](re_artifacts/) — `package.json`, all three preload scripts, overlay files, main index.js (1.1 MB), HID worker, dist HTML.
- Helper scripts used for the pieces REA could not finish within 50 s:
  `C:\Users\Admin\AppData\Local\Temp\opencode\atk_re\asar_extract.py`, `scan_main.py`, `context.py`, `patterns.py`, `strings.py`
- To revisit with REA: re-run `open_binary` on `resources\app.asar` and call `analyze_javascript_application` with `source_paths: ["dist-electron/main/index.js"]` from a client with a tool timeout above 50 s; raise `client tool_timeout_sec` (Codex) / MCP deadline accordingly. Install a disassembly provider (Ghidra/Hopper/Kuna) to enable `search_strings`/`list_procedures`/`procedure_pseudo_code` on `atk-hub-service.exe` and `atk-game-helper.exe`.

## 9. Bottom line

ATK HUB is a three-layer Electron application: a Rust SYSTEM service/agent that launches and supervises the GUI in the interactive session and exposes a local JSON named-pipe control channel; a privileged Electron main process with ~169 IPC handlers and many native addons that manage keyboards, mice, lighting, audio/THX, wallpapers, plugins, Lua scripts, updates and drivers; and a Vue renderer that is hot-loaded from a versioned ASAR. The build intentionally grants its renderer full Node access and ships a production remote-debugging port, and it refreshes significant code and assets from ATK/VXE cloud endpoints. Two unpacked helper binaries were modified after the ASAR manifest was written.
