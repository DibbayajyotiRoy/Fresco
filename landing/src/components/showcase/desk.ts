import type { CSSProperties } from "react";

/**
 * The desktop the What's New showcase reproduces, in Fresco's own logical
 * units (lu). The reference recordings are a 1920 x 1080 output at scale 1,
 * so one lu is 1/1920 of the screen's width (`--lu` in showcase.css).
 *
 * Every size here is the renderer's own formula (src/widgetkit in the Rust
 * app) evaluated at the settings the recordings show: NOS clock at 64 pt with
 * seconds, lyric card at 48 pt with the next line on, the visualiser at 40%
 * wide in rainbow, the disc at its default 220. Placement is the measured
 * anchor and margin of each widget on that desktop. Nothing is eyeballed from
 * a picture; the picture was only used to confirm the formulas.
 *
 * Pure data, no DOM: the server and the client compute identical numbers, so
 * the inline styles hydrate without a mismatch. Only +, x and a few Math
 * calls rounded to 3 decimals feed any style.
 */

export const SCREEN_W = 1920;
export const SCREEN_H = 1080;

export const r3 = (n: number) => Math.round(n * 1000) / 1000;
/** A length in logical units, resolved against the screen's width. */
export const lu = (n: number) => `calc(${r3(n)} * var(--lu))`;
const clamp = (v: number, lo: number, hi: number) =>
  Math.min(hi, Math.max(lo, v));

/* ---- typo.rs ------------------------------------------------------------ */

const capHeight = (s: number) => 0.727 * s;
const descender = (s: number) => 0.1 * s;
const leading = (s: number) =>
  clamp(1.62 - 0.3 * Math.log2(s / 11), 0.94, 1.62);
const tracking = (s: number, micro = false) =>
  r3(clamp(-0.0285 + 0.62 / s, -0.03, 0.14) + (micro ? 0.1 : 0));
const LADDER = [11, 14, 18, 22, 27, 34, 43, 53, 67, 84];
const ladderStep = (s: number) =>
  LADDER.reduce((best, x) =>
    Math.abs(Math.log(x / s)) < Math.abs(Math.log(best / s)) ? x : best,
  );

/* ---- theme.rs ----------------------------------------------------------- */

const radiusCard = (h: number) => clamp(4 * Math.round((0.42 * h) / 4), 12, 32);
const cardPadding = (m: number) =>
  clamp(4 * Math.round((0.055 * m + 8) / 4), 12, 28);

/** The dark palette (`Theme::dark`), which is what `WidgetTheme::Auto` is. */
export const INK = {
  primary: "#fff",
  secondary: "rgb(255 255 255 / 0.7)",
  tertiary: "rgb(255 255 255 / 0.52)",
  nosRed: "#f2555a",
  nosDim: "rgb(255 255 255 / 0.22)",
} as const;

/**
 * Inline style for one text row, placed by its **cap top** the way every
 * widgetkit card lays type out. The browser centres Inter's 1.21 em content
 * area in the line box, which puts the cap top `lh/2 - 0.364` em below the
 * box's top edge.
 */
export function textRow(o: {
  x?: number;
  capTop: number;
  size: number;
  weight: number;
  micro?: boolean;
  lh?: number;
  width?: number;
  color: string;
}): CSSProperties {
  const lh = o.lh ?? 1;
  return {
    left: o.x === undefined ? undefined : lu(o.x),
    top: lu(o.capTop - (lh / 2 - 0.3638) * o.size),
    width: o.width === undefined ? undefined : lu(o.width),
    fontSize: lu(o.size),
    fontWeight: o.weight,
    letterSpacing: `${tracking(o.size, o.micro)}em`,
    lineHeight: r3(lh),
    color: o.color,
  };
}

/* ---- dotmatrix.rs: the 5 x 7 face, digits and the colon ----------------- */

