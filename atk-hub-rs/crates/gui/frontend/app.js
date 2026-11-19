import {
  mousePage, keyboardPage, gamesPage, macroEditor, hubSettingsCard, petControls, companionCards, pcScriptPage, headsetCards,
  wireLiveEvents, loadLivePage, loadHubSettings, translate,
} from "./live.js";

const Tauri = window.__TAURI__;
const invoke = Tauri?.core?.invoke ?? (async (command, args) => {
  console.warn(`[preview] ${command}`, args);
  return previewResponse(command, args);
});

const state = {
  pages: [],
  info: null,
  current: "overview",
  status: "Ready",
  agent: { connected: false, pipe: null, error: null },
  devices: [],
  selectedDevice: { vendor_id: "0x3554", product_id: "0xf505", product: "VGN Dragonfly 4K Receiver" },
  keymaps: [],
  selectedKeymap: "rgb",
  macros: [],
  plugins: [],
  wallpapers: [],
  wallpaperQuery: "",
  configDoc: "{}",
  petState: null,
  musicRunning: null,
  updateInfo: null,
  driverInfo: null,
  mouse: null,
  mouseError: null,
  systemMouse: null,
  testedKeys: new Set(),
  keymapDoc: null,
  keymapName: null,
  hubSettings: null,
  gameRules: null,
  updatePath: null,
};

const glyphs = {
  overview: "⌂", devices: "DV", mouse: "MO", keyboard: "KB", games: "GM", pcscript: "PC", lighting: "LS", macros: "MX", audio: "TH",
  wallpapers: "BG", music: "MS", pets: "PT", plugins: "PM", updater: "UP",
  drivers: "DR", settings: "ST", diagnostics: "DG",
};

const quick = ["mouse", "keyboard", "lighting", "macros", "wallpapers", "settings"];

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[character]);
}

function pretty(value) {
  if (typeof value === "string") return value;
  try { return JSON.stringify(value, null, 2); } catch { return String(value); }
}

function toHex16(value) {
  const num = Number(value);
  return Number.isFinite(num) ? `0x${num.toString(16).padStart(4, "0")}` : String(value ?? "");
}

function groupDevices(interfaces) {
  const map = new Map();
  for (const item of Array.isArray(interfaces) ? interfaces : []) {
    const key = `${item.vid}:${item.pid}`;
    if (!map.has(key)) {
      map.set(key, {
        vid: item.vid,
        pid: item.pid,
        vidHex: toHex16(item.vid),
        pidHex: toHex16(item.pid),
        product: item.product || `USB Device ${toHex16(item.vid)}:${toHex16(item.pid)}`,
        interfaces: [],
      });
    }
    map.get(key).interfaces.push(item);
  }
  return [...map.values()];
}

function setStatus(text) {
  state.status = text;
  const el = document.getElementById("status-left");
  if (el) el.textContent = text;
}

function pageHeading(page) {
  return `
    <div class="page-heading">
      <div class="eyebrow">${escapeHtml(translate(page.group, page.group))}</div>
      <h1>${escapeHtml(translate(page.id, page.label))}</h1>
      <p class="subtitle">${escapeHtml(page.subtitle)}</p>
    </div>`;
}

async function call(command, args = {}, success) {
  setStatus(`${command}…`);
  try {
    const result = await invoke(command, args);
    if (success) setStatus(success);
    return result;
  } catch (error) {
    setStatus(typeof error === "string" ? error : error.message);
    throw error;
  }
}

function button(label, command, args = {}, options = {}) {
  const target = options.output ? ` data-output="${escapeHtml(options.output)}"` : "";
  const cls = ["button", options.variant ?? "", options.size ?? ""].filter(Boolean).join(" ");
  return `<button class="${cls}" data-command="${command}" data-args='${escapeHtml(JSON.stringify(args))}'${target} ${options.disabled ? "disabled" : ""}>${escapeHtml(label)}</button>`;
}

function navButton(label, pageId, options = {}) {
  const cls = ["button", options.variant ?? "", options.size ?? ""].filter(Boolean).join(" ");
  return `<button class="${cls}" data-page="${escapeHtml(pageId)}" type="button">${escapeHtml(label)}</button>`;
}

function output(id = "output") {
  return `<div class="output" id="${id}">No request yet.</div>`;
}

function field(label, id, value = "", type = "text") {
  return `<div class="form-row"><label for="${id}">${escapeHtml(label)}</label><input id="${id}" type="${type}" value="${escapeHtml(value)}" /></div>`;
}

async function runButton(buttonElement) {
  const command = buttonElement.dataset.command;
  if (!command) return;
  const args = JSON.parse(buttonElement.dataset.args || "{}");
  const textFields = [...buttonElement.closest(".card")?.querySelectorAll("input, textarea") ?? []];
  for (const element of textFields) {
    if (element.id) args[element.id] = element.value;
  }
  const target = document.getElementById(buttonElement.dataset.output ?? "output");
  buttonElement.disabled = true;
  try {
    const result = await call(command, args, `${command} complete`);
    if (command === "device_list") {
      state.devices = Array.isArray(result) ? result : [];
      renderGroupedDevices();
    } else if (command === "keymaps") {
      state.keymaps = Array.isArray(result) ? result : [];
      renderKeymapList();
    } else if (command === "macros_load" || command === "macros_add" || command === "macros_delete") {
      state.macros = Array.isArray(result) ? result : [];
      renderMacroList(state.macros);
    } else if (command === "plugins_list") {
      state.plugins = Array.isArray(result) ? result : [];
      renderPluginList();
    } else if (command === "wallpaper_catalog") {
      state.wallpapers = Array.isArray(result?.wallpaper) ? result.wallpaper : [];
      renderWallpaperGrid();
    } else if (command === "config_load" || command === "config_apply") {
      if (typeof result === "string") {
        state.configDoc = result;
        const docEl = document.getElementById("document");
        if (docEl) docEl.value = result;
      }
    } else if (command === "pets_load") {
      state.petState = result;
      renderPetSummary();
    } else if (command === "music_status" || command === "music_stop") {
      state.musicRunning = command === "music_status" ? Boolean(result) : false;
      renderMusicStatus();
    } else if (command === "update_check") {
      state.updateInfo = result;
      renderUpdateSummary();
    } else if (command === "driver_metadata") {
      state.driverInfo = result;
      renderDriverSummary();
    }
    if (target) target.textContent = pretty(result);
  } catch (error) {
    if (target) target.textContent = String(error?.message ?? error);
  } finally {
    buttonElement.disabled = false;
  }
}

