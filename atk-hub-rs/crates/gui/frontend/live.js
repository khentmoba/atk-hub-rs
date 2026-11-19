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
  const buttons = BUTTON_NAMES.map((name, i) => {
    const current = (snap.buttons[i] ?? [0, 0, 0]).join(",");
    const known = BUTTON_ACTIONS.some(([value]) => value === current);
    const [cls, v1, v2] = snap.buttons[i] ?? [0, 0, 0];
    const label = cls === 5 ? "Key combination" : cls === 6 ? `Macro M${v1 + 1}` : cls === 4 ? `Fire key (${v2}×/${v1} ms)` : `Custom [${current}]`;
    return select(name, `mouse-button-${i}`, known ? BUTTON_ACTIONS : [[current, label], ...BUTTON_ACTIONS], current);
  }).join("");
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
      ${onboardCard()}
    </div>`;
}

/* ---------------------------------------------- onboard macros & keys -- */

const MOD_BITS = { Control: 1, Shift: 2, Alt: 4, Meta: 8, Win: 8, Ctrl: 1 };
const VK_MOD = { 16: 2, 17: 1, 18: 4, 91: 8, 92: 128, 160: 2, 161: 32, 162: 1, 163: 16, 164: 4, 165: 64 };
const NAMED_USAGE = {
  ENTER: 40, ESC: 41, ESCAPE: 41, BACKSPACE: 42, TAB: 43, SPACE: 44, " ": 44, "-": 45, "=": 46, "[": 47, "]": 48,
  "\\": 49, ";": 51, "'": 52, "`": 53, ",": 54, ".": 55, "/": 56, CAPSLOCK: 57, PRINTSCREEN: 70, SCROLLLOCK: 71,
  PAUSE: 72, INSERT: 73, HOME: 74, PAGEUP: 75, DELETE: 76, DEL: 76, END: 77, PAGEDOWN: 78, ARROWRIGHT: 79, RIGHT: 79,
  ARROWLEFT: 80, LEFT: 80, ARROWDOWN: 81, DOWN: 81, ARROWUP: 82, UP: 82,
};
const VK_USAGE = { 8: 42, 9: 43, 13: 40, 20: 57, 27: 41, 32: 44, 33: 75, 34: 78, 35: 77, 36: 74, 37: 80, 38: 82, 39: 79, 40: 81,
  44: 70, 45: 73, 46: 76, 186: 51, 187: 46, 188: 54, 189: 45, 190: 55, 191: 56, 192: 53, 219: 47, 220: 49, 221: 48, 222: 52 };

function usageForName(name) {
  const key = String(name).trim().toUpperCase();
  if (/^[A-Z]$/.test(key)) return 4 + key.charCodeAt(0) - 65;
  if (/^[1-9]$/.test(key)) return 30 + Number(key) - 1;
  if (key === "0") return 39;
  const f = /^F([1-9]|1[0-2])$/.exec(key);
  if (f) return 57 + Number(f[1]);
  return NAMED_USAGE[key] ?? null;
}

function usageForVk(vk) {
  if (vk >= 65 && vk <= 90) return 4 + vk - 65;
  if (vk >= 49 && vk <= 57) return 30 + vk - 49;
  if (vk === 48) return 39;
  if (vk >= 112 && vk <= 123) return 58 + vk - 112;
  return VK_USAGE[vk] ?? null;
}

export function parseShortcut(text) {
  const parts = String(text).split("+").map((part) => part.trim()).filter(Boolean);
  const downs = [];
  for (const part of parts) {
    const bit = MOD_BITS[part[0].toUpperCase() + part.slice(1).toLowerCase()] ?? MOD_BITS[part];
    if (bit) { downs.push({ kind: "modifier", down: true, value: bit, delayMs: 0 }); continue; }
    const usage = usageForName(part);
    if (usage === null) throw new Error(`Unknown key: ${part}`);
    downs.push({ kind: "key", down: true, value: usage, delayMs: 0 });
  }
  if (downs.length === 0) throw new Error("Enter a key combination such as Ctrl+C");
  const ups = [...downs].reverse().map((event) => ({ ...event, down: false }));
  const events = downs.length * 2 <= 6 ? [...downs, ...ups] : downs;
  if (events.length > 6) throw new Error("The mouse stores at most 6 keys per shortcut");
  return events;
}

export function stepsToEvents(steps) {
  const events = [];
  for (const step of steps ?? []) {
    const delayMs = Math.min(65535, Math.max(0, Number(step.delay_ms) || 0));
    const mod = step.vk !== undefined ? VK_MOD[step.vk] : MOD_BITS[step.key];
    const usage = step.vk !== undefined ? usageForVk(step.vk) : usageForName(step.key);
    const kind = mod ? "modifier" : "key";
    const value = mod ?? usage;
    if (value === null || value === undefined) throw new Error(`Key ${step.key} cannot be stored on the mouse`);
    if (step.action) {
      events.push({ kind, down: step.action === "down", value, delayMs });
    } else {
      events.push({ kind, down: true, value, delayMs: 20 }, { kind, down: false, value, delayMs });
    }
  }
  if (events.length > 70) throw new Error("The mouse stores at most 70 events per macro");
  return events;
}

function describeEvents(events) {
  if (!events?.length) return "Empty";
  return events.map((e) => {
    const name = e.kind === "modifier" ? (MODIFIERS.find(([bit]) => bit === e.value)?.[1] ?? `mod${e.value}`)
      : e.kind === "key" ? (HID[e.value]?.[0] ?? `#${e.value}`) : `${e.kind}:${e.value}`;
    return `${e.down ? "↓" : "↑"}${name}${e.delayMs ? ` ${e.delayMs}ms` : ""}`;
  }).join("  ");
}

function onboardCard() {
  const buttonOptions = BUTTON_NAMES.map((name, i) => [i, name]);
  const macros = (state.macros ?? []).map((m, i) => [i, m.name]);
  return `
      <article class="card">
        <div class="card-title"><h2>Shortcut keys, macros & fire key</h2><span>stored on the mouse</span></div>
        ${select("Button", "ob-button", buttonOptions, 3)}
        ${select("Assign", "ob-kind", [["shortcut", "Key combination"], ["macro", "Onboard macro"], ["fire", "Fire key (rapid click)"]], state.obKind ?? "shortcut")}
        <div ${state.obKind && state.obKind !== "shortcut" ? "hidden" : ""} data-ob="shortcut">
          ${field("Keys", "ob-keys", "Ctrl+C")}
        </div>
        <div ${state.obKind === "macro" ? "" : "hidden"} data-ob="macro">
          ${select("Macro slot", "ob-slot", Array.from({ length: 12 }, (_, i) => [i, `M${i + 1}`]), 0)}
          ${macros.length ? select("Recorded macro", "ob-macro", macros, 0) : `<div class="notice">Record a macro on the Macros page first.</div>`}
          ${select("Playback", "ob-playback", [["loop", "Play a number of times"], ["stopOnRelease", "Repeat while held"], ["untilSameKey", "Toggle: until pressed again"], ["untilAnyKey", "Until any key is pressed"]], "loop")}
          ${field("Times", "ob-count", "1", "number")}
        </div>
        <div ${state.obKind === "fire" ? "" : "hidden"} data-ob="fire">
          ${range("Click interval", "ob-interval", 10, 255, 1, 40, " ms")}
          ${range("Clicks per press", "ob-clicks", 1, 255, 1, 3)}
        </div>
        <div class="actions">
          <button class="button primary" type="button" data-ob-write>Write to Mouse</button>
          <button class="button" type="button" data-ob-read>Read from Mouse</button>
        </div>
        <div class="output" id="ob-output">Shortcuts and macros keep working without ATK HUB running.</div>
      </article>`;
}

async function writeOnboard() {
  const button = num("ob-button", 3);
  const kind = $("ob-kind").value;
  if (kind === "shortcut") {
    const events = parseShortcut($("ob-keys").value);
    await call("mouse_shortcut_write", { button, events }, "Shortcut stored on the mouse");
    state.mouse.buttons[button] = [5, 0, 0];
    show("ob-output", describeEvents(events));
  } else if (kind === "macro") {
    const macro = state.macros?.[num("ob-macro", 0)];
    if (!macro) throw new Error("Record a macro on the Macros page first");
    const events = stepsToEvents(macro.steps);
    const mode = $("ob-playback").value;
    const playback = mode === "loop" ? { mode: "loop", count: Math.max(1, Math.min(252, num("ob-count", 1))) } : { mode };
    const slot = num("ob-slot", 0);
    await call("mouse_macro_write", { slot, events, button, playback }, `Macro stored in M${slot + 1}`);
    show("ob-output", describeEvents(events));
  } else {
    const mapping = [4, num("ob-interval", 40), num("ob-clicks", 3)];
    await call("mouse_button", { index: button, mapping }, "Fire key assigned");
    state.mouse.buttons[button] = mapping;
    show("ob-output", `Fire key: ${mapping[2]} clicks every ${mapping[1]} ms`);
  }
}

async function readOnboard() {
  const button = num("ob-button", 3);
  const mapping = state.mouse?.buttons?.[button] ?? [];
  if ($("ob-kind").value === "macro") {
    const slot = num("ob-slot", 0);
    show("ob-output", `M${slot + 1}: ${describeEvents(await call("mouse_macro_read", { slot }, "Macro read"))}`);
  } else {
    const events = await call("mouse_shortcut_read", { button }, "Shortcut read");
    show("ob-output", `Mapping [${mapping.join(", ")}] · shortcut: ${describeEvents(events)}`);
  }
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
  const live = state.kbMap;
  box.innerHTML = doc.layouts.keymap.map((row) => `<div class="kb-row">${row.map((key) => {
    const info = keyInfo(key);
    const [r, c] = key.index ?? [0, 0];
    const code = live?.[r]?.[c];
    const label = code === undefined ? info.label : kbLabel(code);
    const selected = state.kbSel && state.kbSel[0] === r && state.kbSel[1] === c ? "selected" : "";
    const style = [`width:${(key.w ?? 1) * unit - 4}px`, `height:${(key.h ?? 1) * unit - 4}px`,
      key.ml ? `margin-left:${key.ml * unit}px` : "", key.mr ? `margin-right:${key.mr * unit}px` : "",
      key.mt ? `margin-top:${key.mt * unit}px` : ""].filter(Boolean).join(";");
    const pressed = info.code && state.testedKeys.has(info.code) ? "tested" : "";
    return `<div class="kb-key ${pressed} ${selected}" data-code="${escapeHtml(info.code)}" data-r="${r}" data-c="${c}" style="${style}" title="row ${r}, col ${c}${code === undefined ? "" : ` · 0x${code.toString(16)}`}">${escapeHtml(label)}</div>`;
  }).join("")}</div>`).join("");
  const count = $("kb-tested");
  if (count) count.textContent = `${state.testedKeys.size} key(s) tested`;
}

export function keyboardPage() {
  const maps = (state.keymaps ?? []).map((name) => [name, name.replace(/\.json$/, "")]);
  return `
    ${pageHeading(state.pages.find((page) => page.id === "keyboard"))}
    ${kbDeviceCards()}
    <article class="card">
      <div class="card-title"><h2>${escapeHtml(state.keymapDoc?.name ?? "Keyboard layout")}</h2><span id="kb-tested">0 key(s) tested</span></div>
      <div class="actions" style="align-items:flex-end">
        ${maps.length ? select("Keymap", "kb-map", maps, state.keymapName ?? maps[0][0]) : ""}
        <button class="button" type="button" data-kb-reset>Reset Tester</button>
        ${state.kb ? select("Layer", "kb-layer", Array.from({ length: state.kb.model.layers }, (_, i) => [i, i === 0 ? "Layer 1 (base)" : `Layer ${i + 1} (Fn${i > 1 ? i : ""})`]), state.kbLayer ?? 0) : ""}
      </div>
      ${state.kb ? kbRemapPanel() : ""}
      <div class="kb-wrap"><div class="kb" id="keyboard-layout">Loading layout…</div></div>
      <div class="notice" style="margin-top:14px">Key tester: press keys on your keyboard; each key lights up when its press is detected.</div>
    </article>`;
}

/* ------------------------------------------------- keyboard: device side -- */

const MEDIA = { 165: "Power", 166: "Sleep", 167: "Wake", 168: "Mute", 169: "Vol+", 170: "Vol-", 171: "Next", 172: "Prev",
  173: "Stop", 174: "Play", 175: "Player", 176: "Eject", 177: "Mail", 178: "Calc", 179: "My PC", 180: "Search", 181: "Home page",
  182: "Back", 183: "Forward", 184: "Web stop", 185: "Refresh", 186: "Favorites", 187: "FF", 188: "Rewind", 189: "Bright+", 190: "Bright-",
  193: "Mission Ctrl", 194: "Launchpad", 2055: "Desktop", 2063: "Lock PC" };
const MOD_KEYS = { 224: "LCtrl", 225: "LShift", 226: "LAlt", 227: "LWin", 228: "RCtrl", 229: "RShift", 230: "RAlt", 231: "RWin" };
const YOGO = { 32256: "Reset", 32257: "BT1", 32258: "BT2", 32259: "BT3", 32260: "2.4G", 32261: "USB", 32262: "Battery", 32263: "NKRO",
  32264: "Wireless", 32266: "WASD", 32268: "BL Tog", 32269: "BL Mode+", 32270: "BL Mode-", 32273: "BL Bri+", 32274: "BL Bri-",
  32275: "BL Spd+", 32276: "BL Spd-" };
const LAYER_KINDS = ["TO", "MO", "DF", "TG", "OSL", "TT"];

