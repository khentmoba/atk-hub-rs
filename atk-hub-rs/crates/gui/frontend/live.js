// Live device pages and settings that talk to the Rust drivers.
import { invoke, state, call, setStatus, escapeHtml, pageHeading, button, field, output, pretty, renderPage } from "./app.js";

/* ---------------------------------------------------------------- i18n -- */

export const LANGUAGES = [
  ["en", "English"], ["zh-CN", "简体中文"], ["zh-TW", "繁體中文"], ["ja", "日本語"], ["ko", "한국어"],
  ["de", "Deutsch"], ["fr", "Français"], ["es", "Español"], ["ru", "Русский"],
];

const PAGE_LABELS = {
  "zh-CN": { overview: "设备中心", devices: "设备", mouse: "鼠标", keyboard: "键盘", lighting: "灯光秀", macros: "宏", audio: "音频", games: "游戏模式", wallpapers: "背景主题", music: "音乐律动", pets: "桌面宠物", plugins: "插件市场", updater: "更新", drivers: "驱动", settings: "设置", diagnostics: "诊断", Device: "设备", Library: "资源库", System: "系统" },
  "zh-TW": { overview: "裝置中心", devices: "裝置", mouse: "滑鼠", keyboard: "鍵盤", lighting: "燈光秀", macros: "巨集", audio: "音訊", games: "遊戲模式", wallpapers: "背景主題", music: "音樂律動", pets: "桌面寵物", plugins: "外掛市集", updater: "更新", drivers: "驅動", settings: "設定", diagnostics: "診斷", Device: "裝置", Library: "資源庫", System: "系統" },
  ja: { overview: "デバイスセンター", devices: "デバイス", mouse: "マウス", keyboard: "キーボード", lighting: "ライトショー", macros: "マクロ", audio: "オーディオ", games: "ゲームモード", wallpapers: "背景テーマ", music: "ミュージック", pets: "デスクトップペット", plugins: "プラグイン", updater: "アップデート", drivers: "ドライバー", settings: "設定", diagnostics: "診断", Device: "デバイス", Library: "ライブラリ", System: "システム" },
  ko: { overview: "장치 센터", devices: "장치", mouse: "마우스", keyboard: "키보드", lighting: "라이트 쇼", macros: "매크로", audio: "오디오", games: "게임 모드", wallpapers: "배경 테마", music: "뮤직", pets: "데스크톱 펫", plugins: "플러그인", updater: "업데이트", drivers: "드라이버", settings: "설정", diagnostics: "진단", Device: "장치", Library: "라이브러리", System: "시스템" },
  de: { overview: "Gerätezentrale", devices: "Geräte", mouse: "Maus", keyboard: "Tastatur", lighting: "Lichtshow", macros: "Makros", audio: "Audio", games: "Spielmodus", wallpapers: "Hintergrund", music: "Musik", pets: "Desktop-Haustiere", plugins: "Plugins", updater: "Update", drivers: "Treiber", settings: "Einstellungen", diagnostics: "Diagnose", Device: "Gerät", Library: "Bibliothek", System: "System" },
  fr: { overview: "Centre des appareils", devices: "Appareils", mouse: "Souris", keyboard: "Clavier", lighting: "Éclairage", macros: "Macros", audio: "Audio", games: "Mode jeu", wallpapers: "Thème d’arrière-plan", music: "Musique", pets: "Animaux de bureau", plugins: "Plugins", updater: "Mise à jour", drivers: "Pilotes", settings: "Paramètres", diagnostics: "Diagnostic", Device: "Appareil", Library: "Bibliothèque", System: "Système" },
  es: { overview: "Centro de dispositivos", devices: "Dispositivos", mouse: "Ratón", keyboard: "Teclado", lighting: "Iluminación", macros: "Macros", audio: "Audio", games: "Modo juego", wallpapers: "Fondo", music: "Música", pets: "Mascotas", plugins: "Plugins", updater: "Actualizar", drivers: "Controladores", settings: "Ajustes", diagnostics: "Diagnóstico", Device: "Dispositivo", Library: "Biblioteca", System: "Sistema" },
  ru: { overview: "Центр устройств", devices: "Устройства", mouse: "Мышь", keyboard: "Клавиатура", lighting: "Подсветка", macros: "Макросы", audio: "Аудио", games: "Игровой режим", wallpapers: "Фон", music: "Музыка", pets: "Питомцы", plugins: "Плагины", updater: "Обновление", drivers: "Драйверы", settings: "Настройки", diagnostics: "Диагностика", Device: "Устройство", Library: "Библиотека", System: "Система" },
};

export function translate(key, fallback) {
  const lang = state.hubSettings?.language ?? "en";
  return PAGE_LABELS[lang]?.[key] ?? fallback;
}

/* --------------------------------------------------------------- helpers -- */

