# Fresco Widget Visual Design Specification

Status: **design deliverable, 2026-08-20.** No code in this document. Target
implementation: `src/widgetkit/` (tiny-skia rasteriser → BGRA premultiplied
bitmaps, consumed by mpv `overlay-add` today and by Fresco's own surface at W2).

Scope: the four widgets Fresco actually has — **clock** (`src/clock.rs`),
**now-playing / synced lyrics** (`src/lyrics.rs`), **audio visualiser**
(`src/visualizer.rs`), **album-art disc** (`src/artwork.rs`) — dressed in one
system, in two themes.

Companions: [WIDGET_THEMES.md](WIDGET_THEMES.md) (the ASS-era plan, superseded
for anything widgetkit draws), [WIDGETS_ROADMAP.md](WIDGETS_ROADMAP.md).

> Every number in this document is concrete. Where a value is derived, the
> formula is given. Where a value was computed against a contrast target, the
> computation is shown. Nothing here says "generous" or "subtle".

---

## 0. The one hard limit, and the whole design's response to it

There is **no backdrop blur**. Blurring the wallpaper behind a card requires the
compositor, and only some compositors offer it. `src/clock.rs:806-838` already
states the house position and it stands unchanged:

> Real glassmorphism is a *backdrop* blur… What it does instead is the two parts
> of the look that *are* expressible: **translucency** and **edge lighting**.
> The blur that is missing is not only decorative: in real glassmorphism it is
> what keeps type legible when the backdrop is bright and busy.

widgetkit gains real gradients, real Gaussian shadows and real arcs, but it does
**not** gain a backdrop filter. So the legibility budget is spent on five
instruments instead, and every card below states which of them it uses:

| # | Instrument | What it buys |
|---|---|---|
| 1 | **Opacity budget** — the card's own alpha | Sets the floor on how much wallpaper luminance can reach the eye (§2.1) |
| 2 | **Scrim** — a feathered plate behind the *text block only* | Buys back the contrast the missing blur would have bought (§2.3) |
| 3 | **Well** — an inset panel behind *data graphics* | Same job as the scrim, for arcs, bars and progress fills, which also need contrast (§2.4) |
| 4 | **Shadow** — a real Gaussian drop shadow | Separates card from wallpaper where luminance alone cannot (§7) |
| 5 | **Hierarchy by size, weight and case** — not by opacity alone | Opacity-thinned ink is the first thing a bright wallpaper destroys (§2.5) |

Instrument 5 is the one the primary reference does *not* do, and it is the
single most important deviation in this document. See §11.

---

## 1. Units, scaling and rasterisation

### 1.1 The logical unit

**`lu`** — one logical unit is one device pixel on a 1920×1080 output at scale
factor 1.0. Every dimension in this spec is in `lu` unless it carries `em`, `%`
or `°`.

```
S      = clamp( round(output_height / 1080 × 8) / 8 , 0.75 , 4.0 )   // eighth steps
S_eff  = S × compositor_fractional_scale                              // wl_output / XRandR
device_px(x) = round(x × S_eff)
```

Eighth steps, not quarters: 1440p is `1.333…` and rounds to `1.375` (a 3.1%
error); quarter steps would round it to `1.25` (a 6.3% error, visible as a
too-small clock on the most common high-DPI desktop panel).

Worked: 1080p → S 1.000 · 1440p → S 1.375 · 4K → S 2.000 · 5K/6K → S 2.625/3.000.

### 1.2 Rasterisation rules

| Thing | Rule |
|---|---|
| Font size | `lu × S_eff`, rounded to the nearest **0.5 px**. Never to an integer — integer-only sizes make the type scale lurch between S steps. |
| Hairline stroke | `max(1, round(1 × S_eff))` device px, and the path is offset by `stroke_width / 2` so a 1 px stroke lands on a pixel centre instead of straddling two. |
| Filled rect edges | Snap the *outer* card rect to integer device px. Do not snap anything inside it: snapping the well and the scrim independently makes their gaps to the card edge differ by a pixel on one side. |
| Shadow blur | Not snapped. `sigma = blur / 2` (blur is quoted CSS-style throughout this document). |
| Text baselines | `round(y × S_eff)` — a fractional baseline costs vertical hinting and makes small labels mushy. Horizontal positions stay fractional. |
| Corner radius | `min(R, 0.5 × min(w, h))` always, then `round`. |

### 1.3 Output budget

Every widget renders to a **tight** BGRA buffer sized to its own bounds plus the
shadow bleed (§7.4), not to the screen. The buffer must be premultiplied and
row-tight (`stride = w × 4`), matching `artwork::Bgra` exactly.

---

## 2. The legibility model

This section derives the token values in §3. It is the reason those numbers are
what they are, and an implementer changing any alpha must redo it.

### 2.1 The compositing model

A card is a translucent fill over an arbitrary photo. For a card fill `C` at
alpha `a` over wallpaper `W`:

```
surface = a·C + (1−a)·W
```

Text at alpha `t` over that surface composites again:

```
ink = t·I + (1−t)·surface
```

and WCAG contrast is computed between `ink` and `surface` — **not** between the
ink token and the card token. The worst case is not "a dark wallpaper"; it is
whichever of pure white or pure black pushes the surface *toward* the ink.

Every ratio in §4 was computed this way over both extremes, and over both ends
of the card's gradient (the lightest gradient stop is the worst case in dark
mode; the darkest stop is the worst case in light mode).

### 2.2 Why the card gradient carries no alpha variation

The card fill is a linear gradient, but **alpha is constant across it**. Only
the colour varies. A gradient that also varies alpha would make the contrast a
function of position within the card, which is unverifiable and un-specifiable.
Constant alpha means two computations — lightest stop, darkest stop — bracket
the whole surface. This is a deliberate restriction on the toolkit's capability.

### 2.3 The scrim law (dark mode)

Solve for the ink alpha that reaches a target ratio on the worst dark surface
(card `#171B24 @ 0.72` over a **white** wallpaper, composited luminance
L = 0.1040):

| Target | Ink alpha needed, **no scrim** | Ink alpha needed, **with scrim** |
|---|---|---|
| 4.5:1 (AA body) | **0.725** | **0.497** |
| 7.0:1 (AAA body) | **unreachable at any alpha** | **0.681** |

That table is the entire argument for the scrim. Without it, a dark Fresco card
over a bright wallpaper supports **exactly one** legible ink level (white at
α ≥ 0.73, giving 4.7:1) and no AAA text at all — the three-tier
micro-label / hero / secondary hierarchy the primary reference is built on is
arithmetically impossible. With a scrim at `#04060A @ 0.50`, the surface drops to
L = 0.0299 and the full three-tier ramp (1.00 / 0.70 / 0.52) clears
13.1 / 7.3 / 4.8:1.

**The scrim is therefore mandatory behind every text block on every dark card.**
It is not a style option and it is not conditional on the wallpaper — the
renderer does not get to see the wallpaper.

### 2.4 Light mode is not an inversion

Two findings force light mode to be designed separately, not derived by flipping
the dark tokens.

**(a) The ink-alpha floor is nearly independent of everything.** Solving for the
alpha of `#0D1016` ink that reaches 4.5:1 on a light card, across card alphas
0.86–0.92 and gradient ends `#EEF1F6`–`#F6F7FA`, over a black wallpaper:

```
alpha for 4.5:1  ∈ [0.593, 0.618]      alpha for 7:1  ∈ [0.728, 0.765]
```

A 6-point swing in card alpha moves the required ink alpha by **0.018**. So in
light mode, raising the card's opacity does almost nothing for text contrast.
The reason is the sRGB transfer curve: at the bright end it is nearly linear, so
thinning dark ink over a light surface loses contrast at a fixed rate no matter
what is behind it. At the dark end the curve is steeply compressive, which is
why the dark card's scrim is so effective.

Consequence: **light mode's ink ramp is compressed** — 1.00 / 0.78 / 0.64,
against dark mode's 1.00 / 0.70 / 0.52. The reference's "secondary at ~60%
opacity" is legal in dark mode over a scrim and **illegal in light mode**:
0.60 gives **4.47:1** on the worst light surface — a fail by 0.03, before any
wallpaper texture is added to it.

**(b) The light card must be more opaque anyway, for a different reason.**
Contrast does not require it; **texture suppression** does. The fraction of the
wallpaper's luminance reaching the eye is `1 − a`:

| Surface | Wallpaper leak |
|---|---|
| Dark card (a 0.72) | 28% |
| Dark card + scrim | 14.0% |
| Dark well | 12.6% |
| Light card (a 0.90) | 10% |
| Light card + scrim | 4.5% |

A busy photo with ±100 of local luminance swing shows through a dark card as a
±28 mottle under the type. That mottle is what makes text *feel* unreadable even
when the mean contrast passes. Dark mode pays for the mottle with the scrim;
light mode pays for it by being more opaque up front, because a light card also
has to hide *colour* leaking through, and a coloured mottle under dark ink is
worse than a grey one. Hence **dark 0.72 / light 0.90**, and the light scrim's
job is texture, not contrast.

**(c) The well inverts direction between themes.** In dark mode an inset well is
*darker* than the card. In light mode a darker well is a trap: it drives the
track toward mid-grey (L 0.52 over a black wallpaper), and mid-grey is the worst
possible backdrop for a saturated accent fill — Green, Teal, Amber and Coral all
fall to 1.95–2.78:1 there, failing the 3:1 non-text minimum. So:

> **In light mode the well is *lighter* than the card, not darker.** It reads as
> inset because of its bevel (dark hairline at the top inner edge, bright
> hairline at the bottom), which is the Reference-B treatment, not because of
> its fill. With a `#FFFFFF @ 0.55` well the same accents clear 4.78–7.46:1.