function renderGroupedDevices() {
  const container = document.getElementById("grouped-device-list");
  if (!container) return;
  const groups = groupDevices(state.devices);
  if (groups.length === 0) {
    container.innerHTML = `<div class="notice">No HID devices reported yet. Ensure <code>atk-hub-service</code> is running and click Refresh Devices.</div>`;
    return;
  }
  container.innerHTML = groups.map((dev) => `
    <div class="list-item interactive ${state.selectedDevice.vendor_id === dev.vidHex && state.selectedDevice.product_id === dev.pidHex ? "selected" : ""}" data-select-device="${dev.vidHex}|${dev.pidHex}|${escapeHtml(dev.product)}">
      <div class="device-summary">
        <strong>${escapeHtml(dev.product)}</strong>
        <small>VID ${escapeHtml(dev.vidHex)} · PID ${escapeHtml(dev.pidHex)} · ${dev.interfaces.length} HID interfaces</small>
      </div>
      <button class="button sm primary" type="button" data-select-device="${dev.vidHex}|${dev.pidHex}|${escapeHtml(dev.product)}">Select</button>
    </div>`).join("");
}

function renderKeymapList() {
  const list = document.getElementById("keymap-list");
  if (!list) return;
  if (!Array.isArray(state.keymaps) || state.keymaps.length === 0) {
    list.textContent = "No keymaps loaded.";
    return;
  }
  list.innerHTML = state.keymaps.map((name) => `
    <div class="list-item interactive ${state.selectedKeymap === name ? "selected" : ""}" data-select-keymap="${escapeHtml(name)}">
      <span>${escapeHtml(name)}</span>
      <span class="chip">${state.selectedKeymap === name ? "Selected" : "Use"}</span>
    </div>`).join("");
}

function renderMacroList(macros) {
  const list = document.getElementById("macro-list");
  if (!list) return;
  if (!Array.isArray(macros) || macros.length === 0) {
    list.textContent = "No macros loaded.";
    return;
  }
  list.innerHTML = macros.map((macro, index) => `
    <div class="list-item">
      <span><strong>${escapeHtml(macro.name)}</strong> · ${escapeHtml((macro.steps ?? []).map((step) => `${step.action === "up" ? "↑" : step.action === "down" ? "↓" : ""}${step.key} ${step.delay_ms}ms`).join(", "))} · ${escapeHtml(macro.repeat_mode ?? "count")}${(macro.repeat_mode ?? "count") === "count" ? ` ×${macro.repeat_count ?? 1}` : ""}</span>
      <div class="actions" style="flex-wrap:nowrap">
        ${button("Run", "macros_run", { index }, { output: "macro-run-output", variant: "primary", size: "sm" })}
        ${button("Delete", "macros_delete", { index }, { output: "macro-run-output", size: "sm" })}
      </div>
    </div>`).join("");
}

function renderPluginList() {
  const list = document.getElementById("plugin-list");
  if (!list) return;
  if (!Array.isArray(state.plugins) || state.plugins.length === 0) {
    list.textContent = "No plugins installed in plugins directory.";
    return;
  }
  list.innerHTML = state.plugins.map((plugin) => `
    <div class="list-item">
      <span><strong>${escapeHtml(plugin.id)}</strong> · v${escapeHtml(plugin.version)}</span>
      <span class="chip live">Installed</span>
    </div>`).join("");
}

function renderWallpaperGrid() {
  const grid = document.getElementById("wallpaper-grid");
  if (!grid) return;
  const query = state.wallpaperQuery.trim().toLowerCase();
  const entries = state.wallpapers.filter((item) =>
    !query || (item.title ?? "").toLowerCase().includes(query) || (item.value ?? "").toLowerCase().includes(query)
  );
  if (entries.length === 0) {
    grid.innerHTML = `<div class="notice">No wallpapers match your filter.</div>`;
    return;
  }
  grid.innerHTML = entries.slice(0, 60).map((item) => {
    const thumb = item.previewUrl || item.urlPath || "";
    const dl = item.downloadUrl || thumb;
    return `
      <div class="wallpaper-card">
        ${thumb ? `<img class="wallpaper-thumb" src="${escapeHtml(thumb)}" alt="${escapeHtml(item.title)}" loading="lazy" />` : `<div class="wallpaper-thumb"></div>`}
        <div class="wallpaper-title" title="${escapeHtml(item.title)}">${escapeHtml(item.title || item.value)}</div>
        <div class="wallpaper-meta">
          <span>v${escapeHtml(item.version ?? "1")}</span>
          ${dl ? `<button class="button sm primary" type="button" data-wallpaper-apply="${escapeHtml(dl)}">Apply</button>` : ""}
        </div>
      </div>`;
  }).join("");
}

function renderPetSummary() {
  const box = document.getElementById("pet-summary");
  if (!box || !state.petState) return;
  box.innerHTML = `
    <div class="stat-row"><span class="stat-label">Enabled</span><span class="stat-value">${state.petState.enabled ? "Yes" : "No"}</span></div>
    <div class="stat-row"><span class="stat-label">Pet visible</span><span class="stat-value">${state.petState.petVisible ? "Yes" : "No"}</span></div>
    <div class="stat-row"><span class="stat-label">Sound</span><span class="stat-value">${state.petState.sound ? "On" : "Off"}</span></div>
    <div class="stat-row"><span class="stat-label">Skin</span><span class="stat-value">${escapeHtml(state.petState.skin || "default")}</span></div>`;
  const controls = document.getElementById("pet-controls");
  if (controls) controls.innerHTML = petControls();
}

function renderMusicStatus() {
  const el = document.getElementById("music-status-badge");
  if (!el) return;
  if (state.musicRunning === null) {
    el.textContent = "Checking…";
  } else {
    el.textContent = state.musicRunning ? "Running (127.0.0.1:5577)" : "Stopped";
  }
}

function renderUpdateSummary() {
  const box = document.getElementById("update-summary");
  if (!box || !state.updateInfo) return;
  box.innerHTML = `
    <div class="stat-row"><span class="stat-label">Latest version</span><span class="stat-value">${escapeHtml(state.updateInfo.version)}</span></div>
    <div class="stat-row"><span class="stat-label">Package size</span><span class="stat-value">${escapeHtml(state.updateInfo.size)} bytes</span></div>
    <div class="stat-row"><span class="stat-label">Installer</span><span class="stat-value" style="font-size:12px;word-break:break-all">${escapeHtml(state.updateInfo.url)}</span></div>`;
}