function toggle(label, id, checked) {
  return `<label class="toggle-row"><span>${escapeHtml(label)}</span><input type="checkbox" id="${id}" ${checked ? "checked" : ""} /><span class="switch"></span></label>`;
}

function select(label, id, options, value) {
  const opts = options.map(([v, text]) => `<option value="${escapeHtml(v)}" ${String(v) === String(value) ? "selected" : ""}>${escapeHtml(text)}</option>`).join("");
  return `<div class="form-row"><label for="${id}">${escapeHtml(label)}</label><select id="${id}">${opts}</select></div>`;
}

function range(label, id, min, max, step, value, unit = "") {
  return `<div class="form-row"><label for="${id}">${escapeHtml(label)} <b class="range-value" data-for="${id}">${escapeHtml(value)}${unit}</b></label><input type="range" id="${id}" min="${min}" max="${max}" step="${step}" value="${escapeHtml(value)}" data-unit="${unit}" /></div>`;
}

const $ = (id) => document.getElementById(id);
const num = (id, fallback = 0) => { const v = Number($(id)?.value); return Number.isFinite(v) ? v : fallback; };
const checked = (id) => Boolean($(id)?.checked);

function show(id, value) {
  const el = $(id);
  if (el) el.textContent = pretty(value);
}

function errorText(error) {
  return String(error?.message ?? error);
}

const rgbHex = ([r, g, b]) => `#${[r, g, b].map((v) => Number(v).toString(16).padStart(2, "0")).join("")}`;
const hexRgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) || 0);

/* ----------------------------------------------------------------- mouse -- */

const RATES = [125, 250, 500, 1000, 2000, 4000, 8000];
const BUTTON_NAMES = ["Left button", "Right button", "Middle button", "Back (side 1)", "Forward (side 2)", "DPI button"];
const BUTTON_ACTIONS = [
  ["0,0,0", "Disabled"], ["1,1,0", "Left click"], ["1,2,0", "Right click"], ["1,4,0", "Middle click"],
  ["1,8,0", "Back"], ["1,16,0", "Forward"], ["2,1,0", "DPI loop"], ["2,2,0", "DPI +"], ["2,3,0", "DPI −"],
  ["3,1,0", "Tilt left"], ["3,2,0", "Tilt right"], ["7,0,0", "Report rate switch"], ["9,0,0", "Profile switch"],
  ["10,0,0", "DPI lock"], ["11,1,0", "Wheel up"], ["11,2,0", "Wheel down"], ["12,0,0", "Turn around"],
];

function mouseHeader(snap) {
  if (!snap) {
    return `<div class="notice" id="mouse-notice">${escapeHtml(state.mouseError ?? "Reading mouse…")}</div>`;
  }
  const profiles = [0, 1, 2, 3].map((p) => `<button class="button sm ${snap.profile === p ? "primary" : ""}" data-mouse-profile="${p}" type="button">Profile ${p + 1}</button>`).join("");
  return `
    <div class="mouse-head">
      <img src="assets/mouse.webp" alt="" />
      <div>
        <h2>${escapeHtml(snap.name)}</h2>
        <div class="chip-row">
          <span class="chip live">${snap.charging ? "Charging" : "Battery"} ${escapeHtml(snap.battery)}%</span>
          <span class="chip">Firmware ${escapeHtml(snap.version)}</span>
          <span class="chip">Receiver ${escapeHtml(snap.dongleVersion)}</span>
          <span class="chip">${escapeHtml(snap.sensor)}</span>
        </div>
        <div class="actions" style="margin-top:12px">${profiles}</div>
      </div>
    </div>`;
}