/** QMK keycode bases: protocol 11+ (VIA v3) moved the layer, macro, mouse and RGB ranges. */
function kbBases() {
  const modern = (state.kb?.protocol ?? 9) >= 11;
  return modern
    ? { layer: [20992, 21024, 21056, 21088, 21120, 21184], macro: 30464, mouse: 205, rgb: 30752 }
    : { layer: [20496, 20736, 20992, 21248, 21504, 22528], macro: 24338, mouse: 240, rgb: 23746 };
}

const MOUSE_KEYS = ["Mouse ↑", "Mouse ↓", "Mouse ←", "Mouse →", "Click L", "Click R", "Click M", "Mouse 4", "Mouse 5", "Wheel ↑", "Wheel ↓", "Wheel ←", "Wheel →"];
const RGB_KEYS = ["RGB Toggle", "RGB Mode+", "RGB Mode-", "Hue+", "Hue-", "Sat+", "Sat-", "Bright+", "Bright-", "Speed+", "Speed-"];

export function kbLabel(code) {
  if (code === 0) return "";
  if (code === 1) return "▽";
  if (HID[code]) return HID[code][0];
  if (MOD_KEYS[code]) return MOD_KEYS[code];
  if (MEDIA[code]) return MEDIA[code];
  if (YOGO[code]) return YOGO[code];
  const b = kbBases();
  for (let k = 0; k < LAYER_KINDS.length; k += 1) {
    if (code >= b.layer[k] && code < b.layer[k] + 16) return `${LAYER_KINDS[k]}(${code - b.layer[k]})`;
  }
  if (code >= b.macro && code < b.macro + 16) return `M${code - b.macro}`;
  if (code >= b.mouse && code < b.mouse + MOUSE_KEYS.length) return MOUSE_KEYS[code - b.mouse];
  if (code >= b.rgb && code < b.rgb + RGB_KEYS.length) return RGB_KEYS[code - b.rgb];
  if (code >= 0x100 && code < 0x2000) {
    const mods = (code >> 8) & 0x1f;
    const right = mods & 0x10;
    const names = [[1, "Ctrl"], [2, "Shift"], [4, "Alt"], [8, "Win"]].filter(([bit]) => mods & bit).map(([, n]) => (right ? "R" : "") + n);
    return `${names.join("+")}+${HID[code & 0xff]?.[0] ?? (code & 0xff)}`;
  }
  return `0x${code.toString(16)}`;
}

function kbKeyGroups() {
  const b = kbBases();
  const basic = Object.keys(HID).map(Number).map((code) => [code, HID[code][0]]);
  return [
    ["Basic", [[0, "None (disable)"], [1, "Transparent ▽"], ...basic]],
    ["Modifiers", Object.entries(MOD_KEYS).map(([c, n]) => [Number(c), n])],
    ["Media & system", Object.entries(MEDIA).map(([c, n]) => [Number(c), n])],
    ...LAYER_KINDS.map((kind, k) => [`${kind} layer`, Array.from({ length: state.kb?.model.layers ?? 4 }, (_, n) => [b.layer[k] + n, `${kind}(${n})`])]),
    ["Macros", Array.from({ length: 8 }, (_, n) => [b.macro + n, `M${n}`])],
    ["Mouse", MOUSE_KEYS.map((name, n) => [b.mouse + n, name])],
    ["Lighting", RGB_KEYS.map((name, n) => [b.rgb + n, name])],
    ["Keyboard functions", Object.entries(YOGO).map(([c, n]) => [Number(c), n])],
  ];
}

function kbRemapPanel() {
  const sel = state.kbSel;
  const current = sel ? state.kbMap?.[sel[0]]?.[sel[1]] : undefined;
  const groups = kbKeyGroups().map(([label, options]) => `<optgroup label="${escapeHtml(label)}">${options.map(([value, name]) =>
    `<option value="${value}" ${value === current ? "selected" : ""}>${escapeHtml(name)}</option>`).join("")}</optgroup>`).join("");
  return `
      <div class="actions" style="align-items:flex-end;margin-top:10px">
        <label class="field"><span>${sel ? `Key r${sel[0]} c${sel[1]} · now ${escapeHtml(kbLabel(current ?? 0) || "None")}` : "Click a key above to remap it"}</span>
          <select id="kb-code" ${sel ? "" : "disabled"}>${groups}</select></label>
        <label class="field"><span>Combo modifier</span><select id="kb-mod" ${sel ? "" : "disabled"}>
          <option value="0">None</option><option value="1">Ctrl+</option><option value="2">Shift+</option><option value="4">Alt+</option><option value="8">Win+</option>
          <option value="3">Ctrl+Shift+</option><option value="5">Ctrl+Alt+</option></select></label>
        <button class="button primary" type="button" data-kb-assign ${sel ? "" : "disabled"}>Assign Key</button>
        <button class="button" type="button" data-kb-reset-map>Restore Default Keymap</button>
      </div>`;
}

const LIGHT_EFFECTS = [[0, "Off"], [1, "Custom"], [2, "Static"], [3, "Focus"], [4, "Rainbow ↕"], [5, "Rainbow ↔"], [6, "Breathing"],
  [7, "Ribbon"], [8, "Laser"], [9, "Pinwheel"], [11, "Vortex"], [13, "Spectrum cycle"], [14, "Streamer"], [19, "Colorful swirl"],
  [23, "Double vortex"], [25, "Christmas"], [26, "Spectrum breathing"], [28, "Spectrum floating"], [30, "Digital rain"],
  [31, "Light up on press"], [36, "Crucifix"]];
const SLEEP = [[0, "Never"], [60, "1 min"], [180, "3 min"], [300, "5 min"], [600, "10 min"], [1800, "30 min"]];

function hex(rgb) {
  return `#${(rgb ?? [255, 255, 255]).map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

function kbDeviceCards() {
  const kb = state.kb;
  if (!kb) {
    return `
    <article class="card">
      <div class="card-title"><h2>Keyboard</h2><span>${escapeHtml(state.kbError ?? "Not connected")}</span></div>
      <div class="actions"><button class="button primary" type="button" data-kb-read>Connect Keyboard</button></div>
      <div class="notice">Plug in an ATK / VXE magnetic keyboard (wired or 2.4G) to remap keys, set actuation and rapid trigger, and change lighting.</div>
    </article>`;
  }
  const t = kb.travel;
  const actMin = t.travelMin || 0.1;
  const actMax = t.travelMax || 3.4;
  const actStep = t.travelStep || 0.01;
  const rtMin = t.rtMin || 0.01;
  const rtMax = t.rtMax || 2;
  const rtStep = t.rtStep || 0.01;
  const l = kb.lighting;
  const f = kb.features;
  const profiles = [0, 1, 2, 3].map((p) => `<button class="button sm ${p === kb.profile ? "primary" : ""}" type="button" data-kb-profile="${p}">Profile ${p + 1}</button>`).join("");
  return `
    <article class="card">
      <div class="mouse-head">
        <div><h2>${escapeHtml(kb.model.name)}</h2><span>Firmware ${escapeHtml(kb.version || "?")} · VIA ${kb.protocol} · ${kb.model.rows}×${kb.model.cols}, ${kb.model.layers} layers</span></div>
        <div class="actions">${profiles}<button class="button sm" type="button" data-kb-read>Refresh</button></div>
      </div>
    </article>
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Backlight</h2><span>${escapeHtml(LIGHT_EFFECTS.find(([v]) => v === l.mode)?.[1] ?? `Effect ${l.mode}`)}</span></div>
        ${select("Effect", "kb-effect", LIGHT_EFFECTS, l.mode)}
        ${range("Brightness", "kb-bright", 0, 100, 1, l.brightness)}
        ${range("Speed", "kb-speed", 0, 255, 1, l.speed)}
        <label class="field"><span>Colour</span><input id="kb-color" type="color" value="${hex(l.color)}"></label>
        ${toggle("Rainbow colours", "kb-colorful", l.colorful)}
        ${select("Sleep after", "kb-sleep", SLEEP, kb.sleepSecs)}
        <div class="actions"><button class="button primary" type="button" data-kb-light>Apply Lighting</button></div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Functions</h2><span>${kb.reportRate} Hz</span></div>
        ${select("Polling rate", "kb-rate", [[1000, "1000 Hz"], [2000, "2000 Hz"], [4000, "4000 Hz"], [8000, "8000 Hz"]], kb.reportRate)}
        ${toggle("Swap WASD and arrow keys", "kb-wasd", f.wasdSwap)}
        ${toggle("Lock Windows key", "kb-winlock", f.winLock)}
        ${toggle("macOS layout", "kb-mac", f.mac)}
        ${toggle("Performance (game) mode", "kb-game", kb.gameMode)}
        <div class="actions"><button class="button primary" type="button" data-kb-functions>Apply Functions</button></div>
      </article>
    </div>
    <article class="card">
      <div class="card-title"><h2>Magnetic switches</h2><span>${state.kbSel ? `selected key r${state.kbSel[0]} c${state.kbSel[1]}` : "all keys"}</span></div>
      <div class="grid two">
        <div>
          ${range("Actuation point", "kb-act", actMin, actMax, actStep, state.kbAct ?? Math.min(actMax, Math.max(actMin, 1.2)), " mm")}
          ${toggle("Rapid trigger", "kb-rt", state.kbRtOn ?? false)}
          ${range("Press sensitivity", "kb-rt-press", rtMin, rtMax, rtStep, state.kbRtPress ?? Math.max(rtMin, 0.1), " mm")}
          ${range("Release sensitivity", "kb-rt-release", rtMin, rtMax, rtStep, state.kbRtRelease ?? Math.max(rtMin, 0.1), " mm")}
        </div>
        <div>
          ${range("Top dead zone", "kb-dz-top", 0, 1, 0.01, kb.deadZone[0], " mm")}
          ${range("Bottom dead zone", "kb-dz-bottom", 0, 1, 0.01, kb.deadZone[1], " mm")}
          ${toggle("Apply to the selected key only", "kb-only-sel", Boolean(state.kbSel))}
        </div>
      </div>
      <div class="actions">
        <button class="button primary" type="button" data-kb-magnet>Apply Switch Settings</button>
        <button class="button" type="button" data-kb-magnet-read>Read Current Values</button>
      </div>
      <div class="output" id="kb-output">Actuation range ${actMin}–${actMax} mm · rapid trigger ${rtMin}–${rtMax} mm.</div>
    </article>
    ${kbMacroCard()}
    ${kbAdvancedCard()}`;
}

/* ------------------------------------------------ keyboard advanced keys -- */

const ADV_KINDS = [["dks", "DKS · dynamic keystroke"], ["mt", "MT · tap / hold"], ["tgl", "TGL · toggle"], ["rs", "RS · rapid snap"], ["socd", "SOCD · opposite keys"], ["oks", "OKS · one key, two keys"], ["aks", "AKS · alternate keys"]];
const SOCD_MODES = [[2, "Last input wins"], [3, "Key 1 wins"], [4, "Key 2 wins"], [5, "Neutral (both cancel)"]];
const AKS_MODES = [[9, "Balanced"], [7, "Key 1 first"], [8, "Key 2 first"]];
const DKS_STAGES = ["Press", "Bottom", "Lift", "Release"];
const ADV_MODE_KIND = { 1: "rs", 2: "socd", 3: "socd", 4: "socd", 5: "socd", 6: "oks", 7: "aks", 8: "aks", 9: "aks" };

function dksDecode(status) {
  const out = [-1, -1, -1, -1];
  const bit = (n) => (status >> n) & 1;
  let k = 0;
  while (k < 4) {
    if (!bit(3 * k)) { k += 1; continue; }
    if (k === 3 || !bit(3 * k + 1)) { out[k] = 0; k += 1; continue; }
    let j = k + 1;
    while (j < 3 && bit(3 * j - 1) && bit(3 * j) && bit(3 * j + 1)) j += 1;
    out[k] = j;
    k = j;
  }
  return out;
}

function dksEncode(stages) {
  let status = 0;
  let k = 0;
  while (k < 4) {
    const choice = stages[k];
    if (choice === 0) status |= 1 << (3 * k);
    if (choice > k) {
      for (let n = 3 * k; n <= 3 * choice - 2; n += 1) status |= 1 << n;
      k = choice;
      continue;
    }
    k += 1;
  }
  return status;
}

function advKeyOptions(value) {
  const groups = kbKeyGroups().slice(0, 3);
  return `<option value="0" ${value ? "" : "selected"}>None</option>${groups.map(([label, options]) => `<optgroup label="${escapeHtml(label)}">${options.filter(([v]) => v > 1).map(([v, name]) =>
    `<option value="${v}" ${v === value ? "selected" : ""}>${escapeHtml(name)}</option>`).join("")}</optgroup>`).join("")}`;
}

function advSlotUsed(kind, i) {
  const a = state.kbAdv;
  if (!a) return false;
  if (kind === "dks") return (a.dks[i]?.actions ?? []).some((x) => x.code && x.status);
  if (kind === "mt") return Boolean(a.tapDance[i]?.tap || a.tapDance[i]?.hold);
  if (kind === "tgl") return Boolean(a.tapDance[i + 20]?.tap);
  return ADV_MODE_KIND[a.advanced[i]?.mode] === kind;
}

function kbAdvancedCard() {
  const a = state.kbAdv;
  const kind = state.advKind ?? "dks";
  const slot = state.advSlot ?? 0;
  const sel = state.kbSel;
  const slotSelect = select("Slot", "adv-slot", Array.from({ length: 20 }, (_, i) => [i, `${i + 1}${advSlotUsed(kind, i) ? " ●" : ""}`]), slot);
  let body = `<div class="notice">Read the keyboard's advanced keys first.</div>`;
  if (a && kind === "dks") {
    const d = a.dks[slot] ?? { actions: [], top: 0.5, bottom: 3 };
    body = `${[0, 1, 2, 3].map((row) => {
      const act = d.actions[row] ?? { status: 0, code: 0 };
      const stages = dksDecode(act.status);
      let covered = -1;
      return `<div class="actions" style="align-items:flex-end">
        <label class="field"><span>Binding ${row + 1}</span><select id="dks-code-${row}">${advKeyOptions(act.code)}</select></label>
        ${DKS_STAGES.map((name, k) => {
          const disabled = k < covered;
          if (stages[k] > k) covered = stages[k];
          const opts = [[-1, "—"], [0, "Tap"], ...[1, 2, 3].filter((j) => j > k).map((j) => [j, `Hold → ${DKS_STAGES[j]}`])];
          return `<label class="field"><span>${name}</span><select id="dks-st-${row}-${k}" ${disabled ? "disabled" : ""}>${opts.map(([v, t]) => `<option value="${v}" ${v === (disabled ? -1 : stages[k]) ? "selected" : ""}>${t}</option>`).join("")}</select></label>`;
        }).join("")}
      </div>`;
    }).join("")}
    ${field("Upper point (mm)", "dks-top", String(d.top || 0.5), "number")}
    ${field("Lower point (mm)", "dks-bottom", String(d.bottom || 3), "number")}`;
  } else if (a && (kind === "mt" || kind === "tgl")) {
    const t = a.tapDance[kind === "mt" ? slot : slot + 20] ?? { tap: 0, hold: 0, delay: 200 };
    body = `<div class="form-row"><label for="td-tap">${kind === "mt" ? "Tap sends" : "Key to toggle"}</label><select id="td-tap">${advKeyOptions(t.tap)}</select></div>
      ${kind === "mt" ? `<div class="form-row"><label for="td-hold">Hold sends</label><select id="td-hold">${advKeyOptions(t.hold)}</select></div>` : ""}
      ${field("Hold threshold (ms)", "td-delay", String(t.delay || 200), "number")}`;
  } else if (a) {
    const k = a.advanced[slot] ?? {};
    const key1 = state.advKey1 ?? (ADV_MODE_KIND[k.mode] ? k.key1 : null);
    const key2 = state.advKey2 ?? (ADV_MODE_KIND[k.mode] ? k.key2 : null);
    const pos = (p) => (p ? `r${p[0]} c${p[1]}${state.kbMap?.[p[0]]?.[p[1]] ? ` · ${kbLabel(state.kbMap[p[0]][p[1]])}` : ""}` : "not set");
    body = `<div class="stat-row"><span class="stat-label">Key 1</span><span class="stat-value">${escapeHtml(pos(key1))}</span></div>
      <div class="stat-row"><span class="stat-label">Key 2</span><span class="stat-value">${escapeHtml(pos(key2))}</span></div>
      <div class="actions">
        <button class="button sm" type="button" data-adv-key="1" ${sel ? "" : "disabled"}>Selected → Key 1</button>
        <button class="button sm" type="button" data-adv-key="2" ${sel ? "" : "disabled"}>Selected → Key 2</button>
      </div>
      ${kind === "socd" ? select("Resolution", "adv-mode", SOCD_MODES, ADV_MODE_KIND[k.mode] === "socd" ? k.mode : 2) : ""}
      ${kind === "aks" ? select("Order", "adv-mode", AKS_MODES, ADV_MODE_KIND[k.mode] === "aks" ? k.mode : 9) : ""}
      ${field("Trigger point (mm)", "adv-point", String(k.point || 1.5), "number")}
      ${toggle("Rapid trigger", "adv-rt", k.rt)}
      ${kind === "aks" ? `${field("Trigger delay (ms)", "adv-tdelay", String(k.triggerDelay || 200), "number")}
      ${field("Key delay (ms)", "adv-kdelay", String(k.keyDelay || 200), "number")}
      ${field("Randomness (%)", "adv-random", String(k.random ?? 50), "number")}` : ""}`;
  }
  const binds = kind === "dks" || kind === "mt" || kind === "tgl";
  return `
    <article class="card">
      <div class="card-title"><h2>Advanced keys</h2><span>${binds ? (sel ? `binds key r${sel[0]} c${sel[1]}` : "select a key on the layout") : "pairs two keys"}</span></div>
      ${select("Type", "adv-kind", ADV_KINDS, kind)}
      ${slotSelect}
      ${body}
      <div class="output" id="adv-output"></div>
      <div class="actions">
        <button class="button" type="button" data-adv-read>Read Advanced Keys</button>
        <button class="button primary" type="button" data-adv-save ${a && (!binds || sel) ? "" : "disabled"}>Save${binds ? " & Bind Key" : ""}</button>
        <button class="button" type="button" data-adv-clear ${a ? "" : "disabled"}>Clear Slot</button>
      </div>
    </article>`;
}

