/* Fresco New Tab — mirrors the current Fresco wallpaper via the local bridge. */
"use strict";

const BRIDGE = "http://127.0.0.1:8765";
const POLL_MS = 30000;
const HINT_KEY = "fresco.hintDismissed";

const bgA = document.getElementById("bg-a");
const bgB = document.getElementById("bg-b");
const clockEl = document.getElementById("clock");
const dateEl = document.getElementById("date");
const creditEl = document.getElementById("credit");
const nameEl = document.getElementById("wallpaper-name");
const hintEl = document.getElementById("hint");

const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

let activeBg = null; // the currently visible layer
let lastEpoch = null;
let currentObjectUrl = null;
let pollTimer = null;

/* ---------- Clock ---------- */

const timeFmt = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
});
const dateFmt = new Intl.DateTimeFormat(undefined, {
  weekday: "long",
  year: "numeric",
  month: "long",
  day: "numeric",
});

function tick() {
  const now = new Date();
  clockEl.textContent = timeFmt.format(now);
  dateEl.textContent = dateFmt.format(now);
  // Re-align to the next minute boundary.
  setTimeout(tick, 60000 - (now.getSeconds() * 1000 + now.getMilliseconds()) + 50);
}
tick();

/* ---------- Wallpaper ---------- */

async function fetchStatus() {
  const res = await fetch(BRIDGE + "/status", { cache: "no-store" });
  if (!res.ok) throw new Error("status " + res.status);
  return res.json();
}

async function showFrame(epoch) {
  const res = await fetch(BRIDGE + "/frame?t=" + encodeURIComponent(epoch), {
    cache: "no-store",
  });
  if (!res.ok) throw new Error("frame " + res.status);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);

  // Decode fully before swapping so the crossfade never shows a blank layer.
  await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = resolve;
    img.onerror = reject;
    img.src = url;
  }).catch((err) => {
    URL.revokeObjectURL(url);
    throw err;
  });

  const next = activeBg === bgA ? bgB : bgA;
  next.style.backgroundImage = "url(" + url + ")";

  if (reducedMotion) {
    next.classList.add("visible");
    if (activeBg) activeBg.classList.remove("visible");
  } else {
    // Force a style flush so the transition runs even on a fresh layer.
    void next.offsetWidth;
    next.classList.add("visible");
    if (activeBg) activeBg.classList.remove("visible");
  }

  const prevUrl = currentObjectUrl;
  currentObjectUrl = url;
  if (prevUrl) setTimeout(() => URL.revokeObjectURL(prevUrl), 1000);
  activeBg = next;
}

function setConnected(status) {
  hintEl.hidden = true;
  const name = status && status.name ? String(status.name) : "";
  nameEl.textContent = name;
  // "source" ("browser" | "desktop") only annotates the credit line.
  creditEl.title =
    status && status.source === "browser"
      ? "Browser-specific wallpaper"
      : "Mirroring your desktop wallpaper";
  creditEl.hidden = !name;
}

function setDisconnected() {
  // Keep the last wallpaper if we had one; otherwise the CSS gradient shows.
  creditEl.hidden = activeBg === null || creditEl.hidden;
  if (activeBg === null && localStorage.getItem(HINT_KEY) !== "1") {
    hintEl.hidden = false;
  }
}

async function refresh() {
  try {
    const status = await fetchStatus();
    setConnected(status);
    const epoch = status.changed_epoch;
    if (epoch !== lastEpoch) {
      await showFrame(epoch);
      lastEpoch = epoch;
    }
  } catch (_err) {
    setDisconnected();
  }
}

function schedulePoll() {
  if (pollTimer) clearInterval(pollTimer);
  pollTimer = setInterval(refresh, POLL_MS);
}

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    refresh();
    schedulePoll(); // reset the interval so we don't double-fire
  }
});

document.getElementById("hint-dismiss").addEventListener("click", () => {
  try {
    localStorage.setItem(HINT_KEY, "1");
  } catch (_err) {
    /* private mode — dismissal just won't persist */
  }
  hintEl.hidden = true;
});

refresh();
schedulePoll();