function mouseForms(snap) {
  if (!snap) return "";
  const total = Math.max(1, snap.rate.dpiTotal || 1);
  const stages = snap.stages.map((stage, i) => `
    <div class="dpi-row ${i >= total ? "inactive" : ""}" data-stage="${i}">
      <input type="radio" name="dpi-level" value="${i}" ${snap.rate.dpiLevel === i ? "checked" : ""} title="Active stage" />
      <span class="dpi-index">${i + 1}</span>
      <input type="number" id="dpi-x-${i}" min="50" max="${snap.maxDpi}" step="50" value="${stage.x}" />
      <input type="number" id="dpi-y-${i}" min="50" max="${snap.maxDpi}" step="50" value="${stage.y}" title="Y DPI" />
      <input type="color" id="dpi-color-${i}" value="${rgbHex(stage.color)}" />
    </div>`).join("");
  const buttons = BUTTON_NAMES.map((name, i) => select(name, `mouse-button-${i}`, BUTTON_ACTIONS, (snap.buttons[i] ?? [0, 0, 0]).join(","))).join("");
  return `
    <div class="grid two" style="margin-top:18px">
      <article class="card">
        <div class="card-title"><h2>DPI stages</h2><span>max ${escapeHtml(snap.maxDpi)}</span></div>
        ${range("Enabled stages", "dpi-total", 1, 8, 1, total)}
        <div class="dpi-head"><span></span><span>#</span><span>X</span><span>Y</span><span>Color</span></div>
        <div id="dpi-stages">${stages}</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Performance</h2><span>sensor</span></div>
        ${select("Polling rate", "mouse-rate", RATES.map((r) => [r, `${r} Hz`]), snap.rate.reportRate)}
        ${select("Sensor mode", "mouse-sensor-mode", [[0, "Low power"], [1, "Competitive"], [2, "Competitive MAX"]], snap.performance.sensorMode)}
        ${select("Lift-off distance", "mouse-lift", [[1, "1 mm"], [2, "2 mm"]], snap.liftOff)}
        ${select("Long-distance mode", "mouse-far", [[0, "Off"], [1, "On"]], snap.farDistance)}
        ${range("Debounce", "mouse-debounce", 0, 30, 1, snap.function.debounceMs, " ms")}
        ${range("Sensor angle", "mouse-angle", -30, 30, 1, snap.angle, "°")}
        ${toggle("Motion sync", "mouse-motion-sync", snap.function.motionSync)}
        ${toggle("Angle snapping", "mouse-angle-snap", snap.function.angleSnap)}
        ${toggle("Ripple control", "mouse-ripple", snap.function.rippleControl)}
      </article>
      <article class="card">
        <div class="card-title"><h2>Power</h2><span>sleep timers</span></div>
        ${range("Sleep after", "mouse-sleep", 10, 1200, 10, snap.function.sleepSeconds, " s")}
        ${toggle("Sensor sleep", "mouse-sensor-sleep", snap.performance.sensorSleepEnabled)}
        ${range("Sensor sleep after", "mouse-sensor-sleep-s", 10, 600, 10, snap.performance.sensorSleepSeconds, " s")}
        <div class="card-title" style="margin-top:18px"><h2>DPI light</h2><span>indicator</span></div>
        ${select("Effect", "light-mode", [[0, "Off"], [1, "Steady"], [2, "Breathing"]], snap.light.mode)}
        ${select("Brightness", "light-bright", [[0, "Low"], [1, "Medium"], [2, "High"]], snap.light.brightness)}
        ${select("Speed", "light-speed", [[0, "Slow"], [1, "Medium"], [2, "Fast"]], snap.light.speed)}
      </article>
      <article class="card">
        <div class="card-title"><h2>Buttons</h2><span>onboard remap</span></div>
        ${buttons}
        <div class="actions">${`<button class="button" type="button" data-mouse-buttons>Apply Buttons</button>`}</div>
      </article>
    </div>`;
}

export function mousePage() {
  const snap = state.mouse;
  return `
    ${pageHeading(state.pages.find((page) => page.id === "mouse"))}
    <article class="card">
      ${mouseHeader(snap)}
      <div class="actions" style="margin-top:14px">
        <button class="button" type="button" data-mouse-read>Read from Mouse</button>
        <button class="button primary" type="button" data-mouse-apply ${snap ? "" : "disabled"}>Apply to Mouse</button>
        <button class="button" type="button" data-mouse-pair>Pair Receiver</button>
        <button class="button" type="button" data-mouse-reset>${state.confirmReset ? "Click again to confirm reset" : "Factory Reset"}</button>
      </div>
      ${output("mouse-output")}
    </article>
    ${mouseForms(snap)}
    <div class="grid two" style="margin-top:18px">
      <article class="card">
        <div class="card-title"><h2>Windows pointer</h2><span>system settings</span></div>
        ${range("Pointer speed", "sys-speed", 1, 20, 1, state.systemMouse?.speed ?? 10)}
        ${range("Wheel scroll lines", "sys-wheel", 1, 100, 1, state.systemMouse?.wheelLines ?? 3)}
        <div class="actions"><button class="button" type="button" data-system-mouse>Apply to Windows</button></div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Receivers</h2><span>HID 0xFF02</span></div>
        <div class="list" id="mouse-receivers">Scanning…</div>
      </article>
    </div>`;
}

function readMousePatch() {
  const snap = state.mouse;
  const total = num("dpi-total", snap.rate.dpiTotal);
  const level = Number(document.querySelector("input[name=dpi-level]:checked")?.value ?? snap.rate.dpiLevel);
  const stages = snap.stages.map((stage, i) => ({
    x: num(`dpi-x-${i}`, stage.x),
    y: num(`dpi-y-${i}`, stage.y),
    color: hexRgb($(`dpi-color-${i}`)?.value ?? rgbHex(stage.color)),
  }));
  return {
    stages,
    rate: { ...snap.rate, reportRate: num("mouse-rate", snap.rate.reportRate), dpiTotal: total, dpiLevel: Math.min(level, total - 1) },
    function: {
      debounceMs: num("mouse-debounce"), motionSync: checked("mouse-motion-sync"), sleepSeconds: num("mouse-sleep"),
      angleSnap: checked("mouse-angle-snap"), rippleControl: checked("mouse-ripple"),
    },
    performance: { sensorSleepEnabled: checked("mouse-sensor-sleep"), sensorSleepSeconds: num("mouse-sensor-sleep-s"), sensorMode: num("mouse-sensor-mode") },
    light: { mode: num("light-mode"), brightness: num("light-bright"), speed: num("light-speed") },
    liftOff: num("mouse-lift", snap.liftOff),
    farDistance: num("mouse-far", snap.farDistance),
    angle: num("mouse-angle", snap.angle),
    angleEnabled: num("mouse-angle", 0) !== 0,
  };
}