function renderDriverSummary() {
  const box = document.getElementById("driver-summary");
  if (!box || !state.driverInfo) return;
  box.innerHTML = `
    <div class="stat-row"><span class="stat-label">Version</span><span class="stat-value">${escapeHtml(state.driverInfo.version)}</span></div>
    <div class="stat-row"><span class="stat-label">File</span><span class="stat-value" style="font-size:12px;word-break:break-all">${escapeHtml(state.driverInfo.file)}</span></div>`;
}

function renderRail() {
  const rail = document.getElementById("rail");
  let lastGroup = "";
  rail.innerHTML = state.pages.map((page) => {
    const group = page.group !== lastGroup ? `<div class="rail-group">${escapeHtml(translate(page.group, page.group))}</div>` : "";
    lastGroup = page.group;
    return group + `
      <button class="rail-item ${page.id === state.current ? "active" : ""}" data-page="${page.id}" type="button">
        <span class="rail-glyph">${glyphs[page.id] ?? page.label.slice(0, 2).toUpperCase()}</span>
        <span>${escapeHtml(translate(page.id, page.label))}</span>
      </button>`;
  }).join("");

  document.getElementById("quick-nav").innerHTML = quick
    .map((id) => state.pages.find((page) => page.id === id))
    .filter(Boolean)
    .map((page) => `<button class="quick-link ${page.id === state.current ? "active" : ""}" data-page="${page.id}" type="button">${escapeHtml(translate(page.id, page.label))}</button>`)
    .join("");
}

function overviewPage() {
  const groups = groupDevices(state.devices);
  const deviceChips = groups.length > 0
    ? groups.slice(0, 4).map((d) => `<span class="chip live">${escapeHtml(d.product)} (${escapeHtml(d.vidHex)})</span>`).join("")
    : `<span class="chip">${state.agent.connected ? "No HID devices found" : "Agent offline"}</span>`;

  return `
    <section class="card hero-card">
      <div class="hero-copy">
        <div class="eyebrow">Device Center</div>
        <h2>Everything your ATK gear needs, in one place.</h2>
        <p>Connect an ATK or VXE device with a 2.4G receiver or a cable to configure it. Lighting, macros, audio, wallpapers and drivers stay in sync from one desktop hub.</p>
        <div class="actions">
          ${navButton("Add Device", "devices")}
          ${navButton("Open Plugins", "plugins", { variant: "primary" })}
        </div>
      </div>
      <div class="hero-art">
        <div class="hero-orbit"></div>
        <img src="assets/mouse.webp" alt="ATK gaming mouse" />
      </div>
    </section>

    <div class="grid three" style="margin-top:18px">
      <article class="card device-card" data-page="devices">
        <div class="device-thumb">⌖</div>
        <div class="device-meta">
          <h3>Devices</h3>
          <p>${groups.length > 0 ? `${groups.length} connected hardware device(s) detected.` : "Live HID interfaces, battery state and firmware summaries."}</p>
          <div class="chip-row"><span class="chip ${state.agent.connected ? "live" : ""}">${state.agent.connected ? "Agent connected" : "Agent offline"}</span><span class="chip">HID</span></div>
        </div>
      </article>
      <article class="card device-card" data-page="lighting">
        <div class="device-thumb"><img src="assets/keyboard.png" alt="Keyboard" /></div>
        <div class="device-meta">
          <h3>Light Show</h3>
          <p>Per-key color, brightness and lighting reports.</p>
          <div class="chip-row"><span class="chip">RGB</span><span class="chip">Keymaps</span></div>
        </div>
      </article>
      <article class="card device-card" data-page="wallpapers">
        <div class="device-thumb">◈</div>
        <div class="device-meta">
          <h3>Background Theme</h3>
          <p>Cloud wallpapers with live preview and metadata.</p>
          <div class="chip-row"><span class="chip">Cloud</span><span class="chip">Catalog</span></div>
        </div>
      </article>
    </div>

    <div class="grid two" style="margin-top:18px">
      <article class="card">
        <div class="card-title"><h2>Detected hardware</h2><span>${escapeHtml(state.agent.pipe ?? "ATK HUB")}</span></div>
        <div class="chip-row">${deviceChips}</div>
        <div class="notice" style="margin-top:16px">The Rust port talks directly to the local ATK agent and cloud services. No Electron runtime or Node integration is loaded.</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Build status</h2><span id="build-version">—</span></div>
        <div class="stat-row"><span class="stat-label">Engine</span><span class="stat-value" id="build-engine">—</span></div>
        <div class="stat-row"><span class="stat-label">Update feed</span><span class="stat-value" id="build-feed">—</span></div>
        <div class="stat-row"><span class="stat-label">User data</span><span class="stat-value" id="build-data">—</span></div>
      </article>
    </div>`;
}

function devicesPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "devices"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Live device list</h2><span>device.list</span></div>
        <div class="actions" style="margin-bottom:12px">${button("Refresh Devices", "device_list", {}, { variant: "primary", output: "device-list-output" })}</div>
        <div class="list" id="grouped-device-list">Loading devices…</div>
        <details class="raw-details">
          <summary>Raw HID interfaces</summary>
          ${output("device-list-output")}
        </details>
      </article>
      <article class="card">
        <div class="card-title"><h2>HID control</h2><span>typed reports</span></div>
        ${field("VID", "vendor_id", state.selectedDevice.vendor_id)}
        ${field("PID", "product_id", state.selectedDevice.product_id)}
        ${field("Report bytes", "report", "00 AA 01")}
        ${field("Scope", "scope", "rgb")}
        ${field("Rule", "rule_name", "launch")}
        ${field("Packet", "packet", "A1")}
        ${field("Target", "target", "notepad.exe")}
        <div class="actions">
          ${button("Connect", "device_control", { operation: "connect" }, { output: "hid-output" })}
          ${button("Disconnect", "device_control", { operation: "disconnect" }, { output: "hid-output" })}
          ${button("Write Report", "device_control", { operation: "write" }, { variant: "primary", output: "hid-output" })}
          ${button("State", "device_control", { operation: "state" }, { output: "hid-output" })}
          ${button("Clear Rules", "device_control", { operation: "clear-rules" }, { output: "hid-output" })}
          ${button("Set Bindings", "device_control", { operation: "bindings" }, { output: "hid-output" })}
        </div>
        ${output("hid-output")}
      </article>
    </div>`;
}

function lightingPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "lighting"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Lighting report</h2><span>device.write</span></div>
        <div class="actions">${button("Load Keymaps", "keymaps")}</div>
        <div class="list" id="keymap-list" style="margin:14px 0">Loading keymaps…</div>
        <div class="form-row">
          <label for="light-color">Color & Brightness</label>
          <div class="color-row">
            <input id="light-color" type="color" value="#ff6900" />
            <input id="light-brightness" type="range" min="0" max="255" value="180" />
          </div>
        </div>
        ${field("Target VID", "vendor_id", state.selectedDevice.vendor_id)}
        ${field("Target PID", "product_id", state.selectedDevice.product_id)}
        ${field("Settings JSON", "document", JSON.stringify({ keymap: state.selectedKeymap, red: 255, green: 105, blue: 0, brightness: 180 }))}
        <div class="actions">
          ${button("Build Report", "lighting_report")}
          ${button("Apply to Device", "lighting_apply", {}, { variant: "primary" })}
        </div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Live preview</h2><span>RGB</span></div>
        <div class="hero-art" style="min-height:300px;border-radius:18px">
          <div class="hero-orbit"></div>
          <img src="assets/keyboard.png" alt="Keyboard lighting preview" />
        </div>
        <div class="notice" style="margin-top:14px">Reports are assembled by the Rust core and sent through the validated agent pipe. The UI never opens a browser or loads Electron.</div>
      </article>
    </div>`;
}

function macrosPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "macros"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Macro library</h2><span>JSON storage</span></div>
        ${field("Storage path", "path", state.info?.macroPath ?? "")}
        <div class="actions" style="margin-bottom:12px">
          ${button("Set Path", "macros_path")}
          ${button("Load", "macros_load")}
          ${button("Save", "macros_save", {}, { variant: "primary" })}
        </div>
        ${field("Name", "name", "Launch editor")}
        ${field("Keys (comma separated)", "keys", "Ctrl, Shift, F13")}
        <div class="actions">${button("Add Macro", "macros_add")}</div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Recorded sequences</h2><span>typed injection</span></div>
        <div class="list" id="macro-list">Loading macros…</div>
        ${output("macro-run-output")}
        <div class="notice" style="margin-top:14px">Key injection only runs when you press Run. Macro names, delays and storage stay under your control.</div>
      </article>
    </div>
    <div id="macro-recorder" style="margin-top:18px">${macroEditor()}</div>`;
}

function audioPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "audio"))}
    <div id="headset" style="margin-bottom:16px">${headsetCards()}</div>
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>THX connection</h2><span>gRPC over TLS</span></div>
        ${field("Product", "product", "atk_n9v2_ultimate")}
        <div class="actions">
          ${button("Validate File", "audio_config")}
          ${button("Read Status", "audio_status", {}, { variant: "primary" })}
          ${button("Enable Spatial", "audio_spatial", { enabled: true })}
          ${button("Disable Spatial", "audio_spatial", { enabled: false })}
        </div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Audio profile</h2><span>headphones</span></div>
        <div class="stat-row"><span class="stat-label">Endpoint</span><span class="stat-value">headphones</span></div>
        <div class="stat-row"><span class="stat-label">Property</span><span class="stat-value">spatial_enabled</span></div>
        <div class="stat-row"><span class="stat-label">Protocol</span><span class="stat-value">THX proto v1</span></div>
        <div class="notice" style="margin-top:16px">The client reads the same <code>thx_properties.tcp</code> file as the original application and validates the TLS certificate before connecting.</div>
      </article>
    </div>`;
}

function wallpapersPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "wallpapers"))}
    <article class="card">
      <div class="card-title"><h2>ATK wallpaper catalogue</h2><span>list-v2.json</span></div>
      <div class="actions" style="align-items:center;justify-content:space-between;margin-bottom:12px">
        ${button("Refresh Catalogue", "wallpaper_catalog", {}, { variant: "primary" })}
        <input id="wallpaper-search" type="search" placeholder="Search wallpapers…" value="${escapeHtml(state.wallpaperQuery)}" style="max-width:260px" />
      </div>
      <div class="wallpaper-grid" id="wallpaper-grid">Loading wallpaper catalogue…</div>
      <details class="raw-details">
        <summary>Raw catalogue JSON</summary>
        ${output()}
      </details>
    </article>`;
}

function musicPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "music"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Legacy rhythm service</h2><span id="music-status-badge">Checking…</span></div>
        <div class="actions">
          ${button("Check Service", "music_status")}
          ${button("Stop Service", "music_stop", {}, { variant: "primary" })}
        </div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Service lifecycle</h2><span>best effort</span></div>
        <div class="notice">The port probes and exits the music rhythm service through its original loopback endpoint. A refused connection is treated as already stopped.</div>
      </article>
    </div>`;
}

function petsPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "pets"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Desktop companion settings</h2><span>wooden-fish.json</span></div>
        ${field("Settings path", "path", state.info?.petPath ?? "")}
        <div class="actions">${button("Load Settings", "pets_load", {}, { variant: "primary" })}</div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Companion status</h2><span>wooden-fish</span></div>
        <div id="pet-summary">Loading companion settings…</div>
      </article>
    </div>
    <div id="companions" style="margin-top:16px">${companionCards()}</div>`;
}

function pluginsPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "plugins"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Installed plugins</h2><span>plugins directory</span></div>
        <div class="actions" style="margin-bottom:12px">${button("Scan Plugins", "plugins_list", {}, { variant: "primary" })}</div>
        <div class="list" id="plugin-list">Scanning plugins…</div>
        <details class="raw-details">
          <summary>Raw plugin output</summary>
          ${output()}
        </details>
      </article>
      <article class="card">
        <div class="card-title"><h2>Install bundle</h2><span>SHA-256 verified</span></div>
        ${field("Bundle URL", "url")}
        ${field("Plugin id", "id")}
        ${field("Version", "version", "1.0")}
        ${field("SHA-256", "sha256")}
        <div class="actions">${button("Install", "plugins_install", {}, { variant: "accent" })}</div>
      </article>
    </div>`;
}

function updaterPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "updater"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Update feed</h2><span>latest.yml</span></div>
        <div class="actions" style="margin-bottom:12px">
          ${button("Check for Updates", "update_check", {}, { variant: "primary" })}
          <button class="button" type="button" data-update-download>Download Update</button>
          <button class="button accent" type="button" data-update-install ${state.updatePath ? "" : "disabled"}>Run Installer</button>
        </div>
        <div id="update-summary">Checking update feed…</div>
        <details class="raw-details">
          <summary>Raw update payload</summary>
          ${output()}
        </details>
      </article>
      <article class="card">
        <div class="card-title"><h2>Release channel</h2><span>latest</span></div>
        <div class="stat-row"><span class="stat-label">Current version</span><span class="stat-value" id="update-current">—</span></div>
        <div class="stat-row"><span class="stat-label">Host</span><span class="stat-value" id="update-host">—</span></div>
        <div class="notice" style="margin-top:16px">Downloads are verified against the feed's SHA-512 before the installer can run. Nothing installs until you click Run Installer.</div>
      </article>
    </div>`;
}

function driversPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "drivers"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Game power service</h2><span>firmware API</span></div>
        <div class="actions" style="margin-bottom:12px">
          ${button("Fetch Metadata", "driver_metadata", {}, { variant: "primary" })}
          ${button("Prepare Archive", "driver_prepare")}
          <button class="button accent" type="button" data-driver-install>Install Driver</button>
        </div>
        <div id="driver-summary" style="margin-bottom:12px">Fetching driver metadata…</div>
        ${field("Expected SHA-256", "sha256", "")}
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Verified extraction</h2><span>ZIP safety checks</span></div>
        <div class="notice">Driver archives are verified against SHA-256 and extracted only after every entry path passes traversal checks.</div>
      </article>
    </div>`;
}

function settingsPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "settings"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Application config</h2><span>config.json</span></div>
        ${field("Config path", "path", state.info?.configPath ?? "")}
        <div class="actions">${button("Load Config", "config_load", {}, { variant: "primary" })}</div>
        <div class="form-row" style="margin-top:14px"><label for="document">Document</label><textarea id="document">${escapeHtml(state.configDoc)}</textarea></div>
        ${field("Key", "key", "atk_service_url")}
        ${field("Value", "value", "")}
        <div class="actions">
          ${button("Apply (memory)", "config_apply")}
          ${button("Apply & Save", "config_apply", { save: true }, { variant: "primary" })}
        </div>
        ${output()}
      </article>
      ${hubSettingsCard()}
      <article class="card">
        <div class="card-title"><h2>Appearance</h2><span>interface</span></div>
        <div class="stat-row"><span class="stat-label">Theme</span><span class="stat-value" id="theme-name">${document.documentElement.dataset.theme === "dark" ? "Dark" : "Light"}</span></div>
        <div class="stat-row"><span class="stat-label">Scaling</span><span class="stat-value">System</span></div>
        <div class="stat-row"><span class="stat-label">Shell</span><span class="stat-value">Tauri 2</span></div>
      </article>
    </div>`;
}

function diagnosticsPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "diagnostics"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Agent protocol</h2><span>JSON frames</span></div>
        ${field("Pipe", "name", state.agent.pipe ?? "atk-hub-agent-session-1")}
        ${field("Request id", "request_id", "7")}
        ${field("Method", "method", "device.list")}
        ${field("Params", "params", "null")}
        <div class="actions">
          ${button("Validate Pipe", "agent_validate_pipe", {}, { output: "agent-output" })}
          ${button("Encode", "agent_encode", {}, { output: "agent-output" })}
          ${button("Connect", "agent_connect", {}, { variant: "primary", output: "agent-output" })}
          ${button("Send", "agent_request", {}, { output: "agent-output" })}
          ${button("Disconnect", "agent_disconnect", {}, { output: "agent-output" })}
        </div>
        ${field("Frame", "frame", '{"id":7,"ok":true,"result":{},"error":null}')}
        <div class="actions">${button("Decode", "agent_decode", {}, { output: "agent-output" })}</div>
        ${output("agent-output")}
      </article>
      <article class="card">
        <div class="card-title"><h2>Static registry</h2><span>recovered</span></div>
        <div class="actions">
          ${button("Endpoints", "endpoint_catalog", {}, { output: "diagnostics-output" })}
          ${button("IPC Channels", "ipc_categories", {}, { output: "diagnostics-output" })}
          ${button("ASAR Manifest", "asar_manifest", { path: state.info?.asarPath ?? "C:\\\\Program Files\\\\ATK HUB\\\\resources\\\\app.asar" }, { output: "diagnostics-output" })}
        </div>
        ${field("Firmware VID", "vendor_id", state.selectedDevice.vendor_id)}
        ${field("Firmware PID", "product_id", state.selectedDevice.product_id)}
        ${field("Tag", "tag", "service")}
        <div class="actions">${button("Firmware URL", "firmware_url", {}, { output: "diagnostics-output" })}</div>
        ${output("diagnostics-output")}
      </article>
    </div>`;
}

const renderers = {
  overview: overviewPage,
  devices: devicesPage,
  mouse: mousePage,
  keyboard: keyboardPage,
  games: gamesPage,
  pcscript: pcScriptPage,
  lighting: lightingPage,
  macros: macrosPage,
  audio: audioPage,
  wallpapers: wallpapersPage,
  music: musicPage,
  pets: petsPage,
  plugins: pluginsPage,
  updater: updaterPage,
  drivers: driversPage,
  settings: settingsPage,
  diagnostics: diagnosticsPage,
};