async function kbAdvancedSave(clear) {
  const kind = state.advKind ?? "dks";
  const index = num("adv-slot", 0);
  const a = state.kbAdv;
  if (kind === "dks") {
    const slot = clear ? { actions: [0, 1, 2, 3].map(() => ({ status: 0, code: 0 })), top: 0, bottom: 0 } : {
      actions: [0, 1, 2, 3].map((row) => ({ code: num(`dks-code-${row}`, 0), status: dksEncode([0, 1, 2, 3].map((k) => ($(`dks-st-${row}-${k}`)?.disabled ? -1 : num(`dks-st-${row}-${k}`, -1)))) })),
      top: num("dks-top", 0.5), bottom: num("dks-bottom", 3),
    };
    await call("kb_set_dks", { index, slot }, clear ? `DKS ${index + 1} cleared` : `DKS ${index + 1} saved`);
    a.dks[index] = slot;
  } else if (kind === "mt" || kind === "tgl") {
    const slot = clear ? { tap: 0, hold: 0, delay: 0 } : { tap: num("td-tap", 0), hold: kind === "mt" ? num("td-hold", 0) : 0, delay: Math.max(0, Math.min(65535, num("td-delay", 200))) };
    const i = kind === "mt" ? index : index + 20;
    await call("kb_set_tap_dance", { index: i, slot }, clear ? `${kind.toUpperCase()} ${index + 1} cleared` : `${kind.toUpperCase()} ${index + 1} saved`);
    a.tapDance[i] = slot;
  } else {
    const prev = a.advanced[index] ?? {};
    const key1 = state.advKey1 ?? prev.key1;
    const key2 = state.advKey2 ?? prev.key2;
    if (!clear && (!key1 || (kind !== "rs" && !key2) )) throw new Error("Choose the keys first: select a key on the layout, then press Selected → Key 1 / Key 2");
    const mode = clear ? 0 : kind === "rs" ? 1 : kind === "oks" ? 6 : num("adv-mode", kind === "socd" ? 2 : 9);
    const key = clear ? { mode: 0, point: 0, withGlobal: false, rt: false, key1: [0, 0], key2: [0, 0], layer: 0, triggerDelay: 0, keyDelay: 0, random: 0, triggerSwitch: 0 } : {
      mode, point: num("adv-point", 1.5), withGlobal: false, rt: checked("adv-rt"), key1, key2: key2 ?? key1, layer: state.kbLayer ?? 0,
      triggerDelay: num("adv-tdelay", 200), keyDelay: num("adv-kdelay", 200), random: Math.max(0, Math.min(100, num("adv-random", 50))), triggerSwitch: 0,
    };
    await call("kb_set_advanced_key", { index, key }, clear ? `Slot ${index + 1} cleared` : `${kind.toUpperCase()} slot ${index + 1} saved`);
    a.advanced[index] = key;
    state.advKey1 = null;
    state.advKey2 = null;
  }
  const bases = { dks: 0x7e05, mt: 0x5700, tgl: 0x5714 };
  if (!clear && bases[kind] !== undefined && state.kbSel) {
    const [row, col] = state.kbSel;
    const code = bases[kind] + index;
    await call("kb_set_key", { layer: state.kbLayer ?? 0, row, col, code }, `Key r${row} c${col} bound to ${kind.toUpperCase()} ${index + 1}`);
    state.kbMap[row][col] = code;
  }
  renderPage();
}

/* ------------------------------------------------ keyboard onboard macros -- */

const KB_MACRO_MODES = [[1, "Run N times, stop on release"], [2, "Run N times, keep going"], [3, "Run N times then stop"], [4, "Finish the round on release"], [5, "Loop until pressed again"], [6, "Loop with no stop condition"]];
const KB_MOD_CODES = { 16: 225, 17: 224, 18: 226, 91: 227, 92: 231, 160: 225, 161: 229, 162: 224, 163: 228, 164: 226, 165: 230 };
const KB_NAMED_MOD = { Shift: 225, Control: 224, Ctrl: 224, Alt: 226, Meta: 227, Win: 227 };
const KB_MOUSE_CODES = [[209, "Left click"], [210, "Right click"], [211, "Middle click"], [212, "Back"], [213, "Forward"]];

function kbStepsToActions(steps) {
  const actions = [];
  for (const step of steps ?? []) {
    const code = step.vk !== undefined ? (KB_MOD_CODES[step.vk] ?? usageForVk(step.vk)) : (KB_NAMED_MOD[step.key] ?? usageForName(step.key));
    if (!code) throw new Error(`Key ${step.key} cannot be stored on the keyboard`);
    const delayMs = Math.min(65535, Math.max(0, Number(step.delay_ms) || 0));
    if (step.action) actions.push({ down: step.action === "down", code, delayMs });
    else actions.push({ down: true, code, delayMs: 20 }, { down: false, code, delayMs });
  }
  return actions;
}

function kbActionName(code) {
  return KB_MOUSE_CODES.find(([c]) => c === code)?.[1] ?? HID[code]?.[0] ?? MOD_KEYS[code] ?? `#${code}`;
}

function kbMacroCard() {
  const slots = state.kbMacros;
  const slot = state.kbMacroSlot ?? 0;
  const current = slots?.[slot];
  const recorded = (state.macros ?? []).map((m, i) => [i, m.name]);
  const summary = current?.actions?.length
    ? current.actions.map((a) => `${a.down ? "↓" : "↑"}${kbActionName(a.code)}${a.delayMs ? ` ${a.delayMs}ms` : ""}`).join("  ")
    : "Empty";
  return `
    <article class="card">
      <div class="card-title"><h2>Onboard macros</h2><span>${slots ? "stored on the keyboard" : "not read yet"}</span></div>
      ${select("Macro slot", "kbm-slot", Array.from({ length: 8 }, (_, i) => [i, `M${i}${slots?.[i]?.actions?.length ? " ●" : ""}`]), slot)}
      ${recorded.length ? select("Recorded macro", "kbm-source", recorded, 0) : `<div class="notice">Record a macro on the Macros page to store it here.</div>`}
      ${select("Playback", "kbm-mode", KB_MACRO_MODES, current?.mode ?? 1)}
      ${field("Times", "kbm-count", String(current?.count ?? 1), "number")}
      <div class="output" id="kbm-output">${escapeHtml(slots ? `M${slot}: ${summary}` : "Read the keyboard's macros first.")}</div>
      <div class="actions">
        <button class="button" type="button" data-kbm-read>Read Macros</button>
        <button class="button primary" type="button" data-kbm-write ${recorded.length && slots ? "" : "disabled"}>Store in Slot</button>
        <button class="button" type="button" data-kbm-clear ${slots ? "" : "disabled"}>Clear Slot</button>
      </div>
      <div class="notice" style="margin-top:10px">Assign a slot to a key with the Macros group (M0–M7) in the key remap list.</div>
    </article>`;
}

async function kbMacroWrite(clear) {
  const slots = structuredClone(state.kbMacros ?? []);
  while (slots.length < 8) slots.push({ actions: [], mode: 1, count: 1 });
  const slot = num("kbm-slot", 0);
  if (clear) {
    slots[slot] = { actions: [], mode: 1, count: 1 };
  } else {
    const macro = state.macros?.[num("kbm-source", 0)];
    if (!macro) throw new Error("Record a macro on the Macros page first");
    slots[slot] = { actions: kbStepsToActions(macro.steps), mode: num("kbm-mode", 1), count: Math.max(1, Math.min(255, num("kbm-count", 1))) };
  }
  await call("kb_set_macros", { macros: slots }, clear ? `M${slot} cleared` : `Macro stored in M${slot}`);
  state.kbMacros = slots;
  state.kbMacroSlot = slot;
  renderPage();
}

async function readKeyboard() {
  try {
    state.kb = await invoke("kb_snapshot");
    state.kbError = null;
    state.kbLayer = Math.min(state.kbLayer ?? 0, state.kb.model.layers - 1);
    state.kbMap = await invoke("kb_keymap", { layer: state.kbLayer });
    setStatus(`${state.kb.model.name} connected`);
  } catch (error) {
    state.kb = null;
    state.kbMap = null;
    state.kbError = errorText(error);
  }
  if (state.current === "keyboard") { renderPage(); await loadKeymap(state.keymapName ?? state.keymaps?.[0]); }
}

async function kbApply(patch, message) {
  state.kb = await call("kb_apply", { patch }, message);
  renderPage();
  await loadKeymap(state.keymapName ?? state.keymaps?.[0]);
}

function kbIndex() {
  const [r, c] = state.kbSel;
  return r * state.kb.model.cols + c;
}