async function readMouse() {
  try {
    state.mouse = await call("mouse_snapshot", {}, "Mouse read");
    state.mouseError = null;
  } catch (error) {
    state.mouse = null;
    state.mouseError = errorText(error);
  }
  if (state.current === "mouse") renderPage();
}

async function loadMouseExtras() {
  try {
    const receivers = await invoke("mouse_list");
    const box = $("mouse-receivers");
    if (box) {
      box.innerHTML = receivers.length === 0
        ? `<div class="notice">No ATK/VXE receiver or wired mouse found.</div>`
        : receivers.map((r) => `<div class="list-item"><span><strong>${escapeHtml(r.product)}</strong> · VID 0x${r.vendorId.toString(16)} PID 0x${r.productId.toString(16)}</span><span class="chip live">COMPX</span></div>`).join("");
    }
  } catch (error) {
    const box = $("mouse-receivers");
    if (box) box.textContent = errorText(error);
  }
  if (!state.systemMouse) {
    try {
      state.systemMouse = await invoke("system_mouse_get");
      for (const [id, value] of [["sys-speed", state.systemMouse.speed], ["sys-wheel", state.systemMouse.wheelLines]]) {
        const el = $(id);
        if (el) { el.value = value; syncRange(el); }
      }
    } catch { /* non-Windows preview */ }
  }
}

/* -------------------------------------------------------------- keyboard -- */

const HID = {
  4: ["A", "KeyA"], 5: ["B", "KeyB"], 6: ["C", "KeyC"], 7: ["D", "KeyD"], 8: ["E", "KeyE"], 9: ["F", "KeyF"], 10: ["G", "KeyG"],
  11: ["H", "KeyH"], 12: ["I", "KeyI"], 13: ["J", "KeyJ"], 14: ["K", "KeyK"], 15: ["L", "KeyL"], 16: ["M", "KeyM"], 17: ["N", "KeyN"],
  18: ["O", "KeyO"], 19: ["P", "KeyP"], 20: ["Q", "KeyQ"], 21: ["R", "KeyR"], 22: ["S", "KeyS"], 23: ["T", "KeyT"], 24: ["U", "KeyU"],
  25: ["V", "KeyV"], 26: ["W", "KeyW"], 27: ["X", "KeyX"], 28: ["Y", "KeyY"], 29: ["Z", "KeyZ"],
  30: ["1", "Digit1"], 31: ["2", "Digit2"], 32: ["3", "Digit3"], 33: ["4", "Digit4"], 34: ["5", "Digit5"], 35: ["6", "Digit6"],
  36: ["7", "Digit7"], 37: ["8", "Digit8"], 38: ["9", "Digit9"], 39: ["0", "Digit0"],
  40: ["Enter", "Enter"], 41: ["Esc", "Escape"], 42: ["Back", "Backspace"], 43: ["Tab", "Tab"], 44: ["Space", "Space"],
  45: ["-", "Minus"], 46: ["=", "Equal"], 47: ["[", "BracketLeft"], 48: ["]", "BracketRight"], 49: ["\\", "Backslash"],
  50: ["#", "IntlHash"], 51: [";", "Semicolon"], 52: ["'", "Quote"], 53: ["`", "Backquote"], 54: [",", "Comma"], 55: [".", "Period"],
  56: ["/", "Slash"], 57: ["Caps", "CapsLock"], 58: ["F1", "F1"], 59: ["F2", "F2"], 60: ["F3", "F3"], 61: ["F4", "F4"], 62: ["F5", "F5"],
  63: ["F6", "F6"], 64: ["F7", "F7"], 65: ["F8", "F8"], 66: ["F9", "F9"], 67: ["F10", "F10"], 68: ["F11", "F11"], 69: ["F12", "F12"],
  70: ["PrtSc", "PrintScreen"], 71: ["ScrLk", "ScrollLock"], 72: ["Pause", "Pause"], 73: ["Ins", "Insert"], 74: ["Home", "Home"],
  75: ["PgUp", "PageUp"], 76: ["Del", "Delete"], 77: ["End", "End"], 78: ["PgDn", "PageDown"], 79: ["→", "ArrowRight"],
  80: ["←", "ArrowLeft"], 81: ["↓", "ArrowDown"], 82: ["↑", "ArrowUp"], 83: ["Num", "NumLock"], 84: ["/", "NumpadDivide"],
  85: ["*", "NumpadMultiply"], 86: ["-", "NumpadSubtract"], 87: ["+", "NumpadAdd"], 88: ["Enter", "NumpadEnter"],
  89: ["1", "Numpad1"], 90: ["2", "Numpad2"], 91: ["3", "Numpad3"], 92: ["4", "Numpad4"], 93: ["5", "Numpad5"], 94: ["6", "Numpad6"],
  95: ["7", "Numpad7"], 96: ["8", "Numpad8"], 97: ["9", "Numpad9"], 98: ["0", "Numpad0"], 99: [".", "NumpadDecimal"],
  100: ["\\", "IntlBackslash"], 101: ["Menu", "ContextMenu"],
};
const MODIFIERS = [[1, "Ctrl", "ControlLeft"], [2, "Shift", "ShiftLeft"], [4, "Alt", "AltLeft"], [8, "Win", "MetaLeft"],
  [16, "Ctrl", "ControlRight"], [32, "Shift", "ShiftRight"], [64, "Alt", "AltRight"], [128, "Win", "MetaRight"]];