async function loadPageData(pageId) {
  loadLivePage(pageId);
  if (pageId === "devices") {
    try {
      const devices = await invoke("device_list");
      state.devices = Array.isArray(devices) ? devices : [];
      renderGroupedDevices();
      const out = document.getElementById("device-list-output");
      if (out) out.textContent = pretty(devices);
    } catch (error) {
      const container = document.getElementById("grouped-device-list");
      if (container) container.innerHTML = `<div class="notice">${escapeHtml(String(error?.message ?? error))}</div>`;
    }
  } else if (pageId === "lighting") {
    try {
      const maps = await invoke("keymaps");
      state.keymaps = Array.isArray(maps) ? maps : [];
      renderKeymapList();
    } catch (error) {
      const list = document.getElementById("keymap-list");
      if (list) list.textContent = String(error?.message ?? error);
    }
  } else if (pageId === "macros") {
    try {
      const loaded = await invoke("macros_load");
      state.macros = Array.isArray(loaded) ? loaded : [];
      renderMacroList(state.macros);
    } catch {
      try {
        const listed = await invoke("macros_list");
        state.macros = Array.isArray(listed) ? listed : [];
        renderMacroList(state.macros);
      } catch {
        renderMacroList([]);
      }
    }
  } else if (pageId === "wallpapers") {
    try {
      const catalog = await invoke("wallpaper_catalog");
      state.wallpapers = Array.isArray(catalog?.wallpaper) ? catalog.wallpaper : [];
      renderWallpaperGrid();
      const out = document.getElementById("output");
      if (out) out.textContent = pretty(catalog);
    } catch (error) {
      const grid = document.getElementById("wallpaper-grid");
      if (grid) grid.innerHTML = `<div class="notice">${escapeHtml(String(error?.message ?? error))}</div>`;
    }
  } else if (pageId === "music") {
    try {
      const running = await invoke("music_status");
      state.musicRunning = Boolean(running);
      renderMusicStatus();
    } catch {
      state.musicRunning = false;
      renderMusicStatus();
    }
  } else if (pageId === "pets") {
    try {
      const pet = await invoke("pets_load", { path: state.info?.petPath ?? "" });
      state.petState = pet;
      renderPetSummary();
      const out = document.getElementById("output");
      if (out) out.textContent = pretty(pet);
    } catch (error) {
      const box = document.getElementById("pet-summary");
      if (box) box.innerHTML = `<div class="notice">${escapeHtml(String(error?.message ?? error))}</div>`;
    }
  } else if (pageId === "plugins") {
    try {
      const plugins = await invoke("plugins_list");
      state.plugins = Array.isArray(plugins) ? plugins : [];
      renderPluginList();
    } catch (error) {
      const list = document.getElementById("plugin-list");
      if (list) list.textContent = String(error?.message ?? error);
    }
  } else if (pageId === "updater") {
    try {
      const info = await invoke("update_check");
      state.updateInfo = info;
      renderUpdateSummary();
      const out = document.getElementById("output");
      if (out) out.textContent = pretty(info);
    } catch (error) {
      const box = document.getElementById("update-summary");
      if (box) box.innerHTML = `<div class="notice">${escapeHtml(String(error?.message ?? error))}</div>`;
    }
  } else if (pageId === "drivers") {
    try {
      const info = await invoke("driver_metadata");
      state.driverInfo = info;
      renderDriverSummary();
      const out = document.getElementById("output");
      if (out) out.textContent = pretty(info);
    } catch (error) {
      const box = document.getElementById("driver-summary");
      if (box) box.innerHTML = `<div class="notice">${escapeHtml(String(error?.message ?? error))}</div>`;
    }
  } else if (pageId === "settings") {
    try {
      const doc = await invoke("config_load", { path: state.info?.configPath ?? "" });
      state.configDoc = doc;
      const docEl = document.getElementById("document");
      if (docEl) docEl.value = doc;
    } catch {
      // Keep default {} if config.json does not exist yet.
    }
  }
}

function renderPage() {
  const page = state.pages.find((item) => item.id === state.current);
  if (!page) return;
  document.getElementById("page").innerHTML = (renderers[page.id] ?? overviewPage)();
  for (const [id, value] of Object.entries({
    "build-version": state.info?.version ?? "—",
    "build-engine": state.info?.engine ?? "—",
    "build-feed": state.info?.updateHost ?? "—",
    "build-data": state.info?.userData ?? "—",
    "update-current": state.info?.version ?? "—",
    "update-host": state.info?.updateHost ?? "—",
  })) {
    const element = document.getElementById(id);
    if (element) element.textContent = value;
  }
  renderRail();
  document.getElementById("content").scrollTop = 0;
  loadPageData(page.id);
}

function goTo(id) {
  if (!state.pages.some((page) => page.id === id)) return;
  state.current = id;
  renderPage();
}

function syncLightingDocument() {
  const docEl = document.getElementById("document");
  const colorEl = document.getElementById("light-color");
  const brightEl = document.getElementById("light-brightness");
  if (!docEl || !colorEl || !brightEl) return;
  const hex = colorEl.value.replace("#", "");
  const red = parseInt(hex.slice(0, 2), 16) || 0;
  const green = parseInt(hex.slice(2, 4), 16) || 0;
  const blue = parseInt(hex.slice(4, 6), 16) || 0;
  const brightness = Number(brightEl.value) || 0;
  docEl.value = JSON.stringify({ keymap: state.selectedKeymap, red, green, blue, brightness });
}

function wireGlobalEvents() {
  document.addEventListener("click", (event) => {
    const devTarget = event.target.closest("[data-select-device]");
    if (devTarget) {
      const [vendor_id, product_id, product] = devTarget.dataset.selectDevice.split("|");
      state.selectedDevice = { vendor_id, product_id, product };
      const vidInput = document.getElementById("vendor_id");
      const pidInput = document.getElementById("product_id");
      if (vidInput) vidInput.value = vendor_id;
      if (pidInput) pidInput.value = product_id;
      renderGroupedDevices();
      setStatus(`Selected ${product} (${vendor_id}:${product_id})`);
      return;
    }

    const keymapTarget = event.target.closest("[data-select-keymap]");
    if (keymapTarget) {
      state.selectedKeymap = keymapTarget.dataset.selectKeymap;
      renderKeymapList();
      syncLightingDocument();
      setStatus(`Keymap: ${state.selectedKeymap}`);
      return;
    }

    const actionTarget = event.target.closest("[data-command]");
    if (actionTarget) {
      runButton(actionTarget);
      return;
    }

    const pageTarget = event.target.closest("[data-page]");
    if (pageTarget) {
      goTo(pageTarget.dataset.page);
    }
  });

  document.addEventListener("input", (event) => {
    if (event.target.id === "light-color" || event.target.id === "light-brightness") {
      syncLightingDocument();
    } else if (event.target.id === "wallpaper-search") {
      state.wallpaperQuery = event.target.value;
      renderWallpaperGrid();
    }
  });

  document.getElementById("home-button").addEventListener("click", () => goTo("overview"));
  document.getElementById("settings-button").addEventListener("click", () => goTo("settings"));
  document.getElementById("theme-toggle").addEventListener("click", () => {
    const root = document.documentElement;
    const next = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = next;
    localStorage.setItem("atk-theme", next);
    const label = document.getElementById("theme-name");
    if (label) label.textContent = next === "dark" ? "Dark" : "Light";
  });

  const windowCommand = (name) => async () => {
    try { await invoke(name); } catch (error) { setStatus(String(error)); }
  };
  document.getElementById("minimize-button").addEventListener("click", windowCommand("window_minimize"));
  document.getElementById("maximize-button").addEventListener("click", windowCommand("window_toggle_maximize"));
  document.getElementById("close-button").addEventListener("click", () => {
    const settings = state.hubSettings ?? {};
    if (!settings.closeMinimizeTray && settings.exitDetectionPromptEnabled) {
      showExitPrompt();
    } else {
      windowCommand("window_close")();
    }
  });

  const updatePill = document.getElementById("update-pill");
  const openUpdate = () => goTo("updater");
  updatePill.addEventListener("click", openUpdate);
  updatePill.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") openUpdate();
  });
}