type Glyph = { rows: number[]; cols: number; adv: number };
const wide = (rows: number[]): Glyph => ({ rows, cols: 5, adv: 6 });
const GLYPHS: Record<string, Glyph> = {
  "0": wide([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
  "1": wide([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
  "2": wide([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
  "3": wide([0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
  "4": wide([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
  "5": wide([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
  "6": wide([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
  "7": wide([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
  "8": wide([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
  "9": wide([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
  ":": { rows: [0, 0b10000, 0, 0, 0b10000, 0, 0], cols: 1, adv: 2 },
};

/** Lit cells of every glyph, as [column, row]. */
export const GLYPH_DOTS: Record<string, [number, number][]> = {};
for (const [ch, g] of Object.entries(GLYPHS)) {
  const dots: [number, number][] = [];
  g.rows.forEach((bits, r) => {
    for (let c = 0; c < g.cols; c++) if (bits & (1 << (4 - c))) dots.push([c, r]);
  });
  GLYPH_DOTS[ch] = dots;
}
export const glyphAdvance = (ch: string) => GLYPHS[ch]?.adv ?? 0;
const advanceCols = (s: string) =>
  Math.max([...s].reduce((t, ch) => t + glyphAdvance(ch), 0) - 1, 0);

/* ---- canvas.rs: the squircle and CSS-style gradient lines --------------- */

function squircle(side: number, radius: number) {
  const d = Math.min(radius, side / 2.56) * 1.28;
  const k = d * 0.72;
  const s = side;
  const p = (n: number) => r3(n);
  return [
    `M${p(d)} 0H${p(s - d)}`,
    `C${p(s - d + k)} 0 ${s} ${p(d - k)} ${s} ${p(d)}`,
    `V${p(s - d)}`,
    `C${s} ${p(s - d + k)} ${p(s - d + k)} ${s} ${p(s - d)} ${s}`,
    `H${p(d)}`,
    `C${p(d - k)} ${s} 0 ${p(s - d + k)} 0 ${p(s - d)}`,
    `V${p(d)}`,
    `C0 ${p(d - k)} ${p(d - k)} 0 ${p(d)} 0Z`,
  ].join("");
}

function gradientLine(w: number, h: number, deg: number) {
  const rad = (deg * Math.PI) / 180;
  const dx = Math.sin(rad);
  const dy = -Math.cos(rad);
  const len = (w * Math.abs(dx) + h * Math.abs(dy)) / 2;
  return {
    x1: r3(w / 2 - dx * len),
    y1: r3(h / 2 - dy * len),
    x2: r3(w / 2 + dx * len),
    y2: r3(h / 2 + dy * len),
  };
}

/* ---- cards/nos.rs: the NOS clock ---------------------------------------- */

export const NOS = (() => {
  const H = 64;
  const side = clamp(4 * Math.round((4.75 * H) / 4), 96, 720);
  const radius = side * 0.3;
  const pad = cardPadding(side);
  const dot = clamp(0.042 * side, 3, 14);
  const head = dot * 1.35;
  const ringR = Math.max(side / 2 - pad - head / 2, dot);
  const dots = clamp(
    Math.round((2 * Math.PI * ringR) / (1.9 * dot)),
    24,
    72,
  );
  const innerR = Math.max(ringR - head / 2 - 8, 1);
  const contentW = 2 * innerR * 0.86;
  const contentH = 2 * innerR * 0.82;
  const micro = Math.max(ladderStep(0.05 * side), 11);
  const microCap = capHeight(micro);
  const gap = 12;
  const heroRoom = Math.max(contentH - 2 * (microCap + gap), 6);
  // Sized from the widest string the settings can reach (seconds on).
  const cols = advanceCols("00:00:00");
  const pitch = Math.min(contentW / cols, heroRoom / 7);
  const heroCap = pitch * 7;
  const blockH = microCap + gap + heroCap + gap + microCap;
  const c = side / 2;
  const microY = c - blockH / 2;
  const heroY = microY + microCap + gap;
  const captionY = heroY + heroCap + gap;
  const chord = (y: number) =>
    2 * Math.sqrt(Math.max(innerR * innerR - y * y, 0)) * 0.94;
  const ring = Array.from({ length: dots }, (_, i) => {
    const a = -Math.PI / 2 + (2 * Math.PI * i) / dots;
    return { x: r3(c + ringR * Math.cos(a)), y: r3(c + ringR * Math.sin(a)) };
  });
  return {
    side,
    radius,
    dot,
    head,
    unlit: dot * 0.55,
    ring,
    micro,
    cols,
    pitch,
    heroX: c - contentW / 2,
    microY,
    heroY,
    captionY,
    microW: chord(microY - c),
    captionW: chord(captionY + microCap - c),
    squircle: squircle(side, radius),
    fill: gradientLine(side, side, 160),
    left: SCREEN_W - 34 - side,
    top: 34,
  };
})();

/* ---- cards/nowplaying.rs: the lyric card -------------------------------- */

export const NP = (() => {
  const L = 48;
  const title = Math.max(ladderStep(0.64 * L), 11);
  const body = Math.max(ladderStep(0.5 * L), 11);
  const art = Math.max(4 * title, 24);
  const radius = radiusCard(L);
  const w = clamp(15 * L, 320, 0.9 * SCREEN_W);
  const pad = cardPadding(Math.min(w, 240));
  const dividerY = pad + art + 24;
  const lyricY = dividerY + 1 + 24;
  const lyricCap = capHeight(L);
  // One current line and the next line (every placeholder line fits one).
  const h = lyricY + lyricCap + 8 + lyricCap + descender(L) + pad;
  const colX = pad + art + 16;
  const labelY = pad;
  const titleY = labelY + capHeight(11) + 8;
  const artistY = titleY + capHeight(title) + 6;

  // The two zone scrims and the waist between them (surface.rs, §9.2):
  // a 0.225 base across the inner rect, the zones topping it up to the full
  // 0.50 scrim, their free edges feathered either side of the divider.
  const headerEdge = Math.max(
    dividerY - 12,
    pad + art + Math.max(0.35 * leading(title) * title, 6),
  );
  const lyricEdge = Math.min(
    dividerY + 12,
    lyricY - Math.max(0.35 * leading(L) * L, 6),
  );
  const s = "4 6 10";
  const scrim = `linear-gradient(to bottom, rgb(${s} / 0.5) ${lu(headerEdge - 6)}, rgb(${s} / 0.225) ${lu(headerEdge + 2)}, rgb(${s} / 0.225) ${lu(lyricEdge - 6)}, rgb(${s} / 0.5) ${lu(lyricEdge + 2)})`;

  // The missing-art note glyph (`note_glyph`), in art-local units.
  const g = art * 0.44;
  const c = art / 2;
  const hd = g * 0.34;
  const hx = c - g * 0.3;
  const hy = c + g * 0.3;
  const stem = Math.max(g * 0.1, 1);
  const note = [
    { x: hx - hd, y: hy - hd * 0.78, w: hd * 2, h: hd * 1.56, r: hd * 0.78 },
    { x: hx + hd - stem, y: c - g * 0.62, w: stem, h: g * 0.94, r: stem / 2 },
    {
      x: hx + hd - stem,
      y: c - g * 0.62,
      w: g * 0.42,
      h: stem * 1.6,
      r: Math.min(stem, stem * 0.8),
    },
  ].map((q) => ({ x: r3(q.x), y: r3(q.y), w: r3(q.w), h: r3(q.h), r: r3(q.r) }));

  return {
    L,
    title,
    body,
    art,
    artRadius: clamp(4 * Math.round((0.18 * art) / 4), 6, 20),
    badge: title * 1.55,
    badgeC: pad + art - 6,
    radius,
    w,
    h,
    pad,
    colX,
    colW: w - pad - colX,
    labelY,
    titleY,
    artistY,
    dividerY,
    lyricY,
    lyricLh: r3(leading(L)),
    nextY: lyricY + lyricCap + 8,
    scrim,
    note,
    left: 20,
    top: SCREEN_H - 20 - h,
  };
})();

/* ---- cards/visualizer.rs: the panel treatment, rainbow ------------------ */

/** The sweep `surface::hue_sweep` interpolates, red round to violet. */
const HUE_STOPS: [number, number, number, number][] = [
  [0, 0xff, 0x4d, 0x4d],
  [0.2, 0xff, 0xa2, 0x4d],
  [0.4, 0xf2, 0xe1, 0x4d],
  [0.6, 0x4d, 0xd9, 0x8c],
  [0.8, 0x4d, 0xa6, 0xff],
  [1, 0xa8, 0x4d, 0xff],
];
function hueSweep(f: number) {
  let i = 0;
  while (i < HUE_STOPS.length - 2 && f > HUE_STOPS[i + 1][0]) i++;
  const [a0, r0, g0, b0] = HUE_STOPS[i];
  const [a1, r1, g1, b1] = HUE_STOPS[i + 1];
  const t = (f - a0) / (a1 - a0);
  const m = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `${m(r0, r1)} ${m(g0, g1)} ${m(b0, b1)}`;
}

/* Band levels at rest, read off the recordings: loud lows, a mid bump, a
   falling top end. Four keyframes per bar swing each level by one of these
   factors; plain literals and products, so every engine agrees on them. */
const LEVELS = [
  0.8, 0.8, 0.73, 0.68, 0.6, 0.56, 0.6, 0.65, 0.64, 0.61, 0.62, 0.51, 0.55,
  0.59, 0.71, 0.68, 0.68, 0.64, 0.66, 0.6, 0.59, 0.62, 0.63, 0.59, 0.55, 0.49,
  0.51, 0.4, 0.41, 0.37, 0.33, 0.24,
];
const SWING = [1, 0.74, 1.12, 0.84, 1.06, 0.68, 0.93];
/** How far a peak cap falls per keyframe once its bar has dropped away. */
const PEAK_DECAY = 0.07;

export const VIZ = (() => {
  const w = Math.round(0.4 * SCREEN_W);
  const h = 120;
  const bed = 16;
  const area = { x: bed + 6, y: bed + 6, w: w - 2 * (bed + 6), h: h - 2 * (bed + 6) };
  const n = LEVELS.length;
  // surface::bar_geometry: two passes, the gap a third of the bar.
  let bw = area.w / n;
  let gap = 0;
  for (let k = 0; k < 2; k++) {
    gap = Math.min(clamp(Math.round(bw * 0.34), 2, 10), (area.w * 0.5) / (n - 1));
    bw = (area.w - (n - 1) * gap) / n;
  }
  const bars = LEVELS.map((base, i) => {
    const b = [0, 1, 2, 3].map((k) =>
      clamp(base * SWING[(i + 3 * k) % 7], 0.06, 0.9),
    );
    // Peak caps: snap up with the bar, fall back slowly (cyclic steady state).
    const c = [0, 0, 0, 0];
    let p = Math.max(...b);
    for (let pass = 0; pass < 2; pass++) {
      for (let k = 0; k < 4; k++) {
        p = Math.max(b[k], p - PEAK_DECAY);
        c[k] = p;
      }
    }
    const dur = 1.3 + 0.15 * ((i * 5) % 7);
    return {
      x: r3(i * (bw + gap)),
      color: `rgb(${hueSweep(i / (n - 1))} / 0.863)`,
      // Bars are full-height and slide down; caps are 2 lu and slide up by
      // their level times the area's height in cap heights.
      bar: b.map((v) => `${r3((1 - v) * 100)}%`),
      cap: c.map((v) => `${r3(-v * (area.h / 2) * 100)}%`),
      dur: `${r3(dur)}s`,
      delay: `${r3(-((i * 0.61) % dur))}s`,
    };
  });
  return {
    w,
    h,
    radius: 12,
    bed,
    bedRadius: Math.max(12 - bed, 4),
    area,
    bw: r3(bw),
    bars,
    left: SCREEN_W - 10 - w,
    top: SCREEN_H - 10 - h,
  };
})();

/* ---- cards/disc.rs: the record ------------------------------------------ */

export const DISC = (() => {
  const size = 220;
  const R = size / 2;
  const circle = (r: number) =>
    `M${r3(R - r)} ${R}a${r3(r)} ${r3(r)} 0 1 0 ${r3(2 * r)} 0a${r3(r)} ${r3(r)} 0 1 0 ${r3(-2 * r)} 0Z`;
  // Canvas angles run clockwise from 12 o'clock.
  const at = (deg: number, r: number) => {
    const a = (deg * Math.PI) / 180;
    return `${r3(R + r * Math.sin(a))} ${r3(R - r * Math.cos(a))}`;
  };
  const rim = R - 0.5;
  const labelR = 0.33 * R;
  const holeR = 0.045 * R;
  return {
    size,
    R,
    labelR: r3(labelR),
    holeR: r3(holeR),
    grooves: [0.42, 0.53, 0.64, 0.75, 0.86].map((g) => r3(g * R)),
    // The paper label, with the spindle hole cut through it.
    label: circle(labelR) + circle(holeR),
    bevelHi: `M${at(-60, rim)}A${rim} ${rim} 0 0 1 ${at(120, rim)}`,
    bevelLo: `M${at(120, rim)}A${rim} ${rim} 0 0 1 ${at(300, rim)}`,
    left: SCREEN_W - 48 - size,
    top: (SCREEN_H - size) / 2,
  };
})();

/* ---- clock.rs: the strings the clock shows ------------------------------ */

const WEEKDAYS = [
  "SUNDAY",
  "MONDAY",
  "TUESDAY",
  "WEDNESDAY",
  "THURSDAY",
  "FRIDAY",
  "SATURDAY",
];
const MONTHS = [
  "JANUARY",
  "FEBRUARY",
  "MARCH",
  "APRIL",
  "MAY",
  "JUNE",
  "JULY",
  "AUGUST",
  "SEPTEMBER",
  "OCTOBER",
  "NOVEMBER",
  "DECEMBER",
];
const pad2 = (n: number) => String(n).padStart(2, "0");

/** `12:13:20`: 24-hour with seconds, as on the recorded desktop. */
export const clockTime = (d: Date) =>
  `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;

/** The micro-label's two fields, `%A` and `%-d %B`, micro-cased. */
export const clockLabel = (d: Date) => ({
  weekday: WEEKDAYS[d.getDay()],
  date: `${d.getDate()} ${MONTHS[d.getMonth()]}`,
});

/** `clock::day_remaining`: "11h 47m left today", "47m left today", "3h left today". */
export function dayRemaining(d: Date) {
  const left = clamp(1440 - (d.getHours() * 60 + d.getMinutes()), 1, 1440);
  const h = Math.floor(left / 60);
  const m = left % 60;
  if (h === 0) return `${m}m left today`;
  if (m === 0) return `${h}h left today`;
  return `${h}h ${m}m left today`;
}

/** How many ring dots are lit: the fraction of the local day elapsed. */
export function litDots(d: Date) {
  const f =
    (d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds()) / 86400;
  return Math.min(Math.round(NOS.ring.length * f), NOS.ring.length);
}