The token stays structurally identical — `well = surface pushed away from the
ink` — so one layout renders in both themes. Only the direction changes.

### 2.5 Hierarchy is carried by size, weight and case

Restating `src/clock.rs:855-857` because it generalises:

> A translucent card can have a bright wallpaper behind it, and faded white over
> that is the first thing to become unreadable.

Wherever a card cannot carry a scrim (the bare visualiser, §9.3.1; the disc
label at reduced opacity, §9.4), the ink ramp collapses to **two** levels and
hierarchy is expressed entirely by type size, weight and letter-case. Never by a
third opacity step.

---

## 3. Design tokens

Two themes. Structurally identical: same token names, same layout, same
component geometry. Only values change.

### 3.1 Dark theme

| Token | Value | Notes |
|---|---|---|
| `surface.card` | linear gradient `#171B24` → `#0A0C11`, **α 0.72 constant**, 160° (top-left → bottom-right) | The glass. 28% wallpaper leak. Near-black, never `#000000`: a pure-black panel reads as a hole punched in the wallpaper. |
| `surface.elevated` | `#1E2430 @ 0.78` | A card stacked on a card (rare — the tinted now-playing header, §9.2). One step lighter *and* one step more opaque. |
| `surface.scrim` | `#04060A @ 0.50` over the card | Behind text blocks only. Feather 6 lu. |
| `surface.well` | `#04060A @ 0.55` over the card | Inset panel: visualiser bed, progress track, LCD. |
| `edge.hairline` | `#FFFFFF @ 0.14`, 1 lu, full perimeter | 1.41–1.49:1 against the card — a material cue, not a boundary. The boundary is the shadow. |
| `edge.highlight` | `#FFFFFF @ 0.34 → 0.00`, 1 lu, top arc only | Linear-gradient stroke, full at 12 o'clock, zero at the horizontal midline. Reads as a light source above the card. |
| `edge.wellTop` | `#000000 @ 0.55`, 1 lu inner, blur 3, offset y +1 | The "pressed in" top of a well. |
| `edge.wellBottom` | `#FFFFFF @ 0.06`, 1 lu inner, offset y −1, no blur | The lift at the bottom of a well. |
| `text.primary` | `#FFFFFF @ 1.00` | |
| `text.secondary` | `#FFFFFF @ 0.70` | Scrim required. |
| `text.tertiary` | `#FFFFFF @ 0.52` | Scrim required. Micro-labels, axis labels. |
| `text.onAccent` | `#0A0C11 @ 1.00` | Text sitting *inside* an accent fill (chips). |
| `accent.ink` | `mix(accent, #FFFFFF, 0.32)` | Accent-coloured **text**. |
| `accent.fill` | `mix(accent, #FFFFFF, 0.14)` | Accent-coloured **graphics** (arcs, bars, progress). |
| `accent.dim` | `accent @ 0.28` | Track tint, gradient far end, gauge remainder. |
| `shadow.colour` | `#000000` | |
| `data.gridline` | `#FFFFFF @ 0.09`, 1 lu | Chart baselines, tick marks. |
| `data.trackEmpty` | `#FFFFFF @ 0.11` | Unfilled part of a progress bar, over the well. |

**Derived accent values** (Fresco's six accents, dark variants from
`daemon::accent_hex`):

| Accent | `accent` | `accent.ink` | `accent.fill` |
|---|---|---|---|
| Blue | `#5E6AD2` | `#929AE0` | `#757FD8` |
| Teal | `#2BB6A2` | `#6FCDC0` | `#49C0AF` |
| Green | `#46B96B` | `#81CF9A` | `#60C380` |
| Amber | `#DBA13C` | `#E7BF7A` | `#E0AE57` |
| Coral | `#F0708A` | `#F59EAF` | `#F2849A` |
| Graphite | `#98A1B0` | `#B9BFC9` | `#A6AEBB` |

`accent.ink` exists because the raw accents are *background* colours, tuned in
`gui::theme::accent_pair` to sit behind white text. Used as ink they fail:
Blue at `#5E6AD2` gives **2.80:1** on the worst scrimmed dark surface. Lifting
32% toward white gives **4.94:1** and lifts the other five to 6.5–7.6:1.

### 3.2 Light theme

| Token | Value | Notes |
|---|---|---|
| `surface.card` | linear gradient `#FFFFFF` → `#F2F4F8`, **α 0.90 constant**, 160° | 10% wallpaper leak. More opaque than dark by design — §2.4(b). |
| `surface.elevated` | `#FFFFFF @ 0.96` | |
| `surface.scrim` | `#FFFFFF @ 0.55` over the card | Feather 8 lu (wider than dark: a bright feather against a bright card needs more distance to disappear). |
| `surface.well` | `#FFFFFF @ 0.55` over the card | **Lighter than the card, not darker** — §2.4(c). |
| `edge.hairline` | `#0B0E14 @ 0.22`, 1 lu, full perimeter | 1.61–1.64:1. Dark, not white: a white hairline on a white card over a bright photo is invisible. |
| `edge.highlight` | `#FFFFFF @ 0.95 → 0.00`, 1 lu, top arc only | Still a bright top edge — the light source does not move between themes. |
| `edge.wellTop` | `#0B0E14 @ 0.18`, 1 lu inner, blur 3, offset y +1 | |
| `edge.wellBottom` | `#FFFFFF @ 0.90`, 1 lu inner, offset y −1, no blur | |
| `text.primary` | `#0D1016 @ 1.00` | Blue-black, not `#000000`. |
| `text.secondary` | `#0D1016 @ 0.78` | Not 0.70 — see §2.4(a). |
| `text.tertiary` | `#0D1016 @ 0.64` | Not 0.52. 0.60 fails. |
| `text.onAccent` | `#FFFFFF @ 1.00` | |
| `accent.ink` | `mix(accent_light, #000000, 0.22)` | Built from `gui::theme::accent_pair(_, false)`, **not** from the dark accent. |
| `accent.fill` | `= accent.ink` | Light mode does **not** get a separate lighter fill: on a light well, a lighter fill fails 3:1. One value, two roles. |
| `accent.dim` | `accent_light @ 0.22` | |
| `shadow.colour` | `#0B1220` | A cool near-black, not pure black. Pure black under a white card on a colourful photo reads as dirt rather than as shade. |
| `data.gridline` | `#0B0E14 @ 0.14`, 1 lu | |
| `data.trackEmpty` | `#0B0E14 @ 0.13` | |

| Accent | `accent_light` | `accent.ink` = `accent.fill` |
|---|---|---|
| Blue | `#5058C4` | `#3E4599` |
| Teal | `#0E8C7E` | `#0B6D62` |
| Green | `#2C9A4C` | `#22783B` |
| Amber | `#AE7820` | `#885E19` |
| Coral | `#DE4567` | `#AD3650` |
| Graphite | `#5B626F` | `#474C57` |

### 3.3 Accent-tinting a card from album art (the "weather card" adaptation)

Reference A proves the system survives one tinted card in a dark set. Fresco's
equivalent is the now-playing card picking up the album cover's hue. The rule
that keeps the contrast tables valid:

> The artwork may modify the card gradient's **hue and chroma only**. Convert the
> token stop to OKLab, replace `(a, b)` with the artwork's dominant `(a, b)`
> scaled so that `hypot(a, b) ≤ 0.06`, and **keep `L` from the token, unchanged**.

Because the composited luminance is unchanged by construction, every ratio in §4
holds for a tinted card without recomputation. Alpha is not touched either. If
the artwork is missing or its dominant chroma is below 0.01, the untinted token
is used and nothing else changes.

---

## 4. Contrast numbers

All figures are WCAG 2.x contrast ratios between the composited ink and the
composited surface underneath it, over the two worst-case wallpapers, at both
ends of the card gradient. **Bold** = the governing worst case.

### 4.1 Dark theme

| Surface state | Wallpaper | Grad. stop | Surface L | primary 1.00 | secondary 0.70 | tertiary 0.52 |
|---|---|---|---|---|---|---|
| card only | white | light | 0.1040 | **6.82** | **4.32 ✗** | **3.15 ✗** |
| card only | white | dark | 0.0802 | 8.06 | 4.94 | 3.51 ✗ |
| card only | black | light | 0.0067 | 18.52 | 9.36 | 5.64 |
| card only | black | dark | 0.0026 | 19.96 | 9.74 | 5.68 |
| **card + scrim** | **white** | **light** | **0.0299** | **13.14** | **7.30** | **4.77** |
| card + scrim | white | dark | 0.0241 | 14.17 | 7.74 | 4.99 |
| card + scrim | black | light | 0.0039 | 19.48 | 9.63 | 5.69 |
| card + scrim | black | dark | 0.0022 | 20.12 | 9.77 | 5.68 |

**Scrim required for `text.secondary` and `text.tertiary`. Always.** Primary
alone passes AA unscrimmed (6.82:1) but not AAA (7:1) — so even a hero-only card
takes the scrim if it wants AAA, which the clock hero does.

### 4.2 Light theme

| Surface state | Wallpaper | Grad. stop | Surface L | primary 1.00 | secondary 0.78 | tertiary 0.64 |
|---|---|---|---|---|---|---|
| card only | white | light | 1.0000 | 19.04 | 9.67 | 5.75 |
| card only | white | dark | 0.9130 | 17.46 | 9.18 | 5.57 |
| card only | black | light | 0.7870 | 15.19 | 8.42 | 5.27 |
| **card only** | **black** | **dark** | **0.7119** | **13.82** | **7.93** | **5.07** |
| card + scrim | black | light | 0.9010 | 17.24 | 9.11 | 5.54 |
| card + scrim | black | dark | 0.8630 | 16.56 | 8.89 | 5.46 |