function keyInfo(key) {
  const [mod, usage] = key.defaultKey?.["0"] ?? [0, 0];
  if (usage && HID[usage]) return { label: HID[usage][0], code: HID[usage][1] };
  const modifier = MODIFIERS.find(([bit]) => bit === mod);
  if (modifier) return { label: modifier[1], code: modifier[2] };
  if (mod === 126 || (mod && !usage)) return { label: "Fn", code: "" };
  return { label: "", code: "" };
}

function renderKeyboard() {
  const box = $("keyboard-layout");
  if (!box) return;
  const doc = state.keymapDoc;
  if (!doc) { box.innerHTML = `<div class="notice">Select a keymap to render its layout.</div>`; return; }
  const unit = 44;
  box.innerHTML = doc.layouts.keymap.map((row) => `<div class="kb-row">${row.map((key) => {
    const info = keyInfo(key);
    const style = [`width:${(key.w ?? 1) * unit - 4}px`, `height:${(key.h ?? 1) * unit - 4}px`,
      key.ml ? `margin-left:${key.ml * unit}px` : "", key.mr ? `margin-right:${key.mr * unit}px` : "",
      key.mt ? `margin-top:${key.mt * unit}px` : ""].filter(Boolean).join(";");
    const pressed = info.code && state.testedKeys.has(info.code) ? "tested" : "";
    return `<div class="kb-key ${pressed}" data-code="${escapeHtml(info.code)}" style="${style}" title="row ${key.index[0]}, col ${key.index[1]}">${escapeHtml(info.label)}</div>`;
  }).join("")}</div>`).join("");
  const count = $("kb-tested");
  if (count) count.textContent = `${state.testedKeys.size} key(s) tested`;
}

export function keyboardPage() {
  const maps = (state.keymaps ?? []).map((name) => [name, name.replace(/\.json$/, "")]);
  return `
    ${pageHeading(state.pages.find((page) => page.id === "keyboard"))}
    <article class="card">
      <div class="card-title"><h2>${escapeHtml(state.keymapDoc?.name ?? "Keyboard layout")}</h2><span id="kb-tested">0 key(s) tested</span></div>
      <div class="actions" style="align-items:flex-end">
        ${maps.length ? select("Keymap", "kb-map", maps, state.keymapName ?? maps[0][0]) : ""}
        <button class="button" type="button" data-kb-reset>Reset Tester</button>
      </div>
      <div class="kb-wrap"><div class="kb" id="keyboard-layout">Loading layout…</div></div>
      <div class="notice" style="margin-top:14px">Key tester: press keys on your keyboard; each key lights up when its press is detected.</div>
    </article>`;
}

async function loadKeymap(name) {
  if (!name) return;
  try {
    state.keymapDoc = await invoke("keymap_load", { name });
    state.keymapName = name;
  } catch (error) {
    state.keymapDoc = null;
    const box = $("keyboard-layout");
    if (box) box.textContent = errorText(error);
    return;
  }
  const title = document.querySelector("#page .card-title h2");
  if (title) title.textContent = state.keymapDoc.name ?? name;
  renderKeyboard();
}

async function loadKeyboard() {
  if (!state.keymaps?.length) {
    try { state.keymaps = await invoke("keymaps"); } catch { state.keymaps = []; }
    if (state.current === "keyboard" && state.keymaps.length) { renderPage(); return; }
  }
  await loadKeymap(state.keymapName ?? state.keymaps?.[0]);
}

/* ------------------------------------------------------------ game mode -- */