async function kbMagnet() {
  const only = $("kb-only-sel")?.checked && state.kbSel;
  const count = state.kb.model.rows * state.kb.model.cols;
  const point = num("kb-act", 1.2);
  const rt = { on: $("kb-rt").checked, press: num("kb-rt-press", 0.1), release: num("kb-rt-release", 0.1) };
  Object.assign(state, { kbAct: point, kbRtOn: rt.on, kbRtPress: rt.press, kbRtRelease: rt.release });
  let points = Array(count).fill(point);
  let keys = Array(count).fill(rt);
  if (only) {
    points = await invoke("kb_actuation");
    keys = await invoke("kb_rapid_trigger");
    points[kbIndex()] = point;
    keys[kbIndex()] = rt;
  }
  show("kb-output", "Writing actuation…");
  await invoke("kb_set_actuation", { points });
  show("kb-output", "Writing rapid trigger…");
  await invoke("kb_set_rapid_trigger", { keys });
  await call("kb_apply", { patch: { deadZone: [num("kb-dz-top", 0), num("kb-dz-bottom", 0)] } }, "Switch settings saved");
  show("kb-output", `${only ? "Selected key" : "All keys"}: actuation ${point} mm · rapid trigger ${rt.on ? `on (${rt.press}/${rt.release} mm)` : "off"}`);
}

async function kbMagnetRead() {
  const points = await invoke("kb_actuation");
  const keys = await invoke("kb_rapid_trigger");
  const i = state.kbSel ? kbIndex() : 0;
  const values = points.filter((v) => v > 0);
  show("kb-output", state.kbSel
    ? `Key r${state.kbSel[0]} c${state.kbSel[1]}: actuation ${points[i]} mm · rapid trigger ${keys[i]?.on ? `on ${keys[i].press}/${keys[i].release} mm` : "off"}`
    : `Actuation ${Math.min(...values)}–${Math.max(...values)} mm across ${values.length} keys · rapid trigger on ${keys.filter((k) => k.on).length} keys`);
  if (keys[i]) Object.assign(state, { kbAct: points[i], kbRtOn: keys[i].on, kbRtPress: keys[i].press || state.kbRtPress, kbRtRelease: keys[i].release || state.kbRtRelease });
}

