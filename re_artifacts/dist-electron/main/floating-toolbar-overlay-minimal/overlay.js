/* global window, document */
const { ipcRenderer } = require("electron");

const CHANNEL_SET_TEXT = "floating-toolbar-overlay-set-text";

/** @type {string} */
let overlayText = "";

function applyText(t) {
  overlayText = String(t ?? "");
  const el = document.getElementById("x");
  const cnt = document.getElementById("cnt");
  if (el) el.textContent = overlayText;
  if (cnt) cnt.textContent = overlayText.length + " 字";
}

ipcRenderer.on(CHANNEL_SET_TEXT, (_e, payload) => {
  const t = payload && payload.text != null ? String(payload.text) : "";
  applyText(t);
});

document.getElementById("copy").onclick = function () {
  ipcRenderer.send("floating-toolbar-overlay-action", { action: "copy", text: overlayText });
};
document.getElementById("hub").onclick = function () {
  ipcRenderer.send("floating-toolbar-overlay-action", { action: "open-hub" });
};
document.getElementById("close").onclick = function () {
  ipcRenderer.send("floating-toolbar-overlay-close");
};

(function () {
  var TARGET_MIN = 0.1;
  var FADE_MS = 1500;
  var IDLE_NEVER_HOVER_MS = 4000;
  var fadeRaf = null;
  var idleTimer = null;
  var hasPointerEntered = false;
  var currentOp = 1;
  function sendOp(v) {
    currentOp = v;
    try {
      ipcRenderer.send("floating-toolbar-overlay-opacity", v);
    } catch (e) {
      /* ignore */
    }
  }
  function cancelFade() {
    if (fadeRaf != null) {
      cancelAnimationFrame(fadeRaf);
      fadeRaf = null;
    }
  }
  function fadeTo(from, to, ms) {
    cancelFade();
    var start = performance.now();
    function tick(now) {
      var u = Math.min(1, (now - start) / ms);
      var v = from + (to - from) * u;
      sendOp(v);
      if (u < 1) {
        fadeRaf = requestAnimationFrame(tick);
      } else {
        fadeRaf = null;
      }
    }
    fadeRaf = requestAnimationFrame(tick);
  }
  function onEnter() {
    hasPointerEntered = true;
    if (idleTimer != null) {
      clearTimeout(idleTimer);
      idleTimer = null;
    }
    cancelFade();
    sendOp(1);
  }
  function onLeave() {
    fadeTo(currentOp, TARGET_MIN, FADE_MS);
  }
  document.body.addEventListener("pointerenter", onEnter);
  document.body.addEventListener("pointerleave", onLeave);
  idleTimer = setTimeout(function () {
    idleTimer = null;
    if (!hasPointerEntered) {
      fadeTo(1, TARGET_MIN, FADE_MS);
    }
  }, IDLE_NEVER_HOVER_MS);
})();