export function gamesPage() {
  const rules = state.gameRules ?? [];
  const rows = rules.length === 0
    ? `<div class="notice">No games configured.</div>`
    : rules.map((rule, i) => `
      <div class="list-item"><span><strong>${escapeHtml(rule.exe)}</strong> → Profile ${rule.profile + 1}</span>
      <button class="button sm" type="button" data-game-delete="${i}">Remove</button></div>`).join("");
  return `
    ${pageHeading(state.pages.find((page) => page.id === "games"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Auto switch</h2><span>foreground app</span></div>
        ${toggle("Switch mouse profile when a game is focused", "game-enabled", state.hubSettings?.gameSwitchEnabled)}
        <div class="list" id="game-rules" style="margin-top:12px">${rows}</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Add game</h2><span>executable</span></div>
        ${field("Executable", "game-exe", "")}
        ${select("Mouse profile", "game-profile", [[0, "Profile 1"], [1, "Profile 2"], [2, "Profile 3"], [3, "Profile 4"]], 1)}
        <div class="actions">
          <button class="button" type="button" data-game-capture>Pick Focused App (3 s)</button>
          <button class="button primary" type="button" data-game-add>Add Game</button>
        </div>
        <div class="notice" style="margin-top:14px">Leaving a configured game switches the mouse back to Profile 1.</div>
      </article>
    </div>`;
}

async function saveRules(rules) {
  state.gameRules = await call("game_rules_set", { rules }, "Game rules saved");
  renderPage();
}

/* --------------------------------------------------------------- macros -- */

const recorder = { active: false, last: 0, steps: [] };

export function macroEditor() {
  const steps = recorder.steps.map((step, i) => `
    <div class="step-row"><span class="chip">${step.action === "up" ? "↑" : "↓"}</span><span>${escapeHtml(step.key)}</span>
    <input type="number" min="0" max="60000" value="${step.delay_ms}" data-step-delay="${i}" /> ms
    <button class="button sm" type="button" data-step-delete="${i}">✕</button></div>`).join("");
  return `
    <article class="card">
      <div class="card-title"><h2>Macro recorder</h2><span>${recorder.active ? "Recording…" : `${recorder.steps.length} step(s)`}</span></div>
      <div class="actions">
        <button class="button ${recorder.active ? "accent" : "primary"}" type="button" data-macro-record>${recorder.active ? "Stop Recording" : "Record"}</button>
        <button class="button" type="button" data-macro-clear>Clear</button>
        <button class="button" type="button" data-macro-test ${recorder.steps.length ? "" : "disabled"}>Test Run</button>
      </div>
      <div class="steps" id="macro-steps">${steps || `<div class="notice">Press Record, then type the sequence. Key downs, key ups and the delays between them are captured.</div>`}</div>
      ${field("Macro name", "rec-name", "Recorded macro")}
      ${select("Repeat", "rec-mode", [["count", "Repeat a number of times"], ["while-held", "Repeat while the key is held"], ["toggle", "Toggle on / off"]], "count")}
      ${field("Repeat count", "rec-count", "1", "number")}
      <div class="actions"><button class="button primary" type="button" data-macro-save ${recorder.steps.length ? "" : "disabled"}>Save Macro</button></div>
    </article>`;
}

function rerenderRecorder() {
  const host = $("macro-recorder");
  if (host) host.innerHTML = macroEditor();
}

/* ------------------------------------------------------------- settings -- */

export function hubSettingsCard() {
  const s = state.hubSettings ?? {};
  return `
    <article class="card">
      <div class="card-title"><h2>General</h2><span>setting.*</span></div>
      ${select("Language", "hs-language", LANGUAGES, s.language ?? "en")}
      ${toggle("Launch at Windows sign-in", "hs-open-at-login", s.openAtLogin)}
      ${toggle("Close button minimizes to tray", "hs-close-tray", s.closeMinimizeTray)}
      ${toggle("Ask before exiting", "hs-exit-prompt", s.exitDetectionPromptEnabled)}
      ${toggle("Show Caps / Num Lock hint", "hs-lock-hint", s.keyboardLockHintEnabled)}
      ${toggle("Global show / hide hotkey", "hs-hotkey", s.globalHubHotkeyEnabled)}
      ${field("Hotkey", "hs-accelerator", s.globalHubHotkeyAccelerator ?? "Alt+Shift+H")}
      <div class="actions"><button class="button primary" type="button" data-hub-settings>Save Settings</button></div>
      ${output("hub-settings-output")}
    </article>`;
}

async function saveHubSettings(patch = {}) {
  const current = state.hubSettings ?? {};
  const onPage = Boolean($("hs-language"));
  const settings = {
    ...current,
    ...(onPage ? {
      language: $("hs-language").value,
      openAtLogin: checked("hs-open-at-login"),
      closeMinimizeTray: checked("hs-close-tray"),
      exitDetectionPromptEnabled: checked("hs-exit-prompt"),
      keyboardLockHintEnabled: checked("hs-lock-hint"),
      globalHubHotkeyEnabled: checked("hs-hotkey"),
      globalHubHotkeyAccelerator: $("hs-accelerator").value,
    } : {}),
    ...patch,
  };
  try {
    state.hubSettings = await call("hub_settings_set", { settings }, "Settings saved");
    show("hub-settings-output", state.hubSettings);
    if (settings.language !== current.language) renderPage();
  } catch (error) {
    show("hub-settings-output", errorText(error));
  }
}