**Light mode passes AA at every level with no scrim** (worst 5.07:1). The light
scrim is therefore **optional for contrast and required for texture** — it is
drawn wherever the type is below 18 lu, where a 10% wallpaper mottle starts to
break glyph counters, and skipped behind a large hero where it would only flatten
the card.

### 4.3 Accent text and accent graphics

Accent **text** always sits inside a scrimmed text block. Ratios on the governing
worst case of each theme:

| Accent | dark `accent.ink` on dark scrim (L 0.0299) | dark, best case | light `accent.ink` on light scrim (L 0.8630) | light, on pure white |
|---|---|---|---|---|
| Blue | **4.94** | 7.56 | **6.83** | 7.86 |
| Teal | 7.01 | 10.73 | 5.00 | 5.75 |
| Green | 7.11 | 10.89 | **4.78** | 5.49 |
| Amber | 7.59 | 11.61 | 5.01 | 5.76 |
| Coral | 6.49 | 9.94 | 5.34 | 6.14 |
| Graphite | 7.11 | 10.89 | 7.46 | 8.57 |

Minimum 4.78:1 — all six accents clear AA for body text in both themes.

Accent **graphics** need 3:1 (WCAG non-text). They always sit on a **well**:

| Accent | dark `accent.fill` on dark well, worst / best | light `accent.fill` on light well, worst / best |
|---|---|---|
| Blue | **3.84** / 5.53 | 7.28 / 8.37 |
| Teal | 6.28 / 9.07 | 5.39 / 6.20 |
| Green | 6.37 / 9.20 | **4.78** / 5.49 |
| Amber | 6.89 / 9.95 | 5.01 / 5.76 |
| Coral | 5.68 / 8.20 | 5.34 / 6.14 |
| Graphite | 6.25 / 9.03 | 7.46 / 8.57 |

Minimum 3.84:1.

**Rule: an accent-filled data graphic never sits directly on the card.** On the
worst dark card the raw accents give only 1.45–2.98:1 — every one of them fails.
The well is not decoration; it is what makes the progress bar visible.

### 4.4 Scrim sizing

The scrim must cover the text block's **ink extent plus a margin big enough that
the feather has finished before the glyphs start**.

```
scrim_rect = union(all glyph bounding boxes in the block)
             inflated by:
                 dx = max(8, 0.55 × cap_height(largest row))
                 dy = max(6, 0.35 × line_height(largest row))
feather    = 6 lu (dark) / 8 lu (light)
radius     = max(8, R_card − (pad − dx))          // see §6.3 nesting
clamp      = never exceeds the card's inner rect (card rect deflated by 2 lu)
```

Worked, Standard clock card (§9.1, hero 64 lu, cap height 46.5, line-height 60):
`dx = max(8, 25.6) = 26`, `dy = max(6, 21) = 21`. The three-row block measures
167 × 92 lu, so the scrim is 219 × 134 — which clamps to the card's inner rect of
203 × 128. **In practice the clock card's scrim fills the card**, and the
translucency the design is paying for shows only in the 2 lu ring outside it plus
the feather. That is the honest cost of the missing blur on a text-only card, and
it is why the visualiser and disc — which have large text-free areas — are where
the glass actually reads.

### 4.5 Degradation when no scrim is possible

If a widget is drawn without a card (the bare visualiser, §9.3.1), the ink ramp
collapses:

| Dark, no card, no scrim, over a white wallpaper | Available ink |
|---|---|
| 4.5:1 | `#FFFFFF @ 0.75` on a bottom scrim gradient, or a 1.5 lu `#000000 @ 0.55` outline |
| 7:1 | not reachable by alpha; requires the outline |

The fallback is a **text outline** (`bord` equivalent): stroke width
`max(1.5, 0.055 × size)` lu in `#000000 @ 0.55` (dark theme) or
`#FFFFFF @ 0.70` (light theme), drawn under the fill. This is what
`LyricStylePreset::Subtitle` already does and it is the only treatment that works
with no surface at all. Never thicker than 6% of the type size — beyond that it
closes Inter's counters.

---

## 5. Type

### 5.1 The scale

Modular, ratio **1.25** (major third), anchored at 14 lu. Sizes in `lu` at S = 1.

| Step | Size | Weight (Latin) | Tracking | Line-height | Role |
|---|---|---|---|---|---|
| `micro` | 11 | 600 | **+0.128 em** | 1.62 | UPPERCASE micro-label |
| `caption` | 11 | 500 | +0.028 em | 1.62 | tertiary caption, axis labels, times |
| `body` | 14 | 500 | +0.016 em | 1.52 | secondary supporting line |
| `title` | 18 | 600 | +0.006 em | 1.41 | track title |
| `lead` | 22 | 500 | 0.000 em | 1.32 | lyric line, small card |
| `lead-lg` | 27 | 500 | −0.006 em | 1.23 | lyric line, large card |
| `hero-s` | 34 | 600 | −0.010 em | 1.13 | |
| `hero-m` | 43 | 650 | −0.014 em | 1.03 | |
| `hero-l` | 53 | 700 | −0.017 em | 0.94 | |
| `hero-xl` | 67 | 700 | −0.019 em | 0.94 | default clock hero |
| `hero-2xl` | 84 | 700 | −0.021 em | 0.94 | |

Tracking and line-height are **generated**, not hand-picked, so a user-set size
between two steps still gets the right values (Apple's rule: tracking is
size-specific, never one value for all sizes; leading tracks size inversely):

```
tracking_em(s)   = clamp(-0.0285 + 0.62 / s, -0.030, +0.140)
                   + 0.100 if the run is an UPPERCASE micro-label
line_height(s)   = clamp(1.62 - 0.30 · log2(s / 11), 0.94, 1.62)
```

Weight is *not* generated. It steps 500 → 600 → 650 → 700 at 14 / 18 / 43 / 53 lu.

**Hierarchy budget.** Between any two adjacent rows in a block, at least two of
{size, weight, case} must differ. Between the micro-label and the hero all three
differ; between the hero and the secondary line, size and weight differ. Opacity
is never the *only* difference — §2.5.

### 5.2 Font families

Fresco ships a Simplified-Chinese UI (`i18n.rs`, `zh-CN` catalogs), so CJK is a
first-class requirement, not a fallback.

**Latin, numerals, symbols** — request in order:
```
"Inter" → "Inter Variable" → "Noto Sans" → "DejaVu Sans" → sans-serif
```
`Inter` matches what `clock.rs` and `lyrics.rs` already request. Never decorate
the family with a face name (`"Inter SemiBold"`): fontconfig substitutes silently
on a missing family name but resolves a *weight* to the nearest real face inside
the family asked for — the reasoning is already written out at
`src/lyrics.rs:452-465` and applies unchanged here.

**Simplified Chinese (zh-Hans)** — request in order:
```
"Noto Sans SC" → "Source Han Sans SC" → "Noto Sans CJK SC"
  → "WenQuanYi Zen Hei" → "Microsoft YaHei" → sans-serif
```
(`Microsoft YaHei` is last because it is present on dual-boot and Wine-heavy
machines, which the Deepin/China userbase over-indexes on — see
[CHINA_DISTRIBUTION.md](CHINA_DISTRIBUTION.md).)

**Monospace / LCD** — Chassis theme readouts, bitrate chips, tabular columns:
```
"JetBrains Mono" → "DejaVu Sans Mono" → "Liberation Mono" → monospace
```

**Emoji**: never requested. A codepoint with no glyph in any resolved face is
**stripped at measure time**, not drawn as `.notdef`. A tofu box in a track title
looks like a rendering fault.

### 5.3 CJK adjustments

CJK is not "the same layout with different glyphs". Four concrete deltas:

| Property | Latin | CJK (Han / Kana / Hangul) |
|---|---|---|
| Weight request | 500 / 600 / 650 / 700 | **500 or 700 only.** Noto Sans SC ships 100/300/400/500/700/900; a request for 600 or 650 is synthesised by the rasteriser and smears at ≤ 18 lu. Map 600 and 650 → 700. |
| Micro-label case | `to_uppercase()`, locale-aware (Turkish dotted i) | **No case transform.** Han has no case. Emphasis comes from weight 700 instead. |
| Micro-label tracking | +0.128 em | **+0.040 em.** Han glyphs are already full-width and squared; +0.128 em breaks a two-character word into two unrelated characters. |
| Line-height | `line_height(s)` | `line_height(s) × 1.18`, floor **1.20**. Taller ascenders and descenders; the Latin 0.94 hero leading collides. |

Script detection is per **run**, not per string: a mixed
"Blue Monday — 蓝色星期一" line splits into a Latin run and a Han run and each
takes its own weight, tracking and vertical metrics. The line's line-height is
the maximum across its runs.

### 5.4 Numerals

Every clock, timer, duration, bitrate and percentage uses **tabular figures**.

- Request OpenType `tnum` on (Inter, Noto Sans SC and JetBrains Mono all ship it).
- Disable `calt` in micro-labels — contextual alternates in a +0.128 em uppercase
  run produce inconsistent spacing.
- **Fallback tabularisation**, if the resolved face has no `tnum`: measure the
  advance of `0`–`9` in that face, take the maximum, and pad every digit's advance
  to it, centring the glyph in its cell. A measure-before-draw rasteriser can do
  this; without it a clock jitters horizontally every time a `1` appears.

### 5.5 Overflow

Three behaviours, chosen per field, never guessed:

| Behaviour | Where used |
|---|---|
| **Ellipsis, one line** (`…`, U+2026, not three periods) | track title, artist, chip labels, disc label |
| **Wrap to N lines, then ellipsis** | lyric line (N = 2), secondary supporting line (N = 1, so effectively ellipsis) |
| **Shrink-to-fit**, in 0.5 lu steps down to 80% of the nominal size, then ellipsis | the clock hero only, and only when a user-chosen `font_size_pt` overflows the screen — matching the existing `CARD_FIT = 0.94` clamp |

Ellipsis is measured, not estimated: cut at the last grapheme cluster whose
advance plus the ellipsis advance fits. **Grapheme cluster**, not `char` — cutting
a Han string mid-codepoint, or splitting an emoji ZWJ sequence, is a correctness
bug not a layout one.

---

## 6. Space and shape

### 6.1 Spacing

Base unit **b = 4 lu**. Ladder: `4 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64 · 80`.

| Token | lu | Use |
|---|---|---|
| `gap.hairline` | 1 | stroke widths, tick marks |
| `gap.xs` | 4 | glyph ↔ its label; bar ↔ bar |
| `gap.s` | 8 | inside a row group |
| `gap.m` | 12 | label ↔ value |
| `gap.l` | 16 | between row groups inside a card |
| `gap.xl` | 24 | between major blocks; column gutter |
| `gap.2xl` | 32 | between two cards |

**Grouping rule.** The gap *inside* a group must be at most half the gap
*between* groups. This is why the clock's micro→hero leading (14) and
hero→secondary leading (12) are both far below the block→dial gap (36): the eye
groups the three rows and reads the dial separately. `clock.rs` already encodes
exactly this ratio (`CARD_LEAD_DAY 0.28`, `CARD_LEAD_DATE 0.13`,
`CARD_GAP_FACE 0.56`); it is promoted here to a system rule.

### 6.2 Card padding

```
pad = clamp( 4 · round( (0.055 · min(w, h) + 8) / 4 ), 12, 28 )
```

| min(w,h) | pad |
|---|---|
| 120 | 16 |
| 200 | 20 |
| 320 | 24 |
| ≥ 400 | 28 |

Padding is **uniform on all four sides**, and it is measured **optically** — to
the *cap top* of the first row and the *baseline plus descender* of the last row,
not to the em box. Inter's ascent runs ≈ 0.25 em above its capitals; without the
correction a 64 lu hero acquires 16 lu of invisible dead band above it and the
card looks bottom-heavy. Constants:

```
CAP_GAP   = 0.25 em    // em-box top → cap top  (Inter)
CAP_H     = 0.727 em   // cap height            (Inter)
DESC      = 0.100 em   // baseline → visual descender bottom, for a row with descenders
```

For CJK runs, `CAP_GAP = 0.12 em`, `CAP_H = 0.86 em` (ideographic face), `DESC = 0.06 em`.

### 6.3 Radii

```
R_card = clamp( 4 · round( (0.42 · H_max) / 4 ), 12, 32 )        capped at 0.5 · min(w, h)
```
where `H_max` is the largest type size on the card. Tying the radius to the type,
not to the box, is what makes a wide short clock strip and a tall lyric card look
like members of one family. Worked: hero 64 → 28 · lyric 27 → 12 · title 18 → 8 → clamps to 12.

| Element | Radius |
|---|---|
| Card | `R_card` |
| Well / inset panel | `max(4, R_card − pad)` |
| Scrim | `max(8, R_card − gap_edge)`, `gap_edge` = distance from card edge to scrim edge |
| Album art thumbnail | `clamp(4 · round(0.18 · side / 4), 6, 20)` |
| Chip / pill | `height / 2` |
| Progress track and fill | `height / 2` |
| Visualiser bar | `min(bar_width / 2, 3)` when `Visualizer::rounded`, else 0 |
| Badge | circle |