function showExitPrompt() {
  const modal = document.createElement("div");
  modal.className = "modal-backdrop";
  modal.innerHTML = `
    <div class="modal card" role="dialog" aria-modal="true">
      <h2>Exit ATK HUB?</h2>
      <p class="subtitle">Device features such as game auto-switch and lock-key hints stop when the hub exits.</p>
      <div class="actions">
        <button class="button" type="button" data-modal="tray">Minimize to Tray</button>
        <button class="button primary" type="button" data-modal="exit">Exit</button>
        <button class="button" type="button" data-modal="cancel">Cancel</button>
      </div>
    </div>`;
  modal.addEventListener("click", async (event) => {
    const choice = event.target.closest("[data-modal]")?.dataset.modal;
    if (!choice && event.target !== modal) return;
    modal.remove();
    if (choice === "exit") await invoke("app_exit");
    if (choice === "tray") await invoke("window_minimize");
  });
  document.body.append(modal);
}

function previewResponse(command, args) {
  const previewPages = state.pages.length > 0 ? state.pages : [
    { id: "overview", label: "Device Center", subtitle: "Connected devices and system status", group: "Device" },
    { id: "devices", label: "Devices", subtitle: "Live HID interfaces from the ATK agent", group: "Device" },
    { id: "mouse", label: "Mouse", subtitle: "DPI, polling rate, buttons and sensor settings", group: "Device" },
    { id: "keyboard", label: "Keyboard", subtitle: "Layout and key tester", group: "Device" },
    { id: "lighting", label: "Light Show", subtitle: "Keyboard backlight and effects", group: "Device" },
    { id: "macros", label: "Macros", subtitle: "Key sequences and bindings", group: "Device" },
    { id: "audio", label: "Audio", subtitle: "Headset EQ, sidetone and THX spatial audio", group: "Device" },
    { id: "games", label: "Game Mode", subtitle: "Switch mouse profiles when a game is in focus", group: "Device" },
    { id: "pcscript", label: "PC Script", subtitle: "Lua scripts that automate mouse and keyboard input", group: "Device" },
    { id: "wallpapers", label: "Background Theme", subtitle: "Cloud wallpaper catalogue", group: "Library" },
    { id: "music", label: "Music Score Tool", subtitle: "Legacy music rhythm service", group: "Library" },
    { id: "pets", label: "Desktop Pets", subtitle: "Companion and wooden-fish settings", group: "Library" },
    { id: "plugins", label: "Plugin Market", subtitle: "Installed and downloadable extensions", group: "Library" },
    { id: "updater", label: "Update", subtitle: "Version feed and installer", group: "System" },
    { id: "drivers", label: "Drivers", subtitle: "Driver packages and firmware", group: "System" },
    { id: "settings", label: "Settings", subtitle: "Persisted application configuration", group: "System" },
    { id: "diagnostics", label: "Diagnostics", subtitle: "ASAR, endpoints, IPC and agent protocol", group: "System" },
  ];
  const responses = {
    app_info: {
      name: "ATK HUB", version: "0.1.0", engine: "Tauri + Rust",
      updateHost: "https://bpcdn.atkgear.com/device/update-v3/win32",
      userData: "C:\\Users\\Admin\\AppData\\Roaming\\ATK-HUB",
      configPath: "C:\\Users\\Admin\\AppData\\Roaming\\ATK-HUB\\config.json",
      petPath: "C:\\Users\\Admin\\AppData\\Roaming\\ATK-HUB\\wooden-fish.json",
      macroPath: "C:\\Users\\Admin\\AppData\\Roaming\\ATK-HUB\\macros.json",
      asarPath: "C:\\Program Files\\ATK HUB\\resources\\app.asar",
      pages: previewPages,
    },
    agent_auto_connect: { connected: true, pipe: "atk-hub-agent-session-1" },
    device_list: [{ product: "VGN Dragonfly 4K Receiver", vid: 13652, pid: 62725, usage: 1 }],
    plugins_list: [{ id: "demo-plugin", version: "1.0" }],
    keymaps: ["rgb.json", "wave.json", "reactive.json"],
    wallpaper_catalog: { wallpaper: [{ id: 1, title: "Aurora", value: "aurora", version: "1" }] },
    ipc_categories: { groups: [{ category: "device", channels: ["device.list", "device.write"] }], agentMethods: ["state.get"] },
    endpoint_catalog: { firmwareBase: "https://api-hub-cloud.atk.pro/firmware" },
    music_status: false,
    pets_load: { enabled: true, visible: true, sound: true },
    pc_script_get_state: { platformOk: true, running: false, config: { enabled: false, resumeOnStartup: true, activeProfileId: "p1", profiles: [{ id: "p1", name: "Default", luaScript: "function on_tick()\n  if pc_rmb_pressed() and pc_lmb_pressed() then\n    input.move_rel(0, 3)\n  end\nend", enabled: false }], profileForegroundExes: {} }, profileTickErrors: {} },
    headset_list: [{ path: "mock", productId: 4706, name: "ATK N9 V2 Ultimate", family: "bolutek" }],
    headset_bolutek_state: { battery: 76, status: "discharging", eqMode: 0, surround: true, spatial: false, autoPowerOff: 2, sidetone: true, sidetoneVolume: 6, promptVolume: 8, dongleVersion: "1.0.3.2", headsetVersion: "1.0.5.1", rgb: true },
    headset_bolutek_peq: [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000].map((freq) => ({ freq, gain: 0, q: 1, kind: 1, enabled: true })),
    kb_advanced_keys: {
      dks: Array.from({ length: 20 }, (_, i) => ({ actions: [{ status: i ? 0 : 3, code: i ? 0 : 4 }, { status: 0, code: 0 }, { status: 0, code: 0 }, { status: 0, code: 0 }], top: 0.5, bottom: 3 })),
      advanced: Array.from({ length: 20 }, (_, i) => ({ mode: i ? 0 : 2, point: 1.5, withGlobal: false, rt: false, key1: [3, 1], key2: [3, 3], layer: 0, triggerDelay: 200, keyDelay: 200, random: 50, triggerSwitch: 0 })),
      tapDance: Array.from({ length: 40 }, () => ({ tap: 0, hold: 0, delay: 200 })),
    },
    kb_macros: [{ actions: [{ down: true, code: 4, delayMs: 30 }, { down: false, code: 4, delayMs: 0 }], mode: 1, count: 1 }, ...Array.from({ length: 7 }, () => ({ actions: [], mode: 1, count: 1 }))],
    pc_script_logs: { logs: [], running: false },
    overlay_state: { fish: { enabled: true, petVisible: true, sound: true, skin: "default", days: { "2026-11-18": 812, "2026-11-19": 230 } }, pet: { visible: false, scale: 100, alwaysOnTop: true, clickThrough: false, autoPlay: true, followInput: true, showKeyHints: true, showLevel: true, petName: "Aby", xp: 42 }, level: 2, customSound: false, customSprite: false },
    config_load: "{\n\t\"atk_service_url\": \"https://bpcdn.atkgear.com\"\n}",
    update_check: { version: "3.2.31", url: "https://example.invalid/atk-hub.exe", sha512: "preview", size: 1234 },
    driver_metadata: { version: "1.0.0", file: "https://example.invalid/driver.zip" },
    macros_load: [],
    hub_settings_get: { language: "en", openAtLogin: false, closeMinimizeTray: true, exitDetectionPromptEnabled: true, keyboardLockHintEnabled: false, globalHubHotkeyEnabled: false, globalHubHotkeyAccelerator: "Alt+Shift+H", gameSwitchEnabled: false },
    mouse_list: [{ product: "VGN Dragonfly 4K Receiver", vendorId: 13652, productId: 62725 }],
    system_mouse_get: { speed: 10, wheelLines: 3 },
    game_rules_get: [{ exe: "cs2.exe", profile: 1 }],
    keymap_load: { name: "ATK68", layouts: { keymap: [[{ index: [0, 0], defaultKey: { 0: [0, 41] } }, { index: [0, 1], defaultKey: { 0: [0, 30] } }, { index: [0, 2], w: 2, defaultKey: { 0: [0, 42] } }]] } },
    kb_snapshot: {
      model: { name: "EDGE 60HE ULTIMATE", layers: 4, rows: 5, cols: 14 }, protocol: 12, version: "1.05", profile: 0, reportRate: 8000,
      lighting: { mode: 4, speed: 128, brightness: 80, color: [0, 170, 255], colorful: 0 }, sleepSecs: 300,
      features: { rgbArea: 0, wasdSwap: 0, mac: 0, winLock: 0, sixKey: 0 }, gameMode: 1, deadZone: [0.1, 0.1],
      travel: { travelMax: 3.4, travelMin: 0.1, travelStep: 0.01, rtMax: 2, rtMin: 0.01, rtStep: 0.01 },
    },
    kb_keymap: [[41, 30, 42], [43, 20, 26]],
    get kb_apply() { return this.kb_snapshot; },
    kb_actuation: Array(70).fill(1.2),
    kb_rapid_trigger: Array(70).fill({ on: false, press: 0.1, release: 0.1 }),
    mouse_snapshot: {
      name: "VXE R1 PRO MAX", cid: 2, mid: 28, sensor: "Paw3395", maxDpi: 26000, version: "1.21", dongleVersion: "1.08",
      online: true, battery: 80, charging: false, profile: 0,
      rate: { reportRate: 2000, dpiTotal: 4, dpiLevel: 1, bhopMs: 0, buttonOperationMode: 0 },
      stages: [400, 800, 1600, 3200, 6400, 12800, 20000, 26000].map((x, i) => ({ x, y: x, color: [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255], [255, 0, 255], [255, 255, 255], [255, 128, 0]][i] })),
      function: { debounceMs: 4, motionSync: true, sleepSeconds: 60, angleSnap: false, rippleControl: false },
      performance: { sensorSleepEnabled: true, sensorSleepSeconds: 120, sensorMode: 1 },
      liftOff: 1, farDistance: 0, angle: 0, angleEnabled: false, light: { mode: 1, brightness: 1, speed: 1 },
      buttons: [[1, 1, 0], [1, 2, 0], [1, 4, 0], [1, 8, 0], [1, 16, 0], [2, 1, 0]],
    },
  };
  if (command === "firmware_url") return `https://api-hub-cloud.atk.pro/firmware/${args.vendor_id}/${args.product_id}/${args.tag}`;
  if (command === "device_update_version_url") return `https://${args.host}/v1/device/update/${args.platform}/version`;
  if (command.startsWith("window_")) return null;
  return responses[command] ?? { command, args, preview: true };
}