export async function kbHandleClick(t) {
  const d = t.dataset;
  if (t.classList.contains("kb-key") && d.r !== undefined) {
    if (!state.kb) return true;
    const sel = [Number(d.r), Number(d.c)];
    state.kbSel = state.kbSel && state.kbSel[0] === sel[0] && state.kbSel[1] === sel[1] ? null : sel;
    renderPage();
    await loadKeymap(state.keymapName ?? state.keymaps?.[0]);
    return true;
  }
  if ("kbRead" in d) { await guarded(t, readKeyboard); return true; }
  if (d.kbProfile !== undefined) {
    await guarded(t, async () => {
      await kbApply({ profile: Number(d.kbProfile) }, "Profile switched");
      state.kbMap = await invoke("kb_keymap", { layer: state.kbLayer ?? 0 });
      renderKeyboard();
    });
    return true;
  }
  if ("kbLight" in d) {
    const color = $("kb-color").value.match(/[0-9a-f]{2}/gi).map((v) => parseInt(v, 16));
    await guarded(t, () => kbApply({
      lighting: { mode: num("kb-effect", 0), brightness: num("kb-bright", 100), speed: num("kb-speed", 128), color, colorful: $("kb-colorful").checked ? 1 : 0 },
      sleepSecs: num("kb-sleep", 0),
    }, "Lighting applied"));
    return true;
  }
  if ("kbFunctions" in d) {
    const f = state.kb.features;
    await guarded(t, () => kbApply({
      reportRate: num("kb-rate", 1000),
      gameMode: $("kb-game").checked,
      features: { ...f, wasdSwap: $("kb-wasd").checked ? 1 : 0, winLock: $("kb-winlock").checked ? 1 : 0, mac: $("kb-mac").checked ? 1 : 0 },
    }, "Functions applied"));
    return true;
  }
  if ("kbAssign" in d && state.kbSel) {
    await guarded(t, async () => {
      let code = num("kb-code", 0);
      const mod = num("kb-mod", 0);
      if (mod && code >= 4 && code <= 0xff) code |= mod << 8;
      const [row, col] = state.kbSel;
      await call("kb_set_key", { layer: state.kbLayer ?? 0, row, col, code }, `Key set to ${kbLabel(code)}`);
      state.kbMap[row][col] = code;
      renderKeyboard();
    });
    return true;
  }
  if ("kbResetMap" in d) {
    if (!state.confirmKbReset) { state.confirmKbReset = true; t.textContent = "Click again to confirm"; return true; }
    state.confirmKbReset = false;
    await guarded(t, async () => {
      await call("kb_reset_keymap", {}, "Default keymap restored");
      state.kbMap = await invoke("kb_keymap", { layer: state.kbLayer ?? 0 });
      renderPage();
      await loadKeymap(state.keymapName ?? state.keymaps?.[0]);
    });
    return true;
  }
  if ("kbMagnet" in d) { await guarded(t, kbMagnet); return true; }
  if ("kbmRead" in d) {
    await guarded(t, async () => {
      if (!state.macros?.length) state.macros = (await invoke("macros_load").catch(() => [])) ?? [];
      state.kbMacros = await call("kb_macros", {}, "Keyboard macros read");
      renderPage();
    });
    return true;
  }
  if ("advRead" in d) { await guarded(t, async () => { state.kbAdv = await call("kb_advanced_keys", {}, "Advanced keys read"); renderPage(); }); return true; }
  if ("advSave" in d) { await guarded(t, () => kbAdvancedSave(false)); return true; }
  if ("advClear" in d) { await guarded(t, () => kbAdvancedSave(true)); return true; }
  if ("advKey" in d && state.kbSel) { state[d.advKey === "1" ? "advKey1" : "advKey2"] = [...state.kbSel]; renderPage(); return true; }
  if ("kbmWrite" in d) { await guarded(t, () => kbMacroWrite(false)); return true; }
  if ("kbmClear" in d) { await guarded(t, () => kbMacroWrite(true)); return true; }
  if ("kbMagnetRead" in d) { await guarded(t, kbMagnetRead); return true; }
  return false;
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
      <div class="list-item"><span><strong>${escapeHtml(rule.exe)}</strong> → Mouse profile ${rule.profile + 1}${rule.keyboardProfile !== undefined && rule.keyboardProfile !== null ? ` · Keyboard profile ${rule.keyboardProfile + 1}` : ""}</span>
      <button class="button sm" type="button" data-game-delete="${i}">Remove</button></div>`).join("");
  return `
    ${pageHeading(state.pages.find((page) => page.id === "games"))}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Auto switch</h2><span>foreground app</span></div>
        ${toggle("Switch mouse and keyboard profiles when a game is focused", "game-enabled", state.hubSettings?.gameSwitchEnabled)}
        <div class="list" id="game-rules" style="margin-top:12px">${rows}</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Add game</h2><span>executable</span></div>
        ${field("Executable", "game-exe", "")}
        ${select("Mouse profile", "game-profile", [[0, "Profile 1"], [1, "Profile 2"], [2, "Profile 3"], [3, "Profile 4"]], 1)}
        ${select("Keyboard profile", "game-kb-profile", [["", "Don't change"], [0, "Profile 1"], [1, "Profile 2"], [2, "Profile 3"], [3, "Profile 4"]], "")}
        <div class="actions">
          <button class="button" type="button" data-game-capture>Pick Focused App (3 s)</button>
          <button class="button primary" type="button" data-game-add>Add Game</button>
        </div>
        <div class="notice" style="margin-top:14px">Leaving a configured game switches the mouse (and keyboard, when set) back to Profile 1.</div>
      </article>
    </div>`;
}

/* ------------------------------------------------------------ pc script -- */

const PC_SAMPLE = `-- Runs about every 16 ms while enabled.
function on_tick()
  if pc_rmb_pressed() and pc_lmb_pressed() then
    input.move_rel(0, 3)
  end
end`;

const PC_API = [
  ["on_tick()", "Your function, called about every 16 ms"],
  ["pc_lmb_pressed() / pc_rmb_pressed() / pc_mmb_pressed()", "Mouse buttons held"],
  ["pc_xbutton1_pressed() / pc_xbutton2_pressed()", "Side buttons held"],
  ["pc_key_pressed(VK.A)", "Any virtual key held"],
  ["pc_digit_or_numpad_pressed(3)", "Top-row or numpad digit held"],
  ["move_rel(dx, dy) / input.move_rel(dx, dy)", "Move the pointer relatively"],
  ["move_abs(x, y)", "Move the pointer to screen coordinates"],
  ["get_cursor_pos_native()", "Returns x, y"],
  ["mouse_click(b) / mouse_down(b) / mouse_up(b)", "1 left, 2 right, 3 middle, 4/5 side"],
  ["send_key_tap(VK.A) / send_key_down / send_key_up", "Virtual key presses"],
  ["chord_tap(VK.A, VK.SHIFT, VK.MENU)", "Key combination"],
  ["send_text(\"hello\") / send_unicode_utf16(32)", "Type text"],
  ["input.log(...)", "Write to the console below"],
];

function pcActive() {
  const config = state.pc?.config;
  return config?.profiles?.find((p) => p.id === state.pcProfile) ?? config?.profiles?.[0];
}

export function pcScriptPage() {
  const pc = state.pc;
  const head = pageHeading(state.pages.find((page) => page.id === "pcscript"));
  if (!pc) return `${head}<div class="notice">Loading PC scripts…</div>`;
  const config = pc.config;
  const active = pcActive();
  const exes = (config.profileForegroundExes?.[active?.id] ?? []).join("\n");
  const tickError = pc.profileTickErrors?.[active?.id];
  const profiles = config.profiles.map((p) => `
      <div class="list-item ${p.id === active?.id ? "selected" : ""}">
        <button class="button sm ${p.id === active?.id ? "primary" : ""}" type="button" data-pc-pick="${escapeHtml(p.id)}">${escapeHtml(p.name)}</button>
        <span>${p.enabled ? "Enabled" : "Off"}</span>
      </div>`).join("");
  return `
    ${head}
    ${pc.platformOk ? "" : `<div class="notice">PC scripts only run on Windows.</div>`}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Script profiles</h2><span>${pc.running ? "Running" : "Stopped"}</span></div>
        <div class="list">${profiles}</div>
        ${toggle("Resume enabled scripts when the Hub starts", "pc-resume", config.resumeOnStartup)}
        <div class="actions">
          <button class="button" type="button" data-pc-add>New Profile</button>
          <button class="button" type="button" data-pc-delete ${config.profiles.length > 1 ? "" : "disabled"}>Delete Profile</button>
          <button class="button" type="button" data-pc-stop ${pc.running ? "" : "disabled"}>Stop All</button>
        </div>
        <details class="raw-details" style="margin-top:12px">
          <summary>API reference</summary>
          <div class="list">${PC_API.map(([sig, what]) => `<div class="list-item"><code>${escapeHtml(sig)}</code><span>${escapeHtml(what)}</span></div>`).join("")}</div>
        </details>
      </article>
      <article class="card">
        <div class="card-title"><h2>${escapeHtml(active?.name ?? "Script")}</h2><span>Lua</span></div>
        ${field("Name", "pc-name", active?.name ?? "")}
        ${toggle("Enable this script", "pc-enabled", active?.enabled)}
        <div class="form-row"><label for="pc-script">Script</label><textarea id="pc-script" rows="14" spellcheck="false" style="font-family:Consolas,monospace">${escapeHtml(active?.luaScript || "")}</textarea></div>
        <div class="form-row"><label for="pc-exes">Only run in these programs (one .exe per line, empty for everywhere)</label><textarea id="pc-exes" rows="3" spellcheck="false">${escapeHtml(exes)}</textarea></div>
        <div class="form-row"><label for="pc-import">Import .lua file</label><input type="file" id="pc-import" accept=".lua,text/plain" /></div>
        <div class="actions">
          <button class="button primary" type="button" data-pc-save>Save &amp; Apply</button>
          <button class="button" type="button" data-pc-validate>Check Syntax</button>
          <button class="button" type="button" data-pc-sample>Insert Example</button>
          <button class="button" type="button" data-pc-export>Export .lua</button>
          <button class="button" type="button" data-pc-capture>Add Focused App (3 s)</button>
        </div>
        ${tickError ? `<div class="notice" style="white-space:pre-wrap">${escapeHtml(tickError)}</div>` : ""}
        <div class="output" id="pc-output">${escapeHtml(state.pcMessage ?? "Save & Apply validates the script and starts it.")}</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Console</h2><span>input.log output</span></div>
        <div class="output" id="pc-console" style="max-height:260px;overflow:auto;white-space:pre-wrap">${escapeHtml((state.pcLogs ?? []).map((l) => `${new Date(l.ts).toLocaleTimeString()}  ${l.message}`).join("\n") || "No output yet.")}</div>
        <div class="actions">
          <button class="button" type="button" data-pc-logs>Refresh</button>
          <button class="button" type="button" data-pc-clear>Clear</button>
        </div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Diagnostics</h2><span>input check</span></div>
        <div class="output" id="pc-probe">Hold keys, then press Probe Keys.</div>
        <div class="actions">
          <button class="button" type="button" data-pc-probe>Probe Keys</button>
          <button class="button" type="button" data-pc-move>Test Move (0, 10)</button>
        </div>
      </article>
    </div>`;
}

function pcCollect() {
  const config = structuredClone(state.pc.config);
  const active = config.profiles.find((p) => p.id === pcActive()?.id);
  if (active && $("pc-script")) {
    active.name = $("pc-name").value.trim() || active.name;
    active.enabled = checked("pc-enabled");
    active.luaScript = $("pc-script").value;
    const exes = $("pc-exes").value.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);
    config.profileForegroundExes = { ...(config.profileForegroundExes ?? {}) };
    if (exes.length) config.profileForegroundExes[active.id] = exes; else delete config.profileForegroundExes[active.id];
  }
  config.resumeOnStartup = checked("pc-resume");
  config.activeProfileId = active?.id ?? null;
  return config;
}

export async function loadPcScript() {
  try {
    state.pc = await invoke("pc_script_get_state");
    state.pcProfile ??= state.pc.config.activeProfileId ?? state.pc.config.profiles[0]?.id;
    state.pcLogs = (await invoke("pc_script_logs"))?.logs ?? [];
  } catch (error) {
    setStatus(errorText(error));
  }
  if (state.current === "pcscript") renderPage();
}

async function pcClick(t) {
  const d = t.dataset;
  if (d.pcPick !== undefined) { state.pc.config = pcCollect(); state.pcProfile = d.pcPick; state.pcMessage = null; renderPage(); return true; }
  if ("pcAdd" in d) {
    const config = pcCollect();
    const id = `${Date.now().toString(16)}-${Math.random().toString(16).slice(2, 8)}`;
    config.profiles.push({ id, name: `Script ${config.profiles.length + 1}`, luaScript: "", enabled: false });
    state.pc.config = config; state.pcProfile = id; renderPage(); return true;
  }
  if ("pcDelete" in d) {
    if (t.dataset.confirm !== "1") { t.dataset.confirm = "1"; t.textContent = "Click again to delete"; return true; }
    const config = pcCollect();
    config.profiles = config.profiles.filter((p) => p.id !== pcActive()?.id);
    state.pc.config = config; state.pcProfile = config.profiles[0]?.id; renderPage(); return true;
  }
  if ("pcSave" in d) {
    await guarded(t, async () => {
      const result = await invoke("pc_script_save_and_apply", { config: pcCollect() });
      state.pcMessage = result.ok ? ((result.warnings ?? []).join("\n") || "Saved. Enabled scripts are running.") : `Failed: ${result.error}`;
      setStatus(result.ok ? "PC scripts applied" : "PC scripts failed");
      await loadPcScript();
    });
    return true;
  }
  if ("pcValidate" in d) {
    const result = await invoke("pc_script_validate", { luaScript: $("pc-script").value });
    show("pc-output", result.ok ? "No syntax errors." : result.error);
    return true;
  }
  if ("pcSample" in d) { $("pc-script").value = PC_SAMPLE; return true; }
  if ("pcExport" in d) {
    await guarded(t, async () => { show("pc-output", `Saved to ${await invoke("pc_script_export", { name: $("pc-name").value, content: $("pc-script").value })}`); });
    return true;
  }
  if ("pcCapture" in d) {
    await guarded(t, async () => {
      show("pc-output", "Switch to the program now…");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const exe = await invoke("foreground_process");
      if (!exe) throw new Error("Could not read the focused program");
      const name = exe.split(/[\\/]/).pop();
      const box = $("pc-exes");
      box.value = [...box.value.split(/\r?\n/).filter(Boolean), name].join("\n");
      show("pc-output", `Added ${name}`);
    });
    return true;
  }
  if ("pcStop" in d) { await invoke("pc_script_stop"); setStatus("PC scripts stopped"); await loadPcScript(); return true; }
  if ("pcLogs" in d) { await loadPcScript(); return true; }
  if ("pcClear" in d) { await invoke("pc_script_clear_logs"); state.pcLogs = []; show("pc-console", "No output yet."); return true; }
  if ("pcProbe" in d) { show("pc-probe", await invoke("pc_script_probe_keys")); return true; }
  if ("pcMove" in d) { await invoke("pc_script_debug_move", { dx: 0, dy: 10 }); return true; }
  return false;
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

/* ------------------------------------------------ wooden fish & pet -- */

const PET_ACTIONS = [["idle", "Idle"], ["waving", "Waving"], ["jumping", "Jumping"], ["running", "Running"], ["running-left", "Running left"], ["running-right", "Running right"], ["waiting", "Waiting"], ["review", "Review"], ["failed", "Failed"]];

function meritStats(days, period) {
  const entries = Object.entries(days ?? {});
  const today = new Date();
  const key = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const start = new Date(today);
  if (period === "week") start.setDate(today.getDate() - ((today.getDay() + 6) % 7));
  if (period === "month") start.setDate(1);
  if (period === "year") start.setMonth(0, 1);
  const from = period === "all" ? "" : key(start);
  const picked = entries.filter(([day]) => day >= from);
  const total = picked.reduce((sum, [, count]) => sum + (Number(count) || 0), 0);
  const best = picked.reduce((top, entry) => (Number(entry[1]) > Number(top?.[1] ?? -1) ? entry : top), null);
  return { total, days: picked.length, best };
}

export function companionCards() {
  const o = state.overlay;
  if (!o) return `<div class="notice">Loading wooden fish and desktop pet…</div>`;
  const fish = o.fish ?? {};
  const pet = o.pet ?? {};
  const period = state.meritPeriod ?? "day";
  const stats = meritStats(fish.days, period);
  return `
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Wooden fish</h2><span>counts every key press</span></div>
        ${toggle("Wooden fish enabled", "pet-enabled", fish.enabled)}
        ${toggle("Show floating wooden fish", "pet-visible", fish.petVisible)}
        ${toggle("Knock sound", "pet-sound", fish.sound)}
        ${select("Skin", "fish-skin", [["default", "Classic"], ["white", "White"], ["orange", "Orange"]], fish.skin || "default")}
        ${select("Merit period", "merit-period", [["day", "Today"], ["week", "This week"], ["month", "This month"], ["year", "This year"], ["all", "All time"]], period)}
        <div class="stat-row"><span class="stat-label">Merit</span><span class="stat-value">${stats.total}</span></div>
        <div class="stat-row"><span class="stat-label">Active days</span><span class="stat-value">${stats.days}</span></div>
        <div class="stat-row"><span class="stat-label">Best day</span><span class="stat-value">${stats.best ? `${escapeHtml(stats.best[0])} · ${stats.best[1]}` : "—"}</span></div>
        <div class="form-row"><label for="fish-sound-file">Custom knock sound ${o.customSound ? "<b>(in use)</b>" : ""}</label><input type="file" id="fish-sound-file" accept="audio/*" /></div>
        <div class="actions">
          <button class="button primary" type="button" data-pet-save>Save</button>
          <button class="button" type="button" data-fish-tap>Knock</button>
          <button class="button" type="button" data-fish-sound-clear ${o.customSound ? "" : "disabled"}>Default Sound</button>
        </div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Desktop pet</h2><span>Lv ${o.level ?? 1} · ${pet.xp ?? 0} XP</span></div>
        ${toggle("Show desktop pet", "dp-visible", pet.visible)}
        ${select("Size", "dp-scale", [50, 75, 100, 125, 150].map((n) => [n, `${n}%`]), pet.scale ?? 100)}
        ${field("Name", "dp-name", pet.petName ?? "Aby")}
        ${toggle("Always on top", "dp-top", pet.alwaysOnTop)}
        ${toggle("Click-through", "dp-click", pet.clickThrough)}
        ${toggle("Idle animations", "dp-auto", pet.autoPlay)}
        ${toggle("React to keyboard and mouse", "dp-follow", pet.followInput)}
        ${toggle("Show key hints", "dp-hints", pet.showKeyHints)}
        ${toggle("Show name and level", "dp-level", pet.showLevel)}
        ${select("Preview action", "dp-action", PET_ACTIONS, "waving")}
        <div class="form-row"><label for="dp-sprite-file">Custom sprite sheet (PNG/WebP, 8×9 frames, ≤4 MB) ${o.customSprite ? "<b>(in use)</b>" : ""}</label><input type="file" id="dp-sprite-file" accept="image/png,image/webp" /></div>
        <div class="actions">
          <button class="button primary" type="button" data-dp-save>Save</button>
          <button class="button" type="button" data-dp-play>Play Action</button>
          <button class="button" type="button" data-dp-reset>Reset Position</button>
          <button class="button" type="button" data-dp-sprite-clear ${o.customSprite ? "" : "disabled"}>Default Sprite</button>
        </div>
      </article>
    </div>`;
}

function readFileUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export async function loadCompanions() {
  try { state.overlay = await invoke("overlay_state"); } catch (error) { setStatus(errorText(error)); }
  renderCompanions();
}

function renderCompanions() {
  const box = $("companions");
  if (box) box.innerHTML = companionCards();
}

function readPetPrefs() {
  return {
    ...(state.overlay?.pet ?? {}),
    visible: checked("dp-visible"), scale: num("dp-scale", 100), petName: $("dp-name")?.value ?? "Aby",
    alwaysOnTop: checked("dp-top"), clickThrough: checked("dp-click"), autoPlay: checked("dp-auto"),
    followInput: checked("dp-follow"), showKeyHints: checked("dp-hints"), showLevel: checked("dp-level"),
  };
}

async function companionClick(t) {
  const d = t.dataset;
  if ("petSave" in d) {
    await guarded(t, async () => {
      const settings = { ...(state.petState ?? {}), enabled: checked("pet-enabled"), petVisible: checked("pet-visible"), sound: checked("pet-sound"), skin: $("fish-skin")?.value ?? state.petState?.skin ?? "" };
      state.petState = await call("pets_save", { path: state.petState?.path ?? state.info?.petPath ?? "", settings }, "Wooden fish settings saved");
      show("output", state.petState);
      await loadCompanions();
    });
    return true;
  }
  if ("fishTap" in d) { await guarded(t, async () => { await invoke("woodenfish_tap"); await loadCompanions(); }); return true; }
  if ("fishSoundClear" in d) { await guarded(t, async () => { state.overlay = await call("woodenfish_sound_set", { data: null }, "Default knock sound restored"); renderCompanions(); }); return true; }
  if ("dpSave" in d) { await guarded(t, async () => { state.overlay = await call("desktop_pet_set", { prefs: readPetPrefs() }, "Desktop pet saved"); renderCompanions(); }); return true; }
  if ("dpPlay" in d) { await window.__TAURI__?.event?.emit("pet:play", $("dp-action")?.value ?? "waving"); return true; }
  if ("dpReset" in d) { await guarded(t, () => call("desktop_pet_reset_position", {}, "Overlay positions reset")); return true; }
  if ("dpSpriteClear" in d) { await guarded(t, async () => { state.overlay = await call("desktop_pet_upload", { data: null }, "Default sprite restored"); renderCompanions(); }); return true; }
  return false;
}

async function companionFile(input) {
  const file = input.files?.[0];
  if (!file) return;
  try {
    if (input.id === "fish-sound-file") {
      if (file.size > 4 * 1024 * 1024) throw new Error("The sound must be 4 MB or smaller");
      state.overlay = await call("woodenfish_sound_set", { data: await readFileUrl(file) }, "Custom knock sound saved");
    } else {
      if (file.size > 4 * 1024 * 1024) throw new Error("The sprite must be 4 MB or smaller");
      state.overlay = await call("desktop_pet_upload", { data: await readFileUrl(file) }, "Custom sprite saved");
    }
    renderCompanions();
  } catch (error) {
    setStatus(errorText(error));
  }
}

/* --------------------------------------------------------------- headset -- */

const BOLUTEK_PEQ_TYPES = [[0, "Off"], [1, "Peaking"], [2, "High-pass"], [3, "Low-pass"], [4, "Low shelf"], [5, "High shelf"]];
const KT_PEQ_TYPES = [[0, "Peak"], [1, "Low-pass"], [2, "High-pass"], [3, "Low shelf"], [4, "High shelf"]];
const MIC_GAIN = [[0, "0 dB"], [1, "6 dB"], [2, "12 dB"], [3, "18 dB"], [4, "24 dB"], [5, "30 dB"]];

function peqRows(prefix, points, types) {
  return `<div class="table-wrap"><table class="peq"><thead><tr><th>#</th><th>On</th><th>Type</th><th>Freq (Hz)</th><th>Gain (dB)</th><th>Q</th></tr></thead><tbody>${(points ?? []).map((p, i) => `
    <tr>
      <td>${i + 1}</td>
      <td><input type="checkbox" id="${prefix}-en-${i}" ${p.enabled ? "checked" : ""} /></td>
      <td><select id="${prefix}-type-${i}">${types.map(([v, t]) => `<option value="${v}" ${v === p.kind ? "selected" : ""}>${t}</option>`).join("")}</select></td>
      <td><input type="number" id="${prefix}-freq-${i}" min="20" max="20000" value="${p.freq}" /></td>
      <td><input type="number" id="${prefix}-gain-${i}" min="-12" max="12" step="0.5" value="${p.gain}" /></td>
      <td><input type="number" id="${prefix}-q-${i}" min="0.1" max="10" step="0.1" value="${Number(p.q).toFixed(2)}" /></td>
    </tr>`).join("")}</tbody></table></div>`;
}

function readPeq(prefix, count) {
  return Array.from({ length: count }, (_, i) => ({
    enabled: checked(`${prefix}-en-${i}`), kind: num(`${prefix}-type-${i}`, 0),
    freq: Math.round(Math.min(20000, Math.max(20, num(`${prefix}-freq-${i}`, 1000)))),
    gain: Math.min(12, Math.max(-12, num(`${prefix}-gain-${i}`, 0))), q: Math.max(0.1, num(`${prefix}-q-${i}`, 1)),
  }));
}