/* ----------------------------------------------------------------- pets -- */

export function petControls() {
  const p = state.petState ?? {};
  return `
    ${toggle("Wooden fish enabled", "pet-enabled", p.enabled)}
    ${toggle("Show desktop pet", "pet-visible", p.petVisible)}
    ${toggle("Knock sound", "pet-sound", p.sound)}
    <div class="actions"><button class="button primary" type="button" data-pet-save>Save Pet Settings</button></div>`;
}

/* --------------------------------------------------------------- events -- */

function syncRange(input) {
  const label = document.querySelector(`.range-value[data-for="${input.id}"]`);
  if (label) label.textContent = `${input.value}${input.dataset.unit ?? ""}`;
}

async function guarded(target, work) {
  target.disabled = true;
  try { await work(); } catch (error) { setStatus(errorText(error)); show("mouse-output", errorText(error)); } finally { target.disabled = false; }
}

export function wireLiveEvents() {
  document.addEventListener("click", async (event) => {
    const t = event.target.closest("button, [data-mouse-profile]");
    if (!t) return;
    const d = t.dataset;
    if ("mouseRead" in d) return guarded(t, readMouse);
    if ("mouseApply" in d) {
      return guarded(t, async () => {
        state.mouse = await call("mouse_apply", { patch: readMousePatch() }, "Mouse settings applied");
        renderPage();
      });
    }
    if ("mouseButtons" in d) {
      return guarded(t, async () => {
        for (let i = 0; i < BUTTON_NAMES.length; i += 1) {
          const mapping = $(`mouse-button-${i}`).value.split(",").map(Number);
          if (mapping.join(",") !== (state.mouse.buttons[i] ?? []).join(",")) {
            await call("mouse_button", { index: i, mapping }, `Button ${i + 1} remapped`);
            state.mouse.buttons[i] = mapping;
          }
        }
        show("mouse-output", "Buttons applied");
      });
    }
    if (d.mouseProfile !== undefined) {
      return guarded(t, async () => {
        state.mouse = await call("mouse_profile", { profile: Number(d.mouseProfile) }, "Profile switched");
        renderPage();
      });
    }
    if ("mousePair" in d) {
      return guarded(t, async () => {
        await call("mouse_pair", {}, "Pairing started: turn the mouse off and on");
        for (let i = 0; i < 30; i += 1) {
          await new Promise((resolve) => setTimeout(resolve, 1000));
          const status = await invoke("mouse_pair_status").catch(() => null);
          show("mouse-output", status ?? "Waiting for the mouse…");
          if (status && status.status === 1) { setStatus("Paired"); break; }
        }
      });
    }
    if ("mouseReset" in d) {
      if (!state.confirmReset) { state.confirmReset = true; t.textContent = "Click again to confirm reset"; return undefined; }
      state.confirmReset = false;
      return guarded(t, async () => { await call("mouse_reset", {}, "Factory settings restored"); await readMouse(); });
    }
    if ("systemMouse" in d) {
      return guarded(t, async () => {
        await call("system_mouse_set", { speed: num("sys-speed", 10), wheelLines: num("sys-wheel", 3) }, "Windows pointer updated");
        state.systemMouse = { speed: num("sys-speed", 10), wheelLines: num("sys-wheel", 3) };
      });
    }
    if ("kbReset" in d) { state.testedKeys.clear(); renderKeyboard(); return undefined; }
    if ("gameAdd" in d) {
      const exe = $("game-exe").value.trim();
      if (!exe) return setStatus("Enter an executable name, e.g. game.exe");
      return saveRules([...(state.gameRules ?? []), { exe, profile: num("game-profile", 1) }]);
    }
    if (d.gameDelete !== undefined) {
      return saveRules((state.gameRules ?? []).filter((_, i) => i !== Number(d.gameDelete)));
    }
    if ("gameCapture" in d) {
      setStatus("Switch to the game window now…");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const exe = await invoke("foreground_process").catch(() => null);
      if (exe && $("game-exe")) $("game-exe").value = exe;
      setStatus(exe ? `Captured ${exe}` : "Could not read the focused app");
      return undefined;
    }
    if ("macroRecord" in d) {
      recorder.active = !recorder.active;
      recorder.last = performance.now();
      rerenderRecorder();
      return undefined;
    }
    if ("macroClear" in d) { recorder.steps = []; recorder.active = false; rerenderRecorder(); return undefined; }
    if (d.stepDelete !== undefined) { recorder.steps.splice(Number(d.stepDelete), 1); rerenderRecorder(); return undefined; }
    if ("macroTest" in d) {
      return guarded(t, async () => { show("output", await call("macros_run_steps", { steps: recorder.steps, repeat: Math.max(1, num("rec-count", 1)) }, "Macro played")); });
    }
    if ("macroSave" in d) {
      return guarded(t, async () => {
        const macros = await call("macros_add", {
          name: $("rec-name").value || "Recorded macro",
          steps: JSON.stringify(recorder.steps),
          repeatMode: $("rec-mode").value,
          repeatCount: Math.max(1, num("rec-count", 1)),
        }, "Macro added");
        await call("macros_save", {}, "Macro saved");
        state.macros = macros;
        recorder.steps = [];
        renderPage();
      });
    }
    if ("hubSettings" in d) return guarded(t, () => saveHubSettings());
    if ("petSave" in d) {
      return guarded(t, async () => {
        const settings = { ...(state.petState ?? {}), enabled: checked("pet-enabled"), petVisible: checked("pet-visible"), sound: checked("pet-sound") };
        state.petState = await call("pets_save", { path: state.petState?.path ?? state.info?.petPath ?? "", settings }, "Pet settings saved");
        show("output", state.petState);
      });
    }
    if (d.wallpaperApply !== undefined) {
      return guarded(t, async () => {
        const result = await call("wallpaper_apply", { url: d.wallpaperApply, apply: true }, "Wallpaper downloaded");
        setStatus(result.applied ? "Desktop wallpaper applied" : `Saved to ${result.path}`);
        show("output", result);
      });
    }
    if ("updateDownload" in d) {
      return guarded(t, async () => {
        state.updatePath = await call("update_download", {}, "Update downloaded and verified");
        show("output", state.updatePath);
        renderPage();
      });
    }
    if ("updateInstall" in d) return guarded(t, () => call("installer_launch", { path: state.updatePath }, "Installer started"));
    if ("driverInstall" in d) {
      return guarded(t, async () => { show("output", await call("driver_install", { sha256: $("sha256")?.value ?? "" }, "Driver installer started")); });
    }
    return undefined;
  });

  document.addEventListener("change", (event) => {
    if (event.target.id === "kb-map") loadKeymap(event.target.value);
    if (event.target.id === "game-enabled") saveHubSettings({ gameSwitchEnabled: event.target.checked });
    if (event.target.id === "dpi-total") {
      document.querySelectorAll(".dpi-row").forEach((row) => row.classList.toggle("inactive", Number(row.dataset.stage) >= Number(event.target.value)));
    }
    if (event.target.dataset.stepDelay !== undefined) {
      recorder.steps[Number(event.target.dataset.stepDelay)].delay_ms = Math.max(0, Number(event.target.value) || 0);
    }
  });

  document.addEventListener("input", (event) => {
    if (event.target.type === "range") syncRange(event.target);
  });

  const record = (action) => (event) => {
    if (recorder.active) {
      if (event.repeat) return;
      event.preventDefault();
      const now = performance.now();
      if (recorder.steps.length) recorder.steps[recorder.steps.length - 1].delay_ms = Math.round(now - recorder.last);
      recorder.last = now;
      recorder.steps.push({ key: event.key.length === 1 ? event.key.toUpperCase() : event.key, delay_ms: 20, action, vk: event.keyCode });
      rerenderRecorder();
      return;
    }
    if (state.current === "keyboard" && action === "down") {
      event.preventDefault();
      state.testedKeys.add(event.code);
      document.querySelectorAll(`.kb-key[data-code="${CSS.escape(event.code)}"]`).forEach((el) => el.classList.add("tested", "down"));
      const count = $("kb-tested");
      if (count) count.textContent = `${state.testedKeys.size} key(s) tested`;
    } else if (state.current === "keyboard") {
      document.querySelectorAll(`.kb-key[data-code="${CSS.escape(event.code)}"]`).forEach((el) => el.classList.remove("down"));
    }
  };
  document.addEventListener("keydown", record("down"));
  document.addEventListener("keyup", record("up"));

  window.__TAURI__?.event?.listen("game-switch", ({ payload }) => {
    setStatus(payload.exe ? `${payload.exe} focused · Profile ${payload.profile + 1}` : "Back to Profile 1");
  });
}

export async function loadLivePage(pageId) {
  if (pageId === "mouse") {
    loadMouseExtras();
    if (!state.mouse && !state.mouseLoading) {
      state.mouseLoading = true;
      await readMouse();
      state.mouseLoading = false;
    }
  } else if (pageId === "keyboard") {
    await loadKeyboard();
  } else if (pageId === "games") {
    if (!state.gameRules) {
      state.gameRules = await invoke("game_rules_get").catch(() => []);
      renderPage();
    }
  }
}

export async function loadHubSettings() {
  try {
    state.hubSettings = await invoke("hub_settings_get");
  } catch {
    state.hubSettings = { language: "en", closeMinimizeTray: true, exitDetectionPromptEnabled: true };
  }
}