async function bootstrap() {
  document.documentElement.dataset.theme = localStorage.getItem("atk-theme") ?? "light";
  wireGlobalEvents();
  wireLiveEvents();
  try {
    state.info = await call("app_info", {}, "Core connected");
    state.pages = state.info.pages ?? [];
  } catch {
    state.pages = previewResponse("app_info").pages;
    state.info = previewResponse("app_info");
    setStatus("Preview mode");
  }
  document.getElementById("app-version").textContent = `v${state.info.version}`;
  await loadHubSettings();
  renderPage();

  try {
    state.agent = await invoke("agent_auto_connect");
    if (state.agent?.connected) {
      const devices = await invoke("device_list");
      state.devices = Array.isArray(devices) ? devices : [];
      const groups = groupDevices(state.devices);
      if (groups.length > 0) {
        const atk = groups.find((d) => d.vid === 0x3554) ?? groups[0];
        state.selectedDevice = { vendor_id: atk.vidHex, product_id: atk.pidHex, product: atk.product };
      }
      setStatus(`Agent connected · ${groups.length} device(s)`);
      if (state.current === "overview" || state.current === "devices") {
        renderPage();
      }
    } else if (state.agent?.error) {
      setStatus(`Agent offline: ${state.agent.error}`);
    }
  } catch {
    // Non-fatal if agent service is not running.
  }
}

export { invoke, state, call, setStatus, escapeHtml, pageHeading, button, field, output, pretty, renderPage, goTo };

bootstrap();