export function headsetCards() {
  const list = state.headsets;
  if (!list) return `<div class="notice">Looking for headsets…</div>`;
  if (!list.length) {
    return `<article class="card"><div class="card-title"><h2>Headset</h2><span>not connected</span></div>
      <div class="notice">Plug in the 2.4G dongle of an ATK N9 V2 or Horizon headset.</div>
      <div class="actions"><button class="button" type="button" data-hs-scan>Scan Again</button></div></article>`;
  }
  const device = list[state.headsetIndex ?? 0] ?? list[0];
  const picker = list.length > 1 ? select("Headset", "hs-device", list.map((d, i) => [i, d.name]), state.headsetIndex ?? 0) : "";
  const hs = state.headset;
  const actions = `<div class="actions">
      <button class="button" type="button" data-hs-read>Read</button>
      <button class="button primary" type="button" data-hs-apply ${hs ? "" : "disabled"}>Apply</button>
      <button class="button" type="button" data-hs-reset>Factory Reset</button>
    </div><div class="output" id="hs-output"></div>`;
  if (device.family === "ktmicro") {
    return `<div class="grid two">
      <article class="card">
        <div class="card-title"><h2>${escapeHtml(device.name)}</h2><span>KTMicro</span></div>
        ${picker}
        ${hs ? `${range("Output volume", "hs-volume", 0, 100, 1, hs.volume)}
        ${select("Microphone gain", "hs-mic", MIC_GAIN, hs.micGain)}
        ${toggle("Headphone EQ", "hs-dac-eq", hs.dacEq)}
        ${toggle("Microphone EQ", "hs-adc-eq", hs.adcEq)}` : `<div class="notice">Read the headset to load its settings.</div>`}
        ${actions}
      </article>
      ${hs ? `<article class="card"><div class="card-title"><h2>Headphone EQ</h2><span>6 bands</span></div>${peqRows("hs-dac", hs.dacPoints, KT_PEQ_TYPES)}</article>
      <article class="card"><div class="card-title"><h2>Microphone EQ</h2><span>6 bands</span></div>${peqRows("hs-adc", hs.adcPoints, KT_PEQ_TYPES)}</article>` : ""}
    </div>`;
  }
  return `<div class="grid two">
    <article class="card">
      <div class="card-title"><h2>${escapeHtml(device.name)}</h2><span>${hs ? `${hs.battery}% · ${hs.status}` : "Bolutek"}</span></div>
      ${picker}
      ${hs ? `${select("EQ preset", "hs-eq", [[0, "Game 1"], [1, "Game 2"], [2, "Custom"]], hs.eqMode)}
      ${toggle("Virtual 7.1 surround", "hs-surround", hs.surround)}
      ${toggle("Spatial sound", "hs-spatial", hs.spatial)}
      ${toggle("Sidetone", "hs-sidetone", hs.sidetone)}
      ${range("Sidetone volume", "hs-sidetone-vol", 0, 15, 1, hs.sidetoneVolume)}
      ${range("Voice prompt volume", "hs-prompt", 0, 15, 1, hs.promptVolume)}
      ${select("Auto power off", "hs-apo", [[0, "Never"], [1, "10 minutes"], [2, "20 minutes"], [3, "30 minutes"], [4, "60 minutes"]], hs.autoPowerOff)}
      ${select("Lighting", "hs-light", [[0, "Unchanged"], [1, "Marquee"], [2, "Breathing"], [3, "Static"], [4, "Blink"], [5, "Color cycle"]], 0)}
      ${range("Lighting speed", "hs-light-speed", 10, 100, 1, 50, " ms")}
      <div class="stat-row"><span class="stat-label">Dongle firmware</span><span class="stat-value">${escapeHtml(hs.dongleVersion)}</span></div>
      <div class="stat-row"><span class="stat-label">Headset firmware</span><span class="stat-value">${escapeHtml(hs.headsetVersion)}</span></div>` : `<div class="notice">Read the headset to load its settings.</div>`}
      ${actions}
    </article>
    <article class="card">
      <div class="card-title"><h2>Custom EQ</h2><span>10-band parametric</span></div>
      ${state.headsetPeq ? peqRows("hs-peq", state.headsetPeq, BOLUTEK_PEQ_TYPES) : `<div class="notice">Read the EQ to edit the Custom preset.</div>`}
      <div class="actions">
        <button class="button" type="button" data-hs-peq-read>Read EQ</button>
        <button class="button primary" type="button" data-hs-peq-write ${state.headsetPeq ? "" : "disabled"}>Write EQ</button>
      </div>
    </article>
  </div>`;
}

function renderHeadset() {
  const box = $("headset");
  if (box) box.innerHTML = headsetCards();
}

function headsetDevice() {
  return state.headsets?.[state.headsetIndex ?? 0];
}

async function readHeadset() {
  const device = headsetDevice();
  if (!device) return;
  state.headset = await call(device.family === "ktmicro" ? "headset_kt_state" : "headset_bolutek_state", { path: device.path }, `${device.name} read`);
  renderHeadset();
}

export async function loadHeadset() {
  try { state.headsets = await invoke("headset_list"); } catch (error) { state.headsets = []; setStatus(errorText(error)); }
  renderHeadset();
  if (headsetDevice() && !state.headset) await readHeadset().catch((error) => show("hs-output", errorText(error)));
}

async function headsetClick(t) {
  const d = t.dataset;
  const device = headsetDevice();
  if ("hsScan" in d) { await guarded(t, loadHeadset); return true; }
  if (!device) return false;
  if ("hsRead" in d) { await guarded(t, readHeadset); return true; }
  if ("hsApply" in d) {
    await guarded(t, async () => {
      if (device.family === "ktmicro") {
        const next = { ...state.headset, volume: num("hs-volume", 50), micGain: num("hs-mic", 0), dacEq: checked("hs-dac-eq"), adcEq: checked("hs-adc-eq"), dacPoints: readPeq("hs-dac", state.headset.dacPoints.length), adcPoints: readPeq("hs-adc", state.headset.adcPoints.length) };
        state.headset = await call("headset_kt_apply", { path: device.path, state: next }, "Headset settings saved");
      } else {
        const patch = { eqMode: num("hs-eq", 0), surround: checked("hs-surround"), spatial: checked("hs-spatial"), sidetone: checked("hs-sidetone"), sidetoneVolume: num("hs-sidetone-vol", 0), promptVolume: num("hs-prompt", 8), autoPowerOff: num("hs-apo", 0) };
        if (num("hs-light", 0) > 0) { patch.lightMode = num("hs-light", 3); patch.lightSpeed = num("hs-light-speed", 50); }
        state.headset = await call("headset_bolutek_apply", { path: device.path, patch }, "Headset settings applied");
      }
      renderHeadset();
    });
    return true;
  }
  if ("hsPeqRead" in d) { await guarded(t, async () => { state.headsetPeq = await call("headset_bolutek_peq", { path: device.path }, "EQ read"); renderHeadset(); }); return true; }
  if ("hsPeqWrite" in d) {
    await guarded(t, async () => {
      const points = readPeq("hs-peq", state.headsetPeq.length);
      await call("headset_bolutek_set_peq", { path: device.path, points }, "Custom EQ written");
      state.headsetPeq = points;
      if (state.headset) state.headset.eqMode = 2;
      renderHeadset();
    });
    return true;
  }
  if ("hsReset" in d) {
    if (t.dataset.confirm !== "1") { t.dataset.confirm = "1"; t.textContent = "Click again to reset"; return true; }
    await guarded(t, async () => { await call("headset_factory_reset", { path: device.path, family: device.family }, "Headset reset to factory settings"); state.headset = null; await readHeadset(); });
    return true;
  }
  return false;
}

/* ------------------------------------------------------------ music rhythm -- */

const RHYTHM_EFFECTS = [["spectrum", "Spectrum bars"], ["center", "Centre bars"], ["pulse", "Beat pulse"], ["wave", "Colour wave"], ["ripple", "Bass ripple"]];
const RHYTHM_PALETTES = [["gradient", "Green → red"], ["rainbow", "Rainbow"], ["cycle", "Colour cycle"], ["single", "Single colour"]];

export function rhythmCards() {
  const r = state.rhythm;
  if (!r) return `<div class="notice">Loading music rhythm…</div>`;
  const p = r.prefs;
  return `
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Music rhythm</h2><span>${r.running ? "playing" : "stopped"}</span></div>
        <canvas id="rhythm-canvas" width="640" height="150" style="width:100%;height:150px;background:#111;border-radius:8px;display:block"></canvas>
        <div class="notice" style="margin-top:8px">Listening to: ${escapeHtml(r.device ?? "default speakers")}</div>
        <div class="actions">
          <button class="button primary" type="button" data-rhythm-toggle>${r.running ? "Stop" : "Start"}</button>
        </div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Effect</h2><span>keyboard backlight</span></div>
        ${select("Effect", "rh-effect", RHYTHM_EFFECTS, p.effect)}
        ${select("Colours", "rh-palette", RHYTHM_PALETTES, p.palette)}
        <div class="form-row"><label for="rh-color">Single colour</label><input type="color" id="rh-color" value="${rgbHex(p.color)}" /></div>
        ${range("Sensitivity", "rh-sens", 10, 300, 5, p.sensitivity, "%")}
        ${range("Fall speed", "rh-gravity", 1, 60, 1, p.gravity)}
        ${toggle("Drive the keyboard backlight", "rh-kb", p.keyboard)}
        ${toggle("Start with ATK HUB", "rh-auto", p.autostart)}
        <div class="actions"><button class="button primary" type="button" data-rhythm-save>Save</button></div>
      </article>
    </div>`;
}

function readRhythm() {
  const c = $("rh-color")?.value ?? "#00aaff";
  return {
    effect: $("rh-effect")?.value ?? "spectrum", palette: $("rh-palette")?.value ?? "gradient",
    color: [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16)),
    sensitivity: num("rh-sens", 100), gravity: num("rh-gravity", 20), keyboard: checked("rh-kb"), autostart: checked("rh-auto"),
  };
}

