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
};

const glyphs = {
  overview: "⌂", devices: "DV", lighting: "LS", macros: "MX", audio: "TH",
  wallpapers: "BG", music: "MS", pets: "PT", plugins: "PM", updater: "UP",
  drivers: "DR", settings: "ST", diagnostics: "DG",
};

const quick = ["wallpapers", "lighting", "macros", "audio", "plugins", "settings"];

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[character]);
}

function pretty(value) {
  if (typeof value === "string") return value;
  try { return JSON.stringify(value, null, 2); } catch { return String(value); }
}

function setStatus(text) {
  state.status = text;
  document.getElementById("status-left").textContent = text;
}

function pageHeading(page) {
  return `
    <div class="page-heading">
      <div class="eyebrow">${escapeHtml(page.group)}</div>
      <h1>${escapeHtml(page.label)}</h1>
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
  return `<button class="button ${options.variant ?? ""}" data-command="${command}" data-args='${escapeHtml(JSON.stringify(args))}'${target} ${options.disabled ? "disabled" : ""}>${escapeHtml(label)}</button>`;
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
    if (command === "macros_load" || command === "macros_add") renderMacroList(result);
    if (target) target.textContent = pretty(result);
  } catch (error) {
    if (target) target.textContent = String(error?.message ?? error);
  } finally {
    buttonElement.disabled = false;
  }
}

function renderMacroList(macros) {
  const list = document.getElementById("macro-list");
  if (!list) return;
  if (!Array.isArray(macros) || macros.length === 0) {
    list.textContent = "No macros loaded.";
    return;
  }
  list.innerHTML = macros.map((macro, index) => `
    <div class="list-row">
      <span><strong>${escapeHtml(macro.name)}</strong> &middot; ${escapeHtml((macro.steps ?? []).map((step) => step.key).join(", "))}</span>
      ${button("Run", "macros_run", { index }, { output: "macro-run-output" })}
    </div>`).join("");
}

function renderRail() {
  const rail = document.getElementById("rail");
  let lastGroup = "";
  rail.innerHTML = state.pages.map((page) => {
    const group = page.group !== lastGroup ? `<div class="rail-group">${escapeHtml(page.group)}</div>` : "";
    lastGroup = page.group;
    return group + `
      <button class="rail-item ${page.id === state.current ? "active" : ""}" data-page="${page.id}" type="button">
        <span class="rail-glyph">${glyphs[page.id] ?? page.label.slice(0, 2).toUpperCase()}</span>
        <span>${escapeHtml(page.label)}</span>
      </button>`;
  }).join("");

  document.getElementById("quick-nav").innerHTML = quick
    .map((id) => state.pages.find((page) => page.id === id))
    .filter(Boolean)
    .map((page) => `<button class="quick-link ${page.id === state.current ? "active" : ""}" data-page="${page.id}" type="button">${escapeHtml(page.label)}</button>`)
    .join("");
}

function overviewPage() {
  return `
    <section class="card hero-card">
      <div class="hero-copy">
        <div class="eyebrow">Device Center</div>
        <h2>Everything your ATK gear needs, in one place.</h2>
        <p>Connect an ATK or VXE device with a 2.4G receiver or a cable to configure it. Lighting, macros, audio, wallpapers and drivers stay in sync from one desktop hub.</p>
        <div class="actions">
          ${button("Add Device", "device_list")}
          ${button("Open Plugins", "plugins_list", {}, { variant: "primary" })}
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
          <p>Live HID interfaces, battery state and firmware summaries.</p>
          <div class="chip-row"><span class="chip live">Agent ready</span><span class="chip">HID</span></div>
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
        <div class="card-title"><h2>Connection modes</h2><span>ATK HUB</span></div>
        <div class="chip-row"><span class="chip">Receiver</span><span class="chip">Cable</span><span class="chip">Bluetooth Mode</span><span class="chip">2.4G Connection</span><span class="chip">NearLink Connection</span></div>
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
        <div class="actions">${button("Refresh Devices", "device_list", {}, { variant: "primary" })}</div>
        ${output("device-list-output")}
      </article>
      <article class="card">
        <div class="card-title"><h2>HID control</h2><span>typed reports</span></div>
        ${field("VID", "vendor_id", "0x3554")}
        ${field("PID", "product_id", "0xf505")}
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
        <div class="list" id="keymap-list" style="margin-top:14px">No keymaps loaded.</div>
        ${field("Settings JSON", "document", '{"keymap":"rgb","red":255,"green":105,"blue":0,"brightness":180}')}
        <div class="actions">${button("Build Report", "lighting_report", {}, { variant: "primary" })}</div>
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
        ${field("Storage path", "path", "")}
        <div class="actions">
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
        <div class="list" id="macro-list">No macros loaded.</div>
        ${output("macro-run-output")}
        <div class="notice" style="margin-top:14px">Key injection only runs when you press Run. Macro names, delays and storage stay under your control.</div>
      </article>
    </div>`;
}

function audioPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "audio"))}
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
      <div class="actions">${button("Refresh Catalogue", "wallpaper_catalog", {}, { variant: "primary" })}</div>
      ${output()}
    </article>`;
}

function musicPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "music"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Legacy rhythm service</h2><span>127.0.0.1:5577</span></div>
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
    <article class="card">
      <div class="card-title"><h2>Desktop companion settings</h2><span>wooden-fish.json</span></div>
      ${field("Settings path", "path", "")}
      <div class="actions">${button("Load Settings", "pets_load", {}, { variant: "primary" })}</div>
      ${output()}
    </article>`;
}

function pluginsPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "plugins"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Installed plugins</h2><span>plugins directory</span></div>
        <div class="actions">${button("Scan Plugins", "plugins_list", {}, { variant: "primary" })}</div>
        <div class="list" id="plugin-list">No scan performed.</div>
        ${output()}
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
        <div class="actions">${button("Check for Updates", "update_check", {}, { variant: "primary" })}</div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Release channel</h2><span>latest</span></div>
        <div class="stat-row"><span class="stat-label">Current version</span><span class="stat-value" id="update-current">—</span></div>
        <div class="stat-row"><span class="stat-label">Host</span><span class="stat-value" id="update-host">—</span></div>
        <div class="notice" style="margin-top:16px">Update metadata is parsed by the same Rust feature layer used by the CLI. No installer runs automatically.</div>
      </article>
    </div>`;
}

function driversPage() {
  return `
    ${pageHeading(state.pages.find((page) => page.id === "drivers"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Game power service</h2><span>firmware API</span></div>
        <div class="actions">
          ${button("Fetch Metadata", "driver_metadata", {}, { variant: "primary" })}
          ${button("Prepare Archive", "driver_prepare")}
        </div>
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
        ${field("Config path", "path", "")}
        <div class="actions">${button("Load Config", "config_load", {}, { variant: "primary" })}</div>
        <div class="form-row" style="margin-top:14px"><label for="document">Document</label><textarea id="document">{}</textarea></div>
        ${field("Key", "key", "atk_service_url")}
        ${field("Value", "value", "")}
        <div class="actions">
          ${button("Apply (memory)", "config_apply")}
          ${button("Apply & Save", "config_apply", { save: true }, { variant: "primary" })}
        </div>
        ${output()}
      </article>
      <article class="card">
        <div class="card-title"><h2>Appearance</h2><span>interface</span></div>
        <div class="stat-row"><span class="stat-label">Theme</span><span class="stat-value" id="theme-name">Light</span></div>
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
        ${field("Pipe", "name", "atk-hub-agent-session-1")}
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
          ${button("ASAR Manifest", "asar_manifest", { path: "C:\\\\Program Files\\\\ATK HUB\\\\resources\\\\main\\\\app.asar" }, { output: "diagnostics-output" })}
        </div>
        ${field("Firmware VID", "vendor_id", "0x3554")}
        ${field("Firmware PID", "product_id", "0xf505")}
        ${field("Tag", "tag", "service")}
        <div class="actions">${button("Firmware URL", "firmware_url", {}, { output: "diagnostics-output" })}</div>
        ${output("diagnostics-output")}
      </article>
    </div>`;
}

const renderers = {
  overview: overviewPage,
  devices: devicesPage,
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
}

function goTo(id) {
  if (!state.pages.some((page) => page.id === id)) return;
  state.current = id;
  renderPage();
}

function wireGlobalEvents() {
  document.addEventListener("click", (event) => {
    const pageTarget = event.target.closest("[data-page]");
    if (pageTarget) {
      goTo(pageTarget.dataset.page);
      return;
    }
    const actionTarget = event.target.closest("[data-command]");
    if (actionTarget) {
      runButton(actionTarget);
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
  document.getElementById("close-button").addEventListener("click", windowCommand("window_close"));

  const updatePill = document.getElementById("update-pill");
  const openUpdate = () => goTo("updater");
  updatePill.addEventListener("click", openUpdate);
  updatePill.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") openUpdate();
  });
}

function previewResponse(command, args) {
  const previewPages = state.pages.length > 0 ? state.pages : [
    { id: "overview", label: "Device Center", subtitle: "Connected devices and system status", group: "Device" },
    { id: "devices", label: "Devices", subtitle: "Live HID interfaces from the ATK agent", group: "Device" },
    { id: "lighting", label: "Light Show", subtitle: "Keyboard backlight and effects", group: "Device" },
    { id: "macros", label: "Macros", subtitle: "Key sequences and bindings", group: "Device" },
    { id: "audio", label: "Audio", subtitle: "THX spatial audio properties", group: "Device" },
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
      pages: previewPages,
    },
    device_list: [{ product: "ATK A9 Mouse", vid: 13684, pid: 62597, usage: 1 }],
    plugins_list: [{ id: "demo-plugin", version: "1.0" }],
    keymaps: ["rgb.json", "wave.json", "reactive.json"],
    wallpaper_catalog: { wallpaper: [{ id: 1, title: "Aurora", value: "aurora" }] },
    ipc_categories: { groups: [{ category: "device", channels: ["device.list", "device.write"] }], agentMethods: ["state.get"] },
    endpoint_catalog: { firmwareBase: "https://api-hub-cloud.atk.pro/firmware" },
    music_status: false,
    update_check: { version: "3.2.31", url: "https://example.invalid/atk-hub.exe", sha512: "preview", size: 1234 },
  };
  if (command === "firmware_url") return `https://api-hub-cloud.atk.pro/firmware/${args.vendor_id}/${args.product_id}/${args.tag}`;
  if (command === "device_update_version_url") return `https://${args.host}/v1/device/update/${args.platform}/version`;
  if (command.startsWith("window_")) return null;
  return responses[command] ?? { command, args, preview: true };
}

async function bootstrap() {
  document.documentElement.dataset.theme = localStorage.getItem("atk-theme") ?? "light";
  wireGlobalEvents();
  try {
    state.info = await call("app_info", {}, "Core connected");
    state.pages = state.info.pages ?? [];
  } catch {
    state.pages = previewResponse("app_info").pages;
    state.info = previewResponse("app_info");
    setStatus("Preview mode");
  }
  document.getElementById("app-version").textContent = `v${state.info.version}`;
  renderPage();
}

bootstrap();