**Nesting rule.** For a shape inset by distance `d` inside a parent of radius
`r_outer`:
```
r_inner = max(4, r_outer − d)
```
This keeps the two curves **concentric** — the gap between them stays `d` all the
way round the corner. Using the same radius for both makes the inner corner look
too round; using an unrelated value makes the gap pinch at 45°. `clock.rs` already
does this for the scrim ("reduced by its own inset, so the curves stay
concentric").

**Concentric strokes.** A stroke of width `t` intended to sit flush with the edge
of a shape of radius `r` is drawn on a path of radius `r − t/2`.

---

## 7. Elevation

Four levels. Shadows are real Gaussians on the shape's alpha, `sigma = blur / 2`.
`x/y/blur` in lu; alpha as stated. Two-part shadows (key + contact) are drawn
key-first.

| Level | Use | Dark | Light |
|---|---|---|---|
| **E0** | anything flush inside a card | none | none |
| **E1** | chip, badge, bar peak-cap, disc label | `0 / 1 / 3 @ 0.28` | `0 / 1 / 3 @ 0.20` |
| **E2** | a card on the wallpaper | key `0 / 10 / 28 @ 0.46` + contact `0 / 1 / 2 @ 0.34` | key `0 / 12 / 32 @ 0.34` + contact `0 / 1 / 2 @ 0.26` |
| **E3** | the album disc; a card that overlaps another | key `0 / 16 / 40 @ 0.52` + contact `0 / 2 / 4 @ 0.36` | key `0 / 18 / 44 @ 0.40` + contact `0 / 2 / 4 @ 0.28` |

**Light shadows are larger and softer than dark ones at the same level, and use
`#0B1220` rather than `#000000`.** Two reasons, both consequences of §0: a white
card on a bright photo has no luminance step at its edge, so the shadow is doing
*all* of the separation work that a backdrop blur would otherwise share; and a
pure-black shadow under a white card over a saturated photo reads as grime,
whereas a blue-black reads as shade.

**Inner shadow** (one level, for wells) is specified in §3 as
`edge.wellTop` / `edge.wellBottom`.

### 7.4 Shadow bleed and buffer sizing

The output buffer must be inflated so the shadow is not clipped:

```
bleed = ceil( blur_key × 1.5 + |offset_y_key| ) lu
buffer = card_rect inflated by bleed on all sides
```
E2 dark → 42 + 10 = **52 lu**; E3 light → 66 + 18 = **84 lu**. At S = 2 (4K) that
is 168 device px of margin around the disc — budget for it, and remember the
widget's *anchor* margin (`margin_px`) is measured to the **card rect**, not to
the buffer edge, or every widget will appear to drift inward at high DPI.

---

## 8. Components

### 8.1 Inset panel (well)

The workhorse. Everything data-bearing sits in one.

```
┌────────────────────────────────────────┐  ← card
│  ╭──────────────────────────────────╮  │
│  │▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│  │  edge.wellTop  (1 lu, blur 3, y+1)
│  │                                  │  │  surface.well
│  │            content               │  │
│  │                                  │  │
│  │__________________________________│  │  edge.wellBottom (1 lu, y−1)
│  ╰──────────────────────────────────╯  │
└────────────────────────────────────────┘
```

- Fill: `surface.well` (dark: darker than card; light: lighter than card).
- Radius: `max(4, R_card − pad)`.
- The two inner edges are what carry "inset". They are drawn **inside** the well's
  clip, so they follow its corners.
- Minimum height 16 lu — below that the two 1 lu edges plus the blur eat the fill
  and it reads as a smudge.

### 8.2 Progress bar

```
   ▐███████████████████●░░░░░░░░░░░░░░░░░░░░░░░░▌
   ↑                    ↑                       ↑
   well (track)         knob                    end
```

| Property | Value |
|---|---|
| Track height | `h = clamp(round(0.14 · title_size), 4, 10)` lu — 18 lu title → 4 lu; 27 lu lyric → 4; a hero-sized bar → up to 10 |
| Track | `surface.well` + `data.trackEmpty` fill, radius `h/2` |
| Fill | `accent.fill`, radius `h/2`, min visible width `h` (so 0.4% progress is still a dot, not nothing) |
| Knob | optional; circle r = `h × 0.9`, `accent.fill`, E1 shadow, only drawn when `h ≥ 6` |
| Marker (the reference's "check at the current position") | **not adopted** — see §11 |
| Determinate? | If MPRIS gives no position, the whole bar is **hidden**, not drawn at 0. A bar pinned at zero says "this track is stuck", which is a lie. |
| Indeterminate | Not used. Fresco always either knows the position or has nothing to say. |

### 8.3 Arc gauge

Reference A's semicircular gauge with a dot at the value. Adopted for the clock's
Expanded variant (day progress) and available for volume.

```
        ····─────────────····
     ··                      ··
   ·                            ●        ← value dot
  ·                              ·
 |                                |
 start (−210°)              end (+30°)
```

| Property | Value |
|---|---|
| Sweep | 240°, from −210° to +30° (screen coords, 0° = 3 o'clock, CW positive) |
| Outer radius | `r` (caller-set) |
| Stroke width | `w = clamp(round(0.085 · r), 3, 14)` lu, round caps |
| Remainder arc | `accent.dim`, or `data.trackEmpty` when the gauge is not accent-coded |
| Value arc | `accent.fill`, with an optional linear gradient from `accent.dim` at the start to `accent.fill` at the value — the gradient runs along the chord, not around the arc (tiny-skia has no conic gradient; a chord-aligned linear gradient over a 240° sweep is visually indistinguishable and is honest about the substrate) |
| Value dot | circle r = `w × 0.85`, `accent.fill`, E1, centred on the arc at the value angle |
| Track well | a `surface.well` ring under the whole sweep, width `w + 4`, so the gauge clears 3:1 (§4.3) |
| Curved text along the arc | **not adopted** — see §11 |
| Centre label | `hero-s` primary + `micro` tertiary below it, both centred, both inside the gauge's own scrim |
| Minimum radius | 28 lu. Below that, degrade to a linear progress bar. |

### 8.4 Bar array (visualiser, spectrum, and any histogram)

```
 ▁▂▃▅▆█▇▅▃▂▁▂▄▆█▇▆▄▂▁     ← bars, bottom-aligned
 ────────────────────      ← baseline, data.gridline
```

| Property | Value |
|---|---|
| Band count | `Visualizer::bands`, clamped 8…160. Above 160 the bar is thinner than the gap (the renderer already folds >200; 160 is the optical limit at 1080p). |
| Bar width | `bw = (W − (n−1)·g) / n`, where `g = clamp(round(bw · 0.34), 2, 10)` solved iteratively (two passes converge) |
| Bar radius | `min(bw/2, 3)` when `rounded` |
| Fill | `accent.fill`; with `GradientMode::Linear`, a linear gradient across the array from `accent.fill` to `mix(colour_end, white/black, same t as accent.fill)`; with `GradientMode::Vertical`, per-bar vertical gradient from `accent.dim` at the base to `accent.fill` at the cap |
| Peak cap | 2 lu tall, `text.primary` at α 0.55, falls at 0.9 lu/frame after holding 380 ms. Optional; on by default. |
| Baseline | 1 lu `data.gridline` across the full width, always drawn — it is what makes a silent spectrum read as "silent" rather than "broken" |
| Minimum bar height | 2 lu, so silence shows a row of dots rather than nothing |
| Axis labels | `caption` / tertiary, 8 lu below the baseline. Fresco draws **no** axis labels by default (§11). |

### 8.5 Chip

```
 ╭──────────────╮
 │  FLAC · 44.1 │      pill
 ╰──────────────╯
```

| Property | Value |
|---|---|
| Height | `2 × micro_size` → 22 lu at micro 11 |
| Radius | `height / 2` |
| Horizontal padding | `height × 0.45` → 10 lu |
| Fill (neutral) | `surface.well` + 1 lu `edge.hairline` |
| Fill (accent) | `accent.fill`, text `text.onAccent`, no hairline |
| Text | `micro` step, uppercase (Latin), `accent.ink` on a neutral chip |
| Elevation | E1 |
| Overflow | ellipsis; a chip never wraps and never shrinks its type |

### 8.6 Circular badge

Used for the source-app icon on the now-playing card, and for overlapping stacks.

| Property | Value |
|---|---|
| Diameter | `d = title_size × 1.55` → 28 lu at title 18 |
| Fill | the icon's own pixels, clipped to a circle with 1 lu of AA feather |
| Ring | 1 lu `edge.hairline` on the outside, drawn at radius `d/2 − 0.5` |
| Elevation | E1 |
| Overlap (a stack) | each subsequent badge is offset by `−0.36 d` on x and is drawn with a `d/2 + 2` circular **cutout** punched in the badge beneath it, so the stack reads as separated without a heavy outline |
| Missing icon | a `surface.well` circle carrying the app name's first grapheme in `micro` / `text.secondary`, centred |

### 8.7 Bevelled readout bezel (Chassis theme only)

Reference B's bevelled square button, **repurposed as a non-interactive bezel**
— see §9.3.3 for why the buttons themselves are not drawn.

| Layer | Value |
|---|---|
| Body | `#2B2B2B`, radius `0.22 × side` |
| Top bevel | 1.5 lu `#FFFFFF @ 0.14`, top arc only, gradient to 0 at the midline |
| Bottom bevel | 1.5 lu `#000000 @ 0.55`, bottom arc only |
| Inner face | inset 3 lu, `#1A1A1A`, radius `bevel_radius − 3`, with `edge.wellTop`/`edge.wellBottom` |
| Elevation | E1 |

---

## 9. The cards

Diagrams are to scale in `lu` at S = 1. `·` marks a measured gap.

### 9.1 Clock card

Maps to `config::Clock` — `theme`, `font_size_pt` (default **64**), `anchor`
(default TopRight), `margin_px` (default 56), `show_seconds`, `show_date`,
`use_24h`, `accent_follow`.

**Derived sizes** (`H` = `font_size_pt` in lu):
```
micro     = nearest_ladder_step(0.17 · H),  min 11
secondary = nearest_ladder_step(0.22 · H),  min 11
pad       = clamp(4 · round(0.30 · H / 4), 12, 28)
R_card    = clamp(4 · round(0.42 · H / 4), 12, 32)
```
At H = 64: micro 11 · secondary 14 · pad 20 · R 28.

**Variant selection** (automatic, from H — a clock too small for three rows must
lose rows, not shrink them below the legibility floor):

| Condition | Variant |
|---|---|
| `H < 24` | **Bare** — hero only, no card, outline treatment (§4.5) |
| `24 ≤ H < 35` | **Compact** — card, micro + hero |
| `H ≥ 35`, `theme != Card` | **Standard** — card, micro + hero + secondary |
| `theme == Card` | **Expanded** — Standard plus the analog dial or the arc gauge |

#### 9.1.1 Standard — worked at H = 64

```
        ◄──────────────── 207 ─────────────────►
      ┌───────────────────────────────────────────┐  ▲   R = 28
      │                                           │  │
      │            (pad 20 all sides)             │  │
      │   ┌─────────────────────────────────┐     │  │   ← scrim, r 26,
      │   │ MONDAY · 28 JULY                │     │  │     feather 6, clamped
      │   │ ↕ 8 (cap)                       │  ·14│  │     to inner rect
      │   │                                 │     │  │
      │   │  ┌───┐                          │     │ 132
      │   │  │0 9 : 4 1│  ↕ 46.5 (cap)      │     │  │
      │   │  └───┘                          │  ·12│  │
      │   │                                 │     │  │
      │   │ Week 31 · GMT+05:30   ↕10 (cap) │     │  │
      │   └─────────────────────────────────┘     │  │
      │                                           │  │
      └───────────────────────────────────────────┘  ▼
      ▲                                           ▲
      edge.hairline 1 lu, full perimeter          edge.highlight 1 lu, top arc
      E2 shadow beneath
```

| Row | Step | Weight | Colour | Case |
|---|---|---|---|---|
| Micro-label (weekday · date) | `micro` 11 | 600 (CJK 700) | `text.tertiary` | UPPERCASE (Latin only) |
| Hero (time) | `H` 64 | 700 | `text.primary`; `accent.ink` when `accent_follow` | — |
| Secondary (ISO week · timezone) | `secondary` 14 | 500 | `text.secondary` | Sentence |

**Vertical rhythm — measured in cap tops and baselines, never em boxes:**
```
y_micro_cap      = pad                                    = 20
y_micro_baseline = y_micro_cap + 0.727 · 11               = 28.0
y_hero_cap       = y_micro_baseline + 0.22 · H            = 42.1  (lead 14.1)
y_hero_baseline  = y_hero_cap + 0.727 · H                 = 88.6
y_sec_cap        = y_hero_baseline + 0.19 · H             = 100.8 (lead 12.2)
y_sec_baseline   = y_sec_cap + 0.727 · 14                 = 111.0
height           = y_sec_baseline + 0.10 · 14 + pad       = 132.4 → 132
```
Each row's draw position subtracts `CAP_GAP × size` from the cap top, so the
renderer positions **capitals**, which is the only part of a row anyone sees.

**Width:**
```
W_text = max( advance(micro row), advance(hero row), advance(secondary row) )
width  = max( W_text + 2·pad , 3.1 · H )
```
At H = 64: hero `09:41` with Inter 700 tnum = (4 × 0.600 + 0.278) × 64 = 171.4,
less tracking (−0.019 × 64 × 4 internal gaps) = −4.9 → **166.5**.
`width = max(166.5 + 40, 198.4) = 206.5 → 207`.

**Alignment.** Everything is **left-aligned to the same x** (`pad`), including the
hero. Centring the hero over a left-aligned label is the single most common way
this layout goes wrong: the eye reads the label's left edge as the card's text
axis and a centred hero looks accidentally indented. The one exception is the
Compact variant when the micro-label is wider than the hero, where the block
centres as a unit.

**Width stability — the bug the reference cannot warn you about.**
`show_seconds` makes the hero change every second and `use_24h = false` makes it
change at noon. If the card is sized from the *current* string it will resize
under the user's cursor. So:

> The card's width is computed once, from the **widest string the current
> settings can ever produce**, and held until a setting changes.
> 24h + seconds → `00:00:00`. 12h + seconds → `00:00:00 PM`. 12h, no seconds →
> `00:00 PM`. The widest date and the widest weekday in the active locale are
> measured the same way, at locale-change time, not per tick.

Combined with tabular figures (§5.4), nothing on the card moves horizontally,
ever.

**Overflow and missing data.**
- `show_date = false` → the micro-label drops the date and keeps the weekday.
  The row is never empty; a card with a hole where a row goes is a different
  design.
- `theme == Minimal` → no micro-label, no secondary. Falls to Compact geometry
  regardless of `H`.
- Locale gives no ISO week or no timezone abbreviation → the secondary row drops
  to whatever it does have; if that is nothing, the row is removed and the card
  height recomputes. It does not render an empty 14 lu band.
- Hero wider than `0.94 × screen_width` → shrink-to-fit (§5.5), floor 80%, then
  the card is clamped to `0.94 × screen_width` and the hero ellipsises the
  seconds group first.

**Legibility over an arbitrary photo.**
Instruments 1+2+4. The scrim covers essentially the whole inner rect (§4.4), so
this card's worst case is the scrimmed dark row of §4.1: **13.1 / 7.3 / 4.8:1**
over a pure-white wallpaper, and **16.6 / 8.9 / 5.5:1** in light mode over pure
black. Both clear AA at every level and the hero clears AAA in both. Over a
bright *busy* wallpaper the residual mottle is 14% of the wallpaper's local swing
in dark mode and 4.5% in light — below the level at which Inter's counters break
at 11 lu, which is the smallest type on the card.

#### 9.1.2 Expanded (`theme == Card`)

```
   ◄───────── 207 ─────────►·24·◄──── 74 ────►
 ┌──────────────────────────────────────────────┐
 │  MONDAY · 28 JULY                    ····    │
 │                                    ·      ·  │
 │  09:41                            ·   ●    · │   ← arc gauge, r 37,
 │                                    ·      ·  │     day progress 0–24h
 │  Week 31 · GMT+05:30                 ····    │
 └──────────────────────────────────────────────┘
```
Two columns, gutter `gap.xl` = 24. Gauge outer radius `1.15 × H / 2` = 37 lu,
stroke `clamp(0.085 × 37, 3, 14)` = 3 lu, sweep 240°, value = fraction of the
local day elapsed, value dot r 2.6 lu.

The existing analog dial from `clock.rs` is the alternative right-column content
and keeps all of its published proportions (`CARD_TICK_*`, `CARD_HAND_*`,
`CARD_HUB_R`) — with one change: it sits in a `surface.well` disc instead of the
flat `CARD_FACE` fill, so the hairline minute ticks get the 3:1 they need. The
`CARD_MINUTE_TICK_MIN_R = 34` rule (drop the sixty minute ticks below that
radius) is unchanged and correct.

The gauge column is dropped entirely below a card width of 260 lu.

### 9.2 Now-playing / lyrics card

Maps to `config::Lyrics` — `style`, `anchor` (default BottomCenter),
`margin_px` (48), `font_size_pt` (default **28**), `accent_follow`, `colour`,
`show_next_line`, and the title/artist switch. `LyricStylePreset::Card` is the
preset this design *is*; `Minimal`, `Karaoke` and `Subtitle` keep their existing
card-less treatments with the §4.5 outline.

**Derived sizes** (`L` = `font_size_pt` in lu, default 28):
```
lyric     = L                                   → 28   (between lead 27 and 34; not snapped, it is user-set)
title     = nearest_ladder_step(0.64 · L)       → 18
body      = nearest_ladder_step(0.50 · L)       → 14
micro     = 11
art       = round(4.0 · title / 4) · 4          → 72   (album art side)
pad       = clamp(4·round((0.055·min(w,h)+8)/4), 12, 28)   → 20
R_card    = clamp(4·round(0.42·L/4), 12, 32)    → 12
```

```
     ◄──────────────────────── 420 ────────────────────────►
   ┌───────────────────────────────────────────────────────────┐ ▲  R 12
   │  ┌────────┐ ·16· NOW PLAYING                    ╭──────╮  │ │
   │  │        │      ↕8                             │ FLAC │  │ │  ← chip, h 22
   │  │  album │  ·8· Blue Monday              ↕13   ╰──────╯  │ │
   │  │  art   │  ·6· New Order · Substance    ↕10             │ │
   │  │  72×72 │                                               │ │
   │  │      ⊙ │ ·10· ▐████████████████●░░░░░░░░░░░░▌ 1:34/7:29│ │ 213
   │  └────────┘                            ↑ track h 4        │ │
   │   ↑ r 12   ↑ badge d 28, offset (−6,−6) from art's        │ │
   │            bottom-right corner                            │ │
   │ ─────────────────────  hairline, α 0.10  ──────────────── │ │
   │                          ·24·                             │ │
   │   I see a ship in the harbour                    ↕20 cap  │ │  ← lyric, 28 lu
   │                          ·8·                              │ │
   │   I can and shall obey                           ↕20 cap  │ │  ← next line, dim
   │                                                           │ │
   └───────────────────────────────────────────────────────────┘ ▼
```

| Element | Step | Weight | Colour |
|---|---|---|---|
| `NOW PLAYING` micro-label | `micro` 11 | 600 | `text.tertiary`, UPPERCASE, +0.128 em |
| Track title | `title` 18 | 600 | `text.primary` |
| Artist · album | `body` 14 | 500 | `text.secondary` |
| Elapsed / total | `caption` 11 | 500, **tnum** | `text.tertiary` |
| Current lyric | `lyric` 28 | 500 | `text.primary`; `accent.ink` when `accent_follow` |
| Next lyric (`show_next_line`) | `lyric` 28 | 500 | `text.tertiary` |
| Format chip | `micro` 11 | 600 | `accent.ink` on `surface.well` |

**Geometry.**
- Album art: 72 × 72, radius 12 (`0.18 × 72 = 12.96 → 12`), 1 lu `edge.hairline`
  ring at radius 11.5, E1 shadow. Non-square source is **centre-cropped**, never
  squashed — same rule `artwork::render_disc` already applies.
- Source-app badge (§8.6) overlaps the art's bottom-right corner by 6 lu on both
  axes, with the cutout punched in the art.
- The text column starts at `pad + art + 16` = 108 and runs to `width − pad`.
- Progress bar: full text-column width minus the time readout's advance minus
  `gap.m`. Track height 4 lu (`clamp(0.14 × 18, 4, 10)`), no knob (h < 6).
- Divider: 1 lu `text.primary @ 0.10`, full inner width, `gap.xl` above and below.
- Lyric rows are left-aligned to `pad`, **not** to the text column — the lyric is
  the card's subject and it gets the full width.

**Two scrims, not one.** The header block (micro + title + artist + progress) and
the lyric block get **separate** scrims, each sized by §4.4, with the divider
falling in the un-scrimmed gap between them. One scrim spanning both would fill
the card and there would be no glass left anywhere. The gap is 40 lu tall and is
where the wallpaper actually reads through.

**Overflow and missing data — this card has the most of it.**

| Condition | Behaviour |
|---|---|
| Title longer than the column | ellipsis, one line |
| Artist longer than the column | ellipsis, one line; `· album` is dropped **before** the artist is ellipsised |
| Lyric line longer than the card | wrap to 2 lines, then ellipsis. Card height grows by one `line_height(28)` = 1.216 × 28 = **34 lu** |
| `show_next_line = false` | the next-line row is removed and the card height shrinks by 8 + 20.4 = **28 lu** |
| No lyrics found for the track | the **entire lyric block and the divider are removed**; the card becomes header-only (height 112). Not an empty band, not a "no lyrics" message — a permanent apology on the wallpaper is worse than silence |
| Between lines / instrumental | the lyric row holds the **previous** line at `text.tertiary` for up to 8 s, then the block collapses as above. A row that blinks in and out every instrumental break is the single most irritating failure mode of a lyric widget |
| No album art | 72 × 72 `surface.well` square, radius 12, carrying a ♪ path glyph (drawn, not a font glyph) at 32 lu in `text.tertiary` centred |
| No MPRIS position | progress bar and time readout both hidden; the row collapses; card height shrinks by 10 + 4 |
| No title (stream with only a URL) | the micro-label becomes the stream host, the title row shows the URL's last path segment ellipsised, the artist row is removed |
| Card wider than `0.9 × screen_width` | clamp the width; the lyric wraps rather than the card growing |

**Card width.** Not measured from content — a card that changes width every time
the track changes is worse than one that is occasionally too wide. Fixed at
```
width = clamp( 15 · L , 320 , 0.9 · screen_width )     → 420 at L = 28
```
Height is content-driven and **animates** between states (§10):

```
pad                                                        20
header block  = max(art 72, text column 72)                72
gap.xl + divider + gap.xl                              24 + 1 + 24
lyric block   = 0.727·28 + 8 + 0.727·28 + 0.10·28         51.6
pad                                                        20
                                                        ───────────
height (both lyric rows present)                        212.6 → 213
height (header only, no lyrics)                                 112
```

**Legibility over an arbitrary photo.** Instruments 1+2+3+4. Header and lyric
blocks are scrimmed → the §4.1 scrimmed row applies: **13.1 / 7.3 / 4.8:1** worst
case dark, **16.6 / 8.9 / 5.5:1** worst case light. The progress bar sits in a
well → **3.84:1 minimum** for the Blue accent, 6.3–6.9:1 for the rest. The album
art carries its own pixels at full opacity and needs no help; its 1 lu hairline
plus E1 shadow keep it from dissolving into a photo of similar tone.

### 9.3 Visualiser panel

Maps to `config::Visualizer` — `style`, `anchor` (BottomCenter), `width_pct`
(60), `height_px` (120), `bands` (32), `accent_follow`, `colour`, `gradient`,
`colour_end`, `opacity` (220), `rounded`.

#### 9.3.1 Which treatment, and why

**Chosen: the Reference-A inset panel. Reference B ("Chassis") is specified as an
alternate theme in §9.3.3.** Four reasons, in order of weight:

1. **Fresco's widget layer has an empty input region. Nothing on it can be
   clicked, ever** — that is a load-bearing property of the design
   (`WIDGETS_ROADMAP.md`: "no click-through problem"). Reference B is
   approximately 80% controls: five large circular transport buttons, a knob, a
   volume slider, a 2 × 3 button grid. Drawing a play button that cannot be
   pressed is an affordance lie. Apple's *Familiarity* and *Grouping & mapping*
   principles and Rams' "make a product understandable" both forbid it, and it
   generates support load: someone *will* click it. Strip Reference B of every
   control it cannot honour and what remains — an LCD readout, a spectrum, a
   progress bar, all sunk into bevelled wells — is Reference A's inset panel with
   an orange tint.
2. **Cost.** The visualiser is the only widget that redraws every frame while
   audio plays, and `config.rs` is explicit that it "costs meaningfully more power
   than the other widgets". Reference A's panel is one well plus *n* bars: the
   card, well, hairlines and shadow rasterise **once** into a cached bitmap and
   only the bar rectangle is re-rasterised per frame. Reference B's chassis is
   ~14 bevelled sub-surfaces, and its analyser sits inside a glow that must be
   recomposited with its surroundings — a larger per-frame damage rect for the
   same information.
3. **Consistency.** The clock, now-playing and disc are all Reference A. One
   skeuomorphic slab among three flat cards is not a system, it is an accident.
4. **Colour.** Reference B's identity is `#F5A623`, which is not one of Fresco's
   six accents and would have to opt out of `accent_follow`. Reference A's
   language is accent-agnostic by construction.

#### 9.3.2 Panel variant (default when `width_pct ≤ 45`)

```
     ◄───────────────── width_pct% of screen ─────────────────►
   ┌──────────────────────────────────────────────────────────────┐ ▲ R 12
   │                          ·16·                                │ │
   │  ╭────────────────────────────────────────────────────────╮  │ │
   │  │▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│  │ │
   │  │              ▂  ▄     ▆  █  ▇     ▅  ▃                 │  │ │
   │  │        ▁  ▃  █  █  ▇  █  █  █  ▆  █  █  ▄  ▂  ▁        │  │ 120
   │  │  ▁  ▂  ▄  █  █  █  █  █  █  █  █  █  █  █  ▆  ▄  ▂  ▁  │  │ │
   │  │────────────────────────────────────────────────────────│  │ │ ← baseline
   │  ╰────────────────────────────────────────────────────────╯  │ │
   │                          ·16·                                │ │
   └──────────────────────────────────────────────────────────────┘ ▼
      ◄─ pad 16 ─►                                    ◄─ pad 16 ─►
```

| Property | Value |
|---|---|
| Card radius | **12 lu, fixed.** `R_card` is derived from `H_max`, and a text-free card has no `H_max`; 12 is what the lyric card lands on, which keeps the two neighbours in one family |
| Card height | `height_px` (config), min 56 lu |
| pad | 16 (from §6.2 at min(w,h) = 120) |
| Well | inset by `pad`, radius `max(4, 12 − 16) = 4`, full inner rect |
| Bar area | the well deflated by 6 lu on all sides |
| Bars | §8.4, `accent.fill`, per-bar vertical gradient `accent.dim` → `accent.fill` when `GradientMode::Vertical` |
| Baseline | 1 lu `data.gridline`, at the bar area's bottom |
| `Visualizer::opacity` | applied to the **bars only**, not to the card. Fading the card as well makes the panel vanish and the bars float, which looks like a bug. Card alpha stays at the token. |
| Peak caps | on; `text.primary @ 0.55`, 2 lu, hold 380 ms, fall 0.9 lu/frame |
| Silence | bars fall to the 2 lu minimum and the panel stays. The daemon stops pushing frames when silent (existing behaviour); the last frame drawn must be the resting one, not whatever the last sample produced |

**Legibility.** Instruments 1+3+4. There is no text, so no scrim. The bars sit on
a well and clear **3.84:1 minimum** (§4.3). Over a bright wallpaper the panel's
28% leak shows as a mottle *behind* the bars, which is harmless — there are no
counters to close.

#### 9.3.2b Bare variant (default when `width_pct > 45`, which includes the shipped default of 60)

A 60%-of-screen card is a slab, and a slab is not what someone turns a live
wallpaper on for. Above 45% width the card and well are **not drawn**. Instead:

| Property | Value |
|---|---|
| Bars | drawn directly on the wallpaper, `accent.fill`, `Visualizer::opacity` |
| Legibility | a **bottom scrim gradient**: `#04060A` (dark) / `#FFFFFF` (light) from α 0.42 at the bar baseline to α 0.00 at `1.35 × bar_area_height` above it, spanning the full bar width plus 24 lu of horizontal feather at each end |
| Per-bar shadow | E1, which is what keeps a bar visible where it crosses a same-tone region of the photo |
| Baseline | 1 lu at `text.primary @ 0.22` |
| Peak caps | off — without a well they read as debris |

This is instrument 5 territory: no card, so contrast is bought by the gradient
scrim and the shadow, and the bars carry no text to lose.

#### 9.3.3 Alternate theme: `Chassis` (Reference B)

Opt-in, and — like `LyricStylePreset::Karaoke`'s amber — it **opts out of
`accent_follow`** and uses a fixed `#F5A623`, because the look is the colour.
There is precedent for a preset owning its own colour (`Lyrics::colour` docs).

```
 ┌────────────────────────────────────────────────────────────────┐
 │▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ top bevel #FFFFFF α0.14 ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│ chassis #2B2B2B
 │                                                                │ R = 20
 │  Fatboy Slim — Ya Man                        4.8 MB · 1:34/3:52│ ← status strip
 │  ╭──────────────╮ ╭──────────────────────────╮ ╭────────────╮  │
 │  │  0 1 : 3 4   │ │ ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿ │ │ 278 KBPS   │  │
 │  │  00:00:00 ▶▮ │ │  glowing waveform, mag.  │ │ 44 KHZ     │  │
 │  │  ▁▂▃▅▆█▇▅▃▂  │ │                          │ │            │  │
 │  ╰──────────────╯ ╰──────────────────────────╯ ╰────────────╯  │
 │  ╭────────────────────────────────────────────────────────╮    │
 │  │▐████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░▌│    │
 │  ╰────────────────────────────────────────────────────────╯    │
 │________________ bottom bevel #000000 α0.55 __________________  │
 └────────────────────────────────────────────────────────────────┘
```

| Layer | Value |
|---|---|
| Chassis | `#2B2B2B`, R = 20, E3 shadow |
| Top bevel | 1.5 lu `#FFFFFF @ 0.14 → 0.00`, top arc |
| Bottom bevel | 1.5 lu `#000000 @ 0.55 → 0.00`, bottom arc |
| Wells | `#141414`, `edge.wellTop` `#000000 @ 0.70`, `edge.wellBottom` `#FFFFFF @ 0.07` |
| LCD digits | mono stack, `hero-s` 34, `#F5A623`, plus a 6 lu Gaussian glow of the same colour at α 0.22 drawn beneath |
| LCD secondary | mono `caption` 11, `#F5A623 @ 0.70` (0.45 gives 3.1:1 on the well — a fail; alpha-thinned orange runs out of contrast fast) |
| Spectrum | §8.4, `#F5A623`, no rounding, peak caps on |
| Waveform | `#D94FE0` → `#8B3BE8` vertical gradient, 6 lu glow at α 0.30 |
| Progress | §8.2, `#F5A623` fill in a `#141414` well |
| Status strip | mono `caption` 11, `#F5A623 @ 1.00` left (6.99:1 on the chassis), `#FFFFFF @ 0.62` right (6.41:1). Neither may be thinned further: `#F5A623 @ 0.60` on the chassis is 3.47:1 and `#FFFFFF @ 0.45` is 4.12:1 — both fail |

**Not drawn, and the theme is not negotiable about it:** the five circular
transport buttons, the small knob, the volume slider, and the 2 × 3 button grid.
Every one of them is a control, and this surface has no input. The bezel geometry
of §8.7 survives as a *readout* frame. The progress bar stays because it is real
data, not a control.

Contrast, `#F5A623` on `#141414` inside a well: **9.09:1**. The waveform's near
end `#D94FE0` on the same: **5.39:1**; its far end `#8B3BE8`: **3.41:1** — which
is why the waveform is specified as a *graphic* (3:1) and never carries type. The chassis is opaque, so no
wallpaper reaches the type and none of §4 applies — which is precisely why this
theme is easy and also why it is not the default: an opaque slab is not a live
wallpaper widget, it is a window without a title bar.

### 9.4 Album-art disc

Maps to `config::Disc` — `anchor` (BottomRight), `size_px` (220), `margin_px`
(48), `spin`, `opacity` (255) — and to `artwork::DiscCfg` (`label_ratio` 0.33,
`hole_ratio` 0.045, `ring_darken` 0.35), whose published proportions are
**unchanged**. This section adds the surface treatment that makes the disc a
member of the same system as the cards.

```
                    light source (12 o'clock)
                            ↓
                  ╭───────────────────╮
              ╭───╯ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ ╰───╮      ← rim highlight  #FFFFFF α0.22
            ╭─╯    ·  ·  ·  ·  ·  ·  ·    ╰─╮      upper arc, 1 lu
           ╭╯   ·  ╭───────────────╮  ·   ╰╮     ← specular sweep, 135°,
          ╭╯  ·   ╭╯               ╰╮   ·  ╰╮      #FFFFFF α0.10 → 0.00,
          │  ·   ╭╯     ╭─────╮     ╰╮   ·  │      FIXED in screen space
          │ ·    │      │  ⊙  │      │    · │
          │  ·   ╰╮     ╰─────╯     ╭╯   ·  │     ⊙ spindle, r = 0.045 R
          ╰╮  ·   ╰╮               ╭╯   ·  ╭╯     ╭─╮ label disc, r = 0.33 R
           ╰╮   ·  ╰───────────────╯  ·   ╭╯
            ╰─╮    ·  ·  ·  ·  ·  ·  ·  ╭─╯      ·  groove rings, 5 hairlines
              ╰───╮ ___________________ ╭───╯
                  ╰───────────────────╯          ← rim shade  #000000 α0.35
                                                   lower arc, 1 lu
                            E3 shadow beneath
```

| Layer | Value | Rotates with the disc? |
|---|---|---|
| E3 shadow | dark `0/16/40 @ 0.52` + `0/2/4 @ 0.36`; light `0/18/44 @ 0.40` + `0/2/4 @ 0.28`, colour `#0B1220` | no |
| Artwork | existing centre-crop, bilinear, analytic AA (`render_disc` steps 1–4, unchanged) | **yes** |
| Rim darkening | existing `ring_darken 0.35`, radial gradient from 0 at `0.82 R` to 0.35 at `1.0 R` — now a **real gradient** rather than the current linear ramp | yes (invisible either way — it is radially symmetric) |
| Groove rings | 5 hairlines at `0.42 / 0.53 / 0.64 / 0.75 / 0.86 R`, `#000000 @ 0.10`, each paired with a `#FFFFFF @ 0.05` hairline 1 lu outside it | yes (invisible — concentric) |
| Rim bevel — highlight | 1 lu arc, `#FFFFFF @ 0.22 → 0.00`, from −150° to +30° (upper), drawn at radius `R − 0.5` | **no** |
| Rim bevel — shade | 1 lu arc, `#000000 @ 0.35 → 0.00`, from +30° to +210° (lower), radius `R − 0.5` | **no** |
| Specular sweep | linear gradient at 135°, `#FFFFFF @ 0.10 → 0.00` across the upper-left 45% of the bounding box, clipped to the disc | **no — counter-rotated** |
| Label disc | circle `r = 0.33 R`, `surface.well` over the artwork, 1 lu `edge.hairline`, E1 | yes |
| Label text | `micro` 11 uppercase / `caption` 11, 2 lines, centred, ellipsised, `text.primary` + `text.secondary` | yes |
| Spindle hole | `r = 0.045 R`, punched from **alpha** (existing behaviour), plus a 1 lu `#000000 @ 0.40` ring just outside it | yes |

**The counter-rotated highlight is the detail that makes it read as a record.**
A specular highlight that spins with the artwork looks like a smear painted onto
the disc. A highlight fixed in screen space, with the artwork turning underneath
it, reads as a light in the room and a vinyl surface catching it. The rim bevel
is fixed for the same reason. Implementation: draw the artwork into a layer,
rotate that layer, then composite the three fixed layers (bevel, sweep, shadow)
on top in unrotated space.

**Label text is drawn only when both:** `2 × 0.33 R ≥ 96` lu (below that,
11 lu type in a 32 lu circle is two ellipses and no information) **and**
`Disc::opacity ≥ 200`. Below 200 the whole disc is being faded and the label's
contrast against the artwork behind it is no longer bounded — so the label
disappears and the disc becomes pure artwork, which is what a faded disc is for.

**Legibility over an arbitrary photo.** Instruments 3+4 only — the disc has no
scrim because it has no text over the wallpaper. Its label sits on a well over
opaque artwork. Its edge against a same-tone photo is carried by the rim shade
(a 0.35 black arc, which survives any backdrop) plus the E3 shadow, which is the
largest in the system precisely because this is the one widget that is a free
floating object rather than a panel.

**Worked size at defaults:** `size_px = 220`, so `R = 110`,
label r = 36.3, spindle r = 4.95, grooves at 46.2 / 58.3 / 70.4 / 82.5 / 94.6,
buffer = 220 + 2 × 84 = **388 × 388** lu at S = 1 (the E3 bleed, §7.4).
`2 × 0.33 R = 72.6 < 96`, so **at the default size the label carries no text** —
it is a plain accent-tinted disc. Text appears at `size_px ≥ 292`.

---

## 10. Motion

Fresco's widget layer takes no input, so almost none of the gesture machinery in
Apple's fluid-interface work applies. What does apply is *continuity*: a value
that changes should be seen to change, and a widget that appears should not
snap into existence.

| Event | Treatment | Duration |
|---|---|---|
| Lyric line advances | cross-fade old→new in place; if the new line has a different height, the card height springs (damping 1.0, response 0.30) | 180 ms ease-out |
| Track changes | album art scales 0.96 → 1.00 while cross-fading; text block cross-fades; progress bar snaps to 0 with **no** animation (animating it would draw a false seek) | 240 ms |
| Minute changes (clock) | cross-fade the hero only, not the card | 200 ms ease-out |
| Second changes (clock, `show_seconds`) | **no animation at all.** 60 cross-fades a minute is the power budget gone | 0 |
| Visualiser bands | per-band attack/release envelope, **not** the raw FFT: attack 45 ms, release 220 ms. This is the spring analogue and it is what separates a spectrum that looks alive from one that looks like noise | continuous |
| Widget appears / config change | fade in over 220 ms with a 0.98 → 1.00 scale about the widget's **anchor corner**, not its centre — it should look like it came from the edge it is pinned to | 220 ms |
| Card height changes (lyrics gained/lost) | spring, damping 1.0, response 0.30, animating the **card rect**; contents cross-fade | ~300 ms |

Everything above is skipped entirely when a hypothetical `widgets.reduced_motion`
is set, or when the desktop reports a reduced-motion preference: cross-fades stay
(they are not vestibular), scale and spring are replaced by an instant cut.

---

## 11. Where the references are not followed

Eight deliberate departures. Each is a place where copying the reference would
have made Fresco worse.

1. **Opacity is not the hierarchy mechanism in dark mode without a scrim.**
   Reference A's "secondary at ~60% opacity" gives **4.32:1** on a dark card over
   a bright wallpaper — a fail. The reference sits on a fixed photographic
   backdrop it was art-directed against; Fresco's backdrop is a video the
   renderer never sees. Hierarchy is therefore carried by size, weight and case,
   and the opacity ramp is only legal above a scrim (§2.5, §4.1).

2. **Light mode's ink ramp is 1.00 / 0.78 / 0.64, not a mirror of dark's
   1.00 / 0.70 / 0.52.** The sRGB transfer curve is not symmetric; the required
   ink alpha in light mode is ~0.61 for AA regardless of card opacity (§2.4a).

3. **The well inverts direction between themes** — darker than the card in dark,
   *lighter* in light. A darker light-mode well drives the surface to mid-grey
   and four of the six accents fail 3:1 there (§2.4c).

4. **No bar chart with weekday/value axes.** Reference A's `S M T W T F S` chart
   is beautiful and Fresco has no weekly data to put in it. The *bar array*
   component is adopted (§8.4) because the visualiser is one; the axis furniture
   is not. Inventing a "listening habits" chart would mean the daemon retaining
   playback history, which is a privacy surface Fresco has deliberately not
   opened (`config.rs`: "Nothing is analysed, stored or sent anywhere").

5. **No text curved along the arc gauge.** tiny-skia can do it (measure the path,
   place each glyph on its tangent), but at Fresco's gauge radii — 37 lu for the
   clock's day gauge — 11 lu type on a 37 lu arc has a per-glyph rotation of
   ~17°, which at 1080p is a stack of individually-rotated 11 px glyphs. It reads
   as damage. The gauge's label goes in its centre instead. Revisit above a
   90 lu radius.

6. **No check-marker on the progress bar.** Reference A puts a status check at the
   current position on its progress bars. A check means *done*; on a playback
   progress bar the current position is the opposite of done. The knob (§8.2) is
   the honest marker and it only appears when the bar is thick enough to carry it.

7. **Reference B's controls are not drawn** — five transport buttons, a knob, a
   volume slider, a 2 × 3 button grid. The surface has an empty input region;
   every one of them would be a lie (§9.3.1). The bevel, the wells, the LCD, the
   glow and the orange all survive.

8. **The visualiser's default is card-less.** Reference A puts everything in a
   card. At Fresco's shipped `width_pct = 60` a card is a slab across most of the
   screen, which defeats the point of a live wallpaper. The panel treatment
   engages below 45% width; above it the bars get a gradient scrim and a shadow
   instead (§9.3.2b).

Two further notes on things the references *do* that Fresco keeps but re-derives:

- **The tinted card** (Reference A's light-blue weather card in a dark set) is
  adopted as the album-art tint, but constrained to hue and chroma with luminance
  pinned to the token, so the contrast tables hold by construction (§3.3).
- **The overlapping circular badges** are adopted with a cutout punched in the
  badge beneath, rather than a heavy stroke around each (§8.6) — a stroke that
  reads well on a fixed backdrop turns into a bright ring on a bright wallpaper.

---

## 12. Implementation checklist

- [ ] `Theme::dark()` / `Theme::light()` carry every token in §3.1 / §3.2 by name.
- [ ] Accent derivation is `mix(accent, white, 0.32 / 0.14)` in dark and
      `mix(accent_light, black, 0.22)` in light — **not** the raw config accent.
- [ ] Card fill gradients vary colour only; alpha is a single constant (§2.2).
- [ ] Every dark text block has a scrim. There is no code path that draws
      `text.secondary` or `text.tertiary` on a dark card without one (§2.3).
- [ ] Every accent-filled data graphic sits on a well (§4.3).
- [ ] Text measurement happens before layout; ellipsis cuts at grapheme cluster
      boundaries; unrenderable codepoints are stripped, not drawn (§5.2, §5.5).
- [ ] Numeric runs request `tnum`, with the max-advance fallback (§5.4).
- [ ] CJK runs get weight 500/700 only, no case transform, +0.040 em tracking,
      and 1.18× line-height (§5.3).
- [ ] The clock's width is computed from the widest reachable string, not the
      current one (§9.1.1).
- [ ] Widget buffers are inflated by the shadow bleed, and `margin_px` is measured
      to the card rect, not the buffer (§7.4).
- [ ] The disc's rim bevel and specular sweep are composited **after** rotation
      (§9.4).
- [ ] Seconds ticks are not animated (§10).