function drawRhythm(bands) {
  const canvas = $("rhythm-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const { width, height } = canvas;
  ctx.clearRect(0, 0, width, height);
  const w = width / bands.length;
  bands.forEach((v, i) => {
    const h = Math.max(2, v * height);
    ctx.fillStyle = `hsl(${120 - v * 120}, 90%, 50%)`;
    ctx.fillRect(i * w + 1, height - h, w - 2, h);
  });
}

async function rhythmClick(t) {
  const d = t.dataset;
  if ("rhythmToggle" in d) {
    await guarded(t, async () => {
      state.rhythm = await invoke(state.rhythm?.running ? "rhythm_stop" : "rhythm_start");
      if (!state.rhythm.running) drawRhythm(new Array(64).fill(0));
      renderPage();
    });
    return true;
  }
  if ("rhythmSave" in d) {
    await guarded(t, async () => { state.rhythm = await call("rhythm_set", { settings: readRhythm() }, "Music rhythm saved"); renderPage(); });
    return true;
  }
  return false;
}

if (window.__TAURI__?.event) {
  window.__TAURI__.event.listen("rhythm:frame", (event) => drawRhythm(event.payload?.bands ?? []));
}

/* --------------------------------------------------------- live wallpapers -- */

const WALL_STYLE = [["brightness", "Brightness", 0, 200, "%"], ["contrast", "Contrast", 0, 200, "%"], ["saturation", "Saturation", 0, 200, "%"],
  ["blur", "Blur", 0, 20, "px"], ["hueRotate", "Hue", 0, 360, "°"], ["grayscale", "Grayscale", 0, 100, "%"]];

export function liveWallCards() {
  const w = state.livewall;
  if (!w) return `<div class="notice">Loading your wallpapers…</div>`;
  const byId = new Map(w.wallpapers.map((p) => [p.id, p]));
  const sel = w.wallpapers.find((p) => p.id === state.livewallSel) ?? byId.get(w.current) ?? w.wallpapers[0];
  const displays = w.displays.map((d) => {
    const on = byId.get(d.wallpaperId);
    return `<div class="list-item">
      <span><strong>${escapeHtml(d.name)}</strong> · ${d.width}×${d.height}${on ? ` · ${escapeHtml(on.title)}` : " · system wallpaper"}</span>
      <span class="actions" style="margin:0">
        <label class="toggle-row" style="margin:0"><span>Use</span><input type="checkbox" id="lw-disp-${d.index}" checked /><span class="switch"></span></label>
        ${on ? `<button class="button sm" type="button" data-lw-clear="${d.index}">Clear</button>` : ""}
      </span>
    </div>`;
  }).join("");
  const mine = w.wallpapers.map((p) => `
    <div class="wallpaper-card${sel && p.id === sel.id ? " selected" : ""}">
      ${p.previewUrl ? `<img class="wallpaper-thumb" src="${escapeHtml(p.previewUrl)}" alt="" loading="lazy" />` : `<div class="wallpaper-thumb notice">${escapeHtml(p.fileType.toUpperCase())}</div>`}
      <div class="wallpaper-title" title="${escapeHtml(p.title)}">${escapeHtml(p.title)}</div>
      <div class="wallpaper-meta">
        <span>${escapeHtml(p.fileType)} · ${(p.fileSize / 1048576).toFixed(1)} MB</span>
        <span class="actions" style="margin:0">
          <button class="button sm primary" type="button" data-lw-apply="${escapeHtml(p.id)}">Apply</button>
          <button class="button sm" type="button" data-lw-pick="${escapeHtml(p.id)}">Style</button>
          ${p.fileType === "image" ? `<button class="button sm" type="button" data-lw-system="${escapeHtml(p.id)}" title="Set as the classic Windows wallpaper">System</button>` : ""}
          <button class="button sm" type="button" data-lw-delete="${escapeHtml(p.id)}">Delete</button>
        </span>
      </div>
    </div>`).join("");
  const style = sel && sel.fileType !== "html" ? WALL_STYLE.map(([k, label, min, max, unit]) =>
    range(label, `lw-st-${k}`, min, max, 1, sel.style?.[k] ?? (k === "brightness" || k === "contrast" || k === "saturation" ? 100 : 0), unit)).join("") : "";
  return `
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Displays</h2><span>${w.displays.length} connected</span></div>
        ${displays || `<div class="notice">No displays found.</div>`}
        <div class="actions">
          <button class="button" type="button" data-lw-clear="all">Clear All</button>
          <button class="button" type="button" data-lw-refresh>Refresh</button>
        </div>
        <div class="notice" style="margin-top:10px">Apply puts the wallpaper behind your desktop icons on every display ticked above. HTML wallpapers react to music while Music Rhythm is running.</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Wallpaper style</h2><span>${sel ? escapeHtml(sel.title) : "none"}</span></div>
        ${!sel ? `<div class="notice">Download a wallpaper from the catalogue first.</div>`
          : sel.fileType === "html" ? `<div class="notice">HTML wallpapers style themselves.</div>`
          : `${style}<div class="actions"><button class="button primary" type="button" data-lw-style="${escapeHtml(sel.id)}">Save Style</button><button class="button" type="button" data-lw-style-reset="${escapeHtml(sel.id)}">Reset</button></div>`}
        <div class="card-title" style="margin-top:14px"><h2>Import</h2><span>.zip, video or image</span></div>
        <input id="lw-import" type="file" accept=".zip,.mp4,.webm,.mov,.jpg,.jpeg,.png,.bmp,.webp,.gif" />
        <div class="actions"><button class="button" type="button" data-lw-import>Import File</button></div>
      </article>
    </div>
    <article class="card">
      <div class="card-title"><h2>My wallpapers</h2><span>${w.wallpapers.length} downloaded</span></div>
      <div class="wallpaper-grid">${mine || `<div class="notice">Nothing downloaded yet.</div>`}</div>
    </article>`;
}

async function liveWallLoad() {
  state.livewall = await invoke("livewall_list").catch(() => null);
  renderPage();
}

async function liveWallClick(t) {
  const d = t.dataset;
  const done = (result, msg) => { state.livewall = result; renderPage(); if (msg) setStatus(msg); };
  const checkedDisplays = () => (state.livewall?.displays ?? []).filter((x) => checked(`lw-disp-${x.index}`)).map((x) => x.index);
  if (d.wallpaperDownload !== undefined) {
    await guarded(t, async () => {
      t.textContent = "0%";
      const result = await invoke("livewall_download", { id: d.wallpaperDownload, url: d.url, title: d.title ?? "", previewUrl: d.preview ?? "" });
      state.livewallSel = d.wallpaperDownload;
      done(result, "Wallpaper downloaded");
      if (d.apply === "1") done(await invoke("livewall_apply", { id: d.wallpaperDownload, displays: checkedDisplays().length ? checkedDisplays() : null }), "Wallpaper applied");
    });
    return true;
  }
  if (d.lwApply !== undefined) {
    await guarded(t, async () => {
      const list = checkedDisplays();
      done(await invoke("livewall_apply", { id: d.lwApply, displays: list.length ? list : null }), "Wallpaper applied");
    });
    return true;
  }
  if (d.lwClear !== undefined) {
    await guarded(t, async () => done(await invoke("livewall_clear", { displays: d.lwClear === "all" ? null : [Number(d.lwClear)] }), "Wallpaper cleared"));
    return true;
  }
  if (d.lwDelete !== undefined) {
    if (d.confirm !== "1") { d.confirm = "1"; t.textContent = "Confirm"; return true; }
    await guarded(t, async () => done(await invoke("livewall_delete", { id: d.lwDelete }), "Wallpaper deleted"));
    return true;
  }
  if (d.lwPick !== undefined) { state.livewallSel = d.lwPick; renderPage(); return true; }
  if (d.lwRefresh !== undefined) { await liveWallLoad(); return true; }
  if (d.lwSystem !== undefined) { await guarded(t, () => call("livewall_set_system", { id: d.lwSystem }, "Windows wallpaper set")); return true; }
  if (d.lwStyle !== undefined || d.lwStyleReset !== undefined) {
    const id = d.lwStyle ?? d.lwStyleReset;
    const style = Object.fromEntries(WALL_STYLE.map(([k]) => [k, d.lwStyleReset !== undefined ? (["brightness", "contrast", "saturation"].includes(k) ? 100 : 0) : num(`lw-st-${k}`, 0)]));
    await guarded(t, async () => done(await invoke("livewall_style", { id, style }), "Wallpaper style saved"));
    return true;
  }
  if (d.lwImport !== undefined) {
    const file = $("lw-import")?.files?.[0];
    if (!file) { setStatus("Choose a file to import first"); return true; }
    await guarded(t, async () => {
      const bytes = new Uint8Array(await file.arrayBuffer());
      done(await invoke("livewall_import", bytes, { headers: { "x-file-name": encodeURIComponent(file.name) } }), `${file.name} imported`);
    });
    return true;
  }
  if (d.lwCancel !== undefined) { await invoke("livewall_cancel", { id: d.lwCancel }).catch(() => {}); return true; }
  return false;
}

if (window.__TAURI__?.event) {
  window.__TAURI__.event.listen("livewall:progress", (event) => {
    const { id, percent } = event.payload ?? {};
    const el = document.querySelector(`[data-wallpaper-download="${CSS.escape(String(id))}"]`);
    if (el) el.textContent = `${percent}%`;
  });
}

/* ------------------------------------------------------ selection toolbar -- */

export function toolbarPage() {
  const heading = pageHeading(state.pages.find((page) => page.id === "toolbar"));
  const p = state.toolbar;
  if (!p) return `${heading}<div class="notice">Loading toolbar settings…</div>`;
  const buttons = [...p.buttons].sort((a, b) => a.order - b.order);
  return `
    ${heading}
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>Floating toolbar</h2><span>${p.enabled ? "on" : "off"}</span></div>
        ${toggle("Enable the selection toolbar", "tb-enabled", p.enabled)}
        ${toggle("Show after selecting text with the mouse", "tb-mouse", p.mouseGesture)}
        ${toggle("Show after pressing Ctrl twice", "tb-ctrl", p.doubleCtrl)}
        ${toggle("Show whenever text is copied", "tb-clip", p.clipboardMonitor)}
        ${toggle("Restore the clipboard after grabbing a selection", "tb-restore", p.restoreClipboard)}
        ${select("Fade out after", "tb-hide", [[0, "Never"], [2000, "2 seconds"], [4000, "4 seconds"], [8000, "8 seconds"], [15000, "15 seconds"]], p.autoHideMs)}
        <div class="notice" style="margin-top:10px">Grabbing a selection sends Ctrl+C to the app you're working in, the same way ATK HUB does.</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Buttons</h2><span>top to bottom = left to right</span></div>
        ${buttons.map((b, i) => `<div class="actions" style="align-items:center">
          <label class="toggle-row" style="flex:1"><span>${escapeHtml(b.label)}</span><input type="checkbox" id="tb-btn-${i}" data-kind="${escapeHtml(b.kind)}" ${b.enabled ? "checked" : ""} /><span class="switch"></span></label>
          <button class="button sm" type="button" data-tb-move="${i}" data-dir="-1" ${i ? "" : "disabled"}>↑</button>
          <button class="button sm" type="button" data-tb-move="${i}" data-dir="1" ${i < buttons.length - 1 ? "" : "disabled"}>↓</button>
        </div>`).join("")}
        ${field("AI server address", "tb-ai-base", p.ai.baseUrl)}
        ${field("AI request path", "tb-ai-path", p.ai.path)}
        ${field("API key (optional)", "tb-ai-key", p.ai.apiKey, "password")}
        ${field("Test text", "tb-test", "Hello from ATK HUB")}
        <div class="actions">
          <button class="button primary" type="button" data-tb-save>Save</button>
          <button class="button" type="button" data-tb-preview>Show Toolbar</button>
          <button class="button" type="button" data-tb-reset>Defaults</button>
        </div>
      </article>
    </div>`;
}

function readToolbar() {
  const p = state.toolbar;
  const buttons = [...p.buttons].sort((a, b) => a.order - b.order).map((b, i) => ({ ...b, enabled: checked(`tb-btn-${i}`), order: i }));
  return {
    ...p, enabled: checked("tb-enabled"), mouseGesture: checked("tb-mouse"), doubleCtrl: checked("tb-ctrl"), clipboardMonitor: checked("tb-clip"),
    restoreClipboard: checked("tb-restore"), autoHideMs: num("tb-hide", 4000), buttons,
    ai: { baseUrl: $("tb-ai-base")?.value.trim() ?? "", path: $("tb-ai-path")?.value.trim() || "/api/ai-tools", apiKey: $("tb-ai-key")?.value ?? "" },
  };
}

async function toolbarClick(t) {
  const d = t.dataset;
  if ("tbSave" in d) { await guarded(t, async () => { state.toolbar = await call("toolbar_set", { settings: readToolbar() }, "Selection toolbar saved"); renderPage(); }); return true; }
  if ("tbPreview" in d) { await guarded(t, () => invoke("toolbar_preview", { text: $("tb-test")?.value ?? "" })); return true; }
  if ("tbReset" in d) {
    const ai = state.toolbar.ai;
    state.toolbar = { enabled: false, mouseGesture: true, doubleCtrl: true, clipboardMonitor: false, restoreClipboard: true, autoHideMs: 4000, ai,
      buttons: [["translate", "Translate"], ["summarize", "Summarize"], ["polish", "Polish"], ["search", "Search"]].map(([kind, label], order) => ({ id: kind, kind, label, enabled: true, order })) };
    renderPage();
    return true;
  }
  if ("tbMove" in d) {
    const next = readToolbar();
    const i = Number(d.tbMove);
    const j = i + Number(d.dir);
    [next.buttons[i], next.buttons[j]] = [next.buttons[j], next.buttons[i]];
    next.buttons.forEach((b, n) => { b.order = n; });
    state.toolbar = next;
    renderPage();
    return true;
  }
  return false;
}

export function petControls() {
  const p = state.petState ?? {};
  return `
    ${toggle("Wooden fish enabled", "pet-enabled", p.enabled)}
    ${toggle("Show desktop pet", "pet-visible", p.petVisible)}
    ${toggle("Knock sound", "pet-sound", p.sound)}
    <div class="actions"><button class="button primary" type="button" data-pet-save>Save Pet Settings</button></div>`;
}

/* ----------------------------------------------------------- dot screen -- */

const DS_SWATCHES = ["#ff6900", "#ff2846", "#00b4ff", "#22c55e", "#facc15", "#a855f7", "#ffffff", "#000000"];

function blankDotFrame() {
  return Array.from({ length: 6 }, () => Array.from({ length: 6 }, () => [0, 0, 0]));
}

function hexToRgb(value) {
  const c = String(value || "#000000").replace("#", "");
  return [0, 2, 4].map((i) => parseInt(c.slice(i, i + 2), 16) || 0);
}

function cloneArt(art) {
  return JSON.parse(JSON.stringify(art));
}

function ensureDotEdit() {
  if (!state.dsColor) state.dsColor = "#ff6900";
  if (!state.dsEdit) {
    const seed = state.dotScreen?.artwork ?? state.dotScreen?.saved?.[0];
    state.dsEdit = seed ? cloneArt(seed) : { version: 1, name: "Custom", frameRate: 2, frames: [blankDotFrame()] };
  }
  if (!Array.isArray(state.dsEdit.frames) || !state.dsEdit.frames.length) {
    state.dsEdit.frames = [blankDotFrame()];
  }
  state.dsFrame = Math.min(state.dsFrame ?? 0, state.dsEdit.frames.length - 1);
}

async function loadDotScreen() {
  state.dotScreen = await invoke("dotscreen_status").catch(() => ({
    running: false, connected: false, artwork: null, frameIndex: 0, error: null, targets: [], saved: [], autoReconnect: true,
  }));
  ensureDotEdit();
  renderPage();
}

export function dotScreenPage() {
  const ds = state.dotScreen;
  if (!ds) return `<div class="notice">Loading dot-matrix screen…</div>`;
  ensureDotEdit();
  const art = state.dsEdit;
  const frame = art.frames[state.dsFrame] ?? blankDotFrame();
  const targets = ds.targets ?? [];
  const targetOpts = targets.length
    ? targets.map((t) => [t.path, `${t.product} (${toHex16(t.vendorId)}:${toHex16(t.productId)})`])
    : [["", "No supported keyboard connected (VID 0x373B)"]];
  const badge = ds.running ? (ds.connected ? `streaming · frame ${(ds.frameIndex ?? 0) + 1}` : "waiting for keyboard") : "stopped";
  const cells = frame.map((row, r) => row.map((px, c) => {
    const on = px[0] || px[1] || px[2];
    const bg = on ? `rgb(${px[0]}, ${px[1]}, ${px[2]})` : "#18181b";
    return `<button type="button" class="ds-cell${on ? " on" : ""}" data-ds-cell="${r},${c}" style="background:${bg}" title="Row ${r + 1}, Col ${c + 1}"></button>`;
  }).join("")).join("");
  const swatches = DS_SWATCHES.map((hexColor) => `<button type="button" class="ds-swatch${state.dsColor === hexColor ? " active" : ""}" data-ds-swatch="${hexColor}" style="background:${hexColor}" title="${hexColor}"></button>`).join("");
  const frameBtns = art.frames.map((_, i) => `<button type="button" class="button sm${i === state.dsFrame ? " primary" : ""}" data-ds-frame="${i}">${i + 1}</button>`).join("");
  const savedRows = (ds.saved ?? []).map((item) => `
    <div class="list-item">
      <div><strong>${escapeHtml(item.name)}</strong><small style="display:block;color:var(--muted)">${item.frames.length} frame(s) · ${item.frameRate} FPS</small></div>
      <div class="actions">
        <button class="button sm" type="button" data-ds-load="${escapeHtml(item.name)}">Load</button>
        <button class="button sm primary" type="button" data-ds-play-saved="${escapeHtml(item.name)}">Play</button>
        <button class="button sm" type="button" data-ds-delete="${escapeHtml(item.name)}">Delete</button>
      </div>
    </div>`).join("");

  return `
    <div class="grid two">
      <article class="card">
        <div class="card-title"><h2>6×6 Pixel Editor</h2><span>${escapeHtml(badge)}</span></div>
        <div class="ds-grid">${cells}</div>
        <div class="ds-swatches">
          <input type="color" id="ds-color" value="${escapeHtml(state.dsColor)}" style="width:42px;height:34px;padding:2px" />
          ${swatches}
        </div>
        <div class="actions" style="margin-top:12px">
          <button class="button sm" type="button" data-ds-fill>Fill Frame</button>
          <button class="button sm" type="button" data-ds-clear>Clear Frame</button>
          <button class="button sm" type="button" data-ds-add-frame ${art.frames.length >= 20 ? "disabled" : ""}>+ Frame</button>
          <button class="button sm" type="button" data-ds-dup-frame ${art.frames.length >= 20 ? "disabled" : ""}>Duplicate</button>
          <button class="button sm" type="button" data-ds-del-frame ${art.frames.length <= 1 ? "disabled" : ""}>Remove Frame</button>
        </div>
        <div class="actions" style="margin-top:10px">${frameBtns}</div>
      </article>
      <article class="card">
        <div class="card-title"><h2>Playback &amp; Device</h2><span>${targets.length} target(s)</span></div>
        ${select("Keyboard", "ds-target", targetOpts, $("ds-target")?.value ?? targets[0]?.path ?? "")}
        ${field("Name", "ds-name", art.name)}
        ${range("Frame rate", "ds-fps", 1, 6, 1, art.frameRate, " FPS")}
        ${toggle("Auto-reconnect saved animation", "ds-auto", ds.autoReconnect)}
        ${ds.error ? `<div class="notice" style="margin-bottom:10px">${escapeHtml(ds.error)}</div>` : ""}
        <div class="actions">
          <button class="button primary" type="button" data-ds-start>Play on Keyboard</button>
          <button class="button" type="button" data-ds-stop ${ds.running ? "" : "disabled"}>Stop</button>
          <button class="button" type="button" data-ds-save>Save to Library</button>
          <button class="button sm" type="button" data-ds-refresh>Refresh</button>
        </div>
        <div class="actions" style="margin-top:10px">
          <button class="button sm" type="button" data-ds-export>Export JSON</button>
          <label class="button sm" style="cursor:pointer">Import JSON<input type="file" id="ds-import" accept=".json,application/json" hidden /></label>
        </div>
      </article>
    </div>
    <article class="card" style="margin-top:16px">
      <div class="card-title"><h2>Saved Animations</h2><span>${(ds.saved ?? []).length} saved</span></div>
      <div class="list">${savedRows || `<div class="notice">No saved animations yet.</div>`}</div>
    </article>`;
}

async function dotScreenClick(t) {
  const d = t.dataset;
  if (d.dsCell !== undefined) {
    ensureDotEdit();
    const [r, c] = d.dsCell.split(",").map(Number);
    const frame = state.dsEdit.frames[state.dsFrame];
    const next = hexToRgb(state.dsColor);
    const cur = frame[r][c];
    frame[r][c] = (cur[0] === next[0] && cur[1] === next[1] && cur[2] === next[2]) ? [0, 0, 0] : next;
    renderPage();
    return true;
  }
  if (d.dsSwatch !== undefined) { state.dsColor = d.dsSwatch; renderPage(); return true; }
  if (d.dsFrame !== undefined) { state.dsFrame = Number(d.dsFrame); renderPage(); return true; }
  if ("dsFill" in d) {
    ensureDotEdit();
    const rgb = hexToRgb(state.dsColor);
    state.dsEdit.frames[state.dsFrame] = Array.from({ length: 6 }, () => Array.from({ length: 6 }, () => [...rgb]));
    renderPage();
    return true;
  }
  if ("dsClear" in d) {
    ensureDotEdit();
    state.dsEdit.frames[state.dsFrame] = blankDotFrame();
    renderPage();
    return true;
  }
  if ("dsAddFrame" in d) {
    ensureDotEdit();
    if (state.dsEdit.frames.length < 20) {
      state.dsEdit.frames.push(blankDotFrame());
      state.dsFrame = state.dsEdit.frames.length - 1;
      renderPage();
    }
    return true;
  }
  if ("dsDupFrame" in d) {
    ensureDotEdit();
    if (state.dsEdit.frames.length < 20) {
      state.dsEdit.frames.splice(state.dsFrame + 1, 0, cloneArt(state.dsEdit.frames[state.dsFrame]));
      state.dsFrame += 1;
      renderPage();
    }
    return true;
  }
  if ("dsDelFrame" in d) {
    ensureDotEdit();
    if (state.dsEdit.frames.length > 1) {
      state.dsEdit.frames.splice(state.dsFrame, 1);
      state.dsFrame = Math.min(state.dsFrame, state.dsEdit.frames.length - 1);
      renderPage();
    }
    return true;
  }
  if ("dsRefresh" in d) { await guarded(t, loadDotScreen); return true; }
  if ("dsStart" in d) {
    await guarded(t, async () => {
      ensureDotEdit();
      state.dsEdit.name = ($("ds-name")?.value ?? state.dsEdit.name).trim() || "Custom";
      state.dsEdit.frameRate = num("ds-fps", state.dsEdit.frameRate || 2);
      const path = $("ds-target")?.value || null;
      state.dotScreen = await call("dotscreen_start", { artwork: state.dsEdit, path }, "Dot-screen animation started");
      renderPage();
    });
    return true;
  }
  if ("dsStop" in d) {
    await guarded(t, async () => {
      state.dotScreen = await call("dotscreen_stop", {}, "Dot-screen playback stopped");
      renderPage();
    });
    return true;
  }
  if ("dsSave" in d) {
    await guarded(t, async () => {
      ensureDotEdit();
      state.dsEdit.name = ($("ds-name")?.value ?? state.dsEdit.name).trim() || "Custom";
      state.dsEdit.frameRate = num("ds-fps", state.dsEdit.frameRate || 2);
      state.dotScreen = await call("dotscreen_save", { artwork: state.dsEdit }, "Dot-screen artwork saved");
      renderPage();
    });
    return true;
  }
  if (d.dsLoad !== undefined) {
    const found = (state.dotScreen?.saved ?? []).find((a) => a.name === d.dsLoad);
    if (found) { state.dsEdit = cloneArt(found); state.dsFrame = 0; renderPage(); setStatus(`Loaded "${found.name}"`); }
    return true;
  }
  if (d.dsPlaySaved !== undefined) {
    const found = (state.dotScreen?.saved ?? []).find((a) => a.name === d.dsPlaySaved);
    if (found) {
      await guarded(t, async () => {
        state.dsEdit = cloneArt(found);
        state.dsFrame = 0;
        state.dotScreen = await call("dotscreen_start", { artwork: found, path: $("ds-target")?.value || null }, `Playing "${found.name}"`);
        renderPage();
      });
    }
    return true;
  }
  if (d.dsDelete !== undefined) {
    await guarded(t, async () => {
      state.dotScreen = await call("dotscreen_delete", { name: d.dsDelete }, `Deleted "${d.dsDelete}"`);
      renderPage();
    });
    return true;
  }
  if ("dsExport" in d) {
    ensureDotEdit();
    const blob = new Blob([JSON.stringify(state.dsEdit, null, 2)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${(state.dsEdit.name || "dotscreen").replace(/[^a-z0-9_-]+/gi, "_")}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
    return true;
  }
  return false;
}

if (window.__TAURI__?.event) {
  window.__TAURI__.event.listen("dotscreen:status", (event) => {
    if (!event.payload) return;
    state.dotScreen = event.payload;
    if (state.current === "dotscreen") renderPage();
  });
}

/* --------------------------------------------------------------- events -- */

function syncRange(input) {
  const label = document.querySelector(`.range-value[data-for="${input.id}"]`);
  if (label) label.textContent = `${input.value}${input.dataset.unit ?? ""}`;
}

async function guarded(target, work) {
  target.disabled = true;
  try { await work(); } catch (error) { setStatus(errorText(error)); show("mouse-output", errorText(error)); show("kb-output", errorText(error)); } finally { target.disabled = false; }
}

export function wireLiveEvents() {
  document.addEventListener("click", async (event) => {
    const t = event.target.closest("button, [data-mouse-profile], .kb-key[data-r]");
    if (!t) return;
    const d = t.dataset;
    if (await kbHandleClick(t)) return undefined;
    if (await headsetClick(t)) return undefined;
    if (await toolbarClick(t)) return undefined;
    if ("mouseRead" in d) return guarded(t, readMouse);
    if ("mouseApply" in d) {
      return guarded(t, async () => {
        state.mouse = await call("mouse_apply", { patch: readMousePatch() }, "Mouse settings applied");
        renderPage();
      });
    }
    if ("obWrite" in d) return guarded(t, writeOnboard);
    if ("obRead" in d) return guarded(t, readOnboard);
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
      return saveRules([...(state.gameRules ?? []), { exe, profile: num("game-profile", 1), ...($("game-kb-profile").value === "" ? {} : { keyboardProfile: num("game-kb-profile", 0) }) }]);
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
    if (await companionClick(t)) return undefined;
    if (await pcClick(t)) return undefined;
    if (await liveWallClick(t)) return undefined;
    if (await rhythmClick(t)) return undefined;
    if (await dotScreenClick(t)) return undefined;
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
    if (event.target.id === "kb-layer") {
      state.kbLayer = Number(event.target.value);
      invoke("kb_keymap", { layer: state.kbLayer }).then((map) => { state.kbMap = map; renderKeyboard(); }).catch((error) => setStatus(errorText(error)));
    }
    if (event.target.id === "hs-device") { state.headsetIndex = Number(event.target.value); state.headset = null; state.headsetPeq = null; loadHeadset(); }
    if (event.target.id === "adv-kind") { state.advKind = event.target.value; state.advKey1 = null; state.advKey2 = null; renderPage(); }
    if (event.target.id === "adv-slot") { state.advSlot = Number(event.target.value); state.advKey1 = null; state.advKey2 = null; renderPage(); }
    if (event.target.id === "kbm-slot") { state.kbMacroSlot = Number(event.target.value); renderPage(); }
    if (event.target.id === "ob-kind") {
      state.obKind = event.target.value;
      document.querySelectorAll("[data-ob]").forEach((el) => { el.hidden = el.dataset.ob !== state.obKind; });
    }
    if (event.target.id === "fish-sound-file" || event.target.id === "dp-sprite-file") companionFile(event.target);
    if (event.target.id === "pc-import" && event.target.files?.[0]) {
      event.target.files[0].text().then((text) => { $("pc-script").value = text; show("pc-output", `Imported ${event.target.files[0].name}`); });
    }
    if (event.target.id === "merit-period") { state.meritPeriod = event.target.value; renderCompanions(); }
    if (event.target.id === "ds-auto") {
      invoke("dotscreen_auto_reconnect", { enabled: event.target.checked }).then((s) => { state.dotScreen = s; }).catch(() => {});
    }
    if (event.target.id === "ds-color") { state.dsColor = event.target.value; }
    if (event.target.id === "ds-fps" && state.dsEdit) { state.dsEdit.frameRate = num("ds-fps", 2); renderPage(); }
    if (event.target.id === "ds-name" && state.dsEdit) { state.dsEdit.name = event.target.value; }
    if (event.target.id === "ds-import" && event.target.files?.[0]) {
      event.target.files[0].text().then((text) => {
        try {
          const parsed = JSON.parse(text);
          if (parsed && Array.isArray(parsed.frames) && parsed.frames.length) {
            state.dsEdit = { version: 1, name: parsed.name || "Imported", frameRate: Math.min(6, Math.max(1, Number(parsed.frameRate) || 2)), frames: parsed.frames.slice(0, 20) };
            state.dsFrame = 0;
            renderPage();
            setStatus("Dot-screen artwork imported");
          }
        } catch (error) { setStatus(`Invalid JSON: ${error.message}`); }
      });
    }
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
    if (!state.macros?.length) {
      try { state.macros = (await invoke("macros_load")) ?? []; } catch { state.macros = []; }
    }
    if (!state.mouse && !state.mouseLoading) {
      state.mouseLoading = true;
      await readMouse();
      state.mouseLoading = false;
    }
  } else if (pageId === "keyboard") {
    await loadKeyboard();
    if (!state.kb && !state.kbLoading) {
      state.kbLoading = true;
      await readKeyboard();
      state.kbLoading = false;
    }
  } else if (pageId === "pcscript") {
    if (!state.pc) await loadPcScript();
  } else if (pageId === "music") {
    if (!state.rhythm) {
      state.rhythm = await invoke("rhythm_get").catch(() => null);
      if (state.rhythm) renderPage();
    }
  } else if (pageId === "wallpapers") {
    if (!state.livewall) await liveWallLoad();
  } else if (pageId === "dotscreen") {
    if (!state.dotScreen) {
      await loadDotScreen();
    }
  } else if (pageId === "toolbar") {
    if (!state.toolbar) {
      state.toolbar = await invoke("toolbar_get").catch(() => null);
      if (state.toolbar) renderPage();
    }
  } else if (pageId === "audio") {
    if (!state.headsets) await loadHeadset();
    else renderHeadset();
  } else if (pageId === "pets") {
    await loadCompanions();
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
