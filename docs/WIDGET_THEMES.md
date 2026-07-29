# Fresco Widget Themes — design plan

Status: proposed 2026-07-29. Nothing implemented. Design only; no code in this document.
Companion to [WIDGETS_ROADMAP.md](WIDGETS_ROADMAP.md) (W5 "Themes & styling").

Scope: visual themes for the on-wallpaper widgets — **lyrics** (`src/lyrics.rs`), **clock**
(`src/clock.rs`) and **visualiser** (`src/visualizer.rs`). Lyrics are confirmed working
end-to-end on a live Wayland/COSMIC session against Firefox, and widgets now draw on **all**
displays by default (`widgets.monitor` names a single connector when you want just one). So
this is a shipping feature being dressed, not a hypothesis being explored.

---

## 1. The substrate, honestly

Every widget Fresco draws is a string of **ASS/SSA subtitle markup** pushed to mpv's
`osd-overlay` command with `format: ass-events`. libass rasterises it over the wallpaper
video. That is the whole rendering budget, and it is worth being blunt about its edges before
proposing anything, because a plan full of un-buildable ideas is worse than a short buildable
one.

### What ASS *can* do

Verified against libass's own tag parser
([`ass_parse.c`](https://github.com/libass/libass/blob/master/libass/ass_parse.c) — the
accepted tag list is literally the string table in that file):

| Capability | Tags |
|---|---|
| Family, size, weight, italic, underline, strikeout | `\fn` `\fs` `\b` `\i` `\u` `\s` |
| Letter spacing | `\fsp` |
| Fill / outline / shadow colour | `\1c` `\3c` `\4c` (and `\2c`, unused on this path) |
| Per-channel alpha | `\1a` `\3a` `\4a` `\alpha` |
| Outline width, asymmetric outline | `\bord` `\xbord` `\ybord` |
| Shadow offset, asymmetric shadow | `\shad` `\xshad` `\yshad` |
| Gaussian blur, box blur | `\blur` `\be` |
| Rotation in three axes (real perspective) | `\frz` `\frx` `\fry`, origin via `\org` |
| Non-uniform scale | `\fscx` `\fscy` |
| **Shear / oblique** | `\fax` `\fay` |
| Placement | `\an` `\pos` |
| Line wrapping mode | `\q` |
| **Rectangular and vector clipping** | `\clip` `\iclip` |
| **Arbitrary filled vector shapes** (lines + cubic béziers) | `\p1` … `\p0` |
| Per-run overrides mid-line | `{…}` blocks anywhere in the text |

Two capabilities in that table are under-used today and unlock most of what follows:
**`\fax` shear**, **`\frx`/`\fry` perspective**, and **`\iclip` vector masking**.

### What ASS *cannot* do

- **No bitmaps.** Images need mpv's separate `overlay-add`, which is a different command with
  a different Z stratum ("bitmap overlays added by `overlay-add` are always on top of the ASS
  overlays added by `osd-overlay`", [mpv `input.rst`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/input.rst)).
  No album art, no textures, no noise, no film grain.
- **No gradients.** There is no gradient tag. Every fill is flat. Every synthwave sunset,
  every chrome bevel, every "sky fading to purple" reference below has to be reinterpreted as
  flat bands or abandoned.
- **No time-based animation.** This is the one people get wrong. mpv builds every overlay
  event with `Start = 0, Duration = 100` and then calls
  `ass_render_frame(ass->render, ass->track, 0, &ass_changed)` — **time is a hard-coded
  literal `0`** ([`sub/osd_libass.c`](https://github.com/mpv-player/mpv/blob/master/sub/osd_libass.c),
  the `add_osd_ass_event` and `append_ass` functions). Therefore:
  - `\t(…)` transforms render at their *pre-transform* value. Visually a no-op.
  - `\k` / `\kf` / `\ko` karaoke sweeps never advance — a `\kf` payload renders permanently
    unswept, which reads as *broken*, not as *absent*.
  - `\move` never moves.
  - `\fad(in,out)` at t=0 is **fully transparent**. A theme that uses it renders nothing at
    all. Do not use it.
  This is already documented in the `TODO(W5)` block in `src/lyrics.rs`; it is restated here
  because it kills roughly a third of the visual references in this space (CRT flicker, VHS
  tracking roll, neon buzz, XP-bar fill animation, achievement toast slide-in).

### Three things that *are* possible and that the current code deliberately avoids

These are the levers that make striking themes reachable without W2, and each is a small,
contained change to an existing renderer.

**(a) Multi-event payloads.** mpv's docs are explicit: with `ass-events`, the `data` string
"is split on the newline character. Every line is turned into the `Text` part of a `Dialogue`
ASS event", and "it's better to put multiple lines into `data`, instead of adding multiple OSD
overlays." Every event gets `ReadOrder = n` and the same Layer, so **later lines draw on top
of earlier lines**. Today `lyrics::render_ass`, `clock::render_ass` and
`visualizer::render_ass` all assert "no newlines" — a *safety* rule (an under-specified second
event would inherit mpv's OSD style), not a platform limit. Lifting it deliberately, with
every event fully specifying its own tags, gives us **stacked layers**: the single most
important technique in this document.

**(b) Layered duplicate text = fake glow, fake gradient, fake extrusion.** Draw the same
string two or three times at the same `\pos`, back to front:

```
event 1:  {…\1a&HFF&\3c&HFF00FF&\bord14\blur10}  same text   ← outer magenta halo, no fill
event 2:  {…\1a&HFF&\3c&HFF00FF&\bord6 \blur4 }  same text   ← inner tighter halo
event 3:  {…\1c&HFFFFFF&\bord0\shad0           }  same text   ← hot white core
```

That is exactly the layer structure every "how to fake neon" tutorial describes (bright core
+ coloured bloom + outer glow) and it costs one string push. `\1a&HFF&` makes the fill
invisible so only the halo survives — that trick is what makes layers 1 and 2 pure glow.

**(c) A real background plate.** `\p1` draws filled vector shapes, and a rectangle is four
`l` commands. A dark rounded plate as event 1 and the text as event 2 gives the
**"text on a box"** treatment that the accessibility literature considers the most reliable
option (see §3). `\bord` + `\blur` on the plate feather its edge. This replaces the
`LyricStylePreset::Card` hack in `src/daemon/lyrics_runtime.rs`, which currently fakes a panel
with a heavy near-white *outline* around near-black ink — a genuinely clever workaround that
a real plate makes unnecessary.

Two supporting facts: `osd-overlay` takes a **`z`** parameter (currently hard-coded `"0"` in
`src/daemon/mpv/player.rs`) and mpv sorts overlays by it, so a plate can also live in its own
overlay id below the text. And `compute_bounds` on the same command returns the rendered
text's `x0/x1/y0/y1` rectangle in PlayRes coordinates — which is how a plate gets sized to fit
the *actual* line rather than a guess. Both are small extensions to existing plumbing.

### Fonts: the hard practical constraint

`\fn` names a family; libass resolves it through fontconfig, and **fontconfig always
substitutes silently**. Measured on the maintainer's own machine:

```
$ fc-match Orbitron
NotoSans-Regular.ttf: "Noto Sans" "Regular"     ← not a failure, a silent swap
$ fc-list "Orbitron" family
                                                 ← empty means genuinely absent
```

So a theme naming a font the user does not have does not fail loudly — it renders in Noto
Sans and looks like a bug. What is actually present on that machine, out of every candidate in
this document:

| Present | Absent (would need bundling) |
|---|---|
| Inter, DejaVu Sans Mono, Noto Sans, Noto Sans Mono, **Noto Sans CJK**, Liberation Sans | Orbitron, VT323, Share Tech Mono, Press Start 2P, DSEG, Oswald, Anton, Bebas Neue, Archivo, Saira, Rajdhani, Chakra Petch, Michroma, Audiowide, Monoton, Bungee, Silkscreen, Pixelify Sans |

**Consequence, and it is the single biggest decision in this plan:** every theme below is
tagged either *stock font* (ships today, zero risk) or *needs bundling*. Bundled fonts go into
`/app/share/fonts` via a new module in `flatpak/io.github.dibbayajyotiroy.Fresco.yaml` (the
sandbox already bundles its own mpvpaper, so libass inside the sandbox will see them), and
into `/usr/share/fonts/fresco/` from the `packaging/debian` and `packaging/aur` recipes.
Fresco should also run `fc-list "<family>" family` at theme-apply time and, if empty, show a
"this theme's font is not installed — it will look wrong" note in the picker rather than
silently rendering Noto Sans. **Only SIL OFL / Apache-2.0 / MIT / public-domain families
appear below.** Fresco is GPL-3.0-or-later; OFL fonts are safe to bundle and redistribute.

**Explicitly rejected on licence grounds** (they are the "correct" references, and we cannot
ship them):

- **Eurostile / Microgramma Bold Extended** — the actual in-world Blade Runner face; the
  Spinner's `CAUTION` display is Eurostile Bold Extended, per the exhaustive frame-by-frame
  breakdown at [Typeset in the Future](https://typesetinthefuture.com/2016/06/19/bladerunner/).
  Commercial (Linotype). Substitute: **Michroma** (OFL).
- **"Blade Runner Movie Font"** (Phil Steinschneider, [dafont](https://www.dafont.com/blade-runner-movie-font.font))
  — freeware, but not under a redistributable free-software licence and of unclear
  provenance. Do not bundle.
- **Digital-7**, and most "LED clock" fonts on the free-font aggregators — personal-use-only.
  Substitute: **DSEG** (keshikan), which is SIL OFL.
- **The Ultimate Oldschool PC Font Pack** (IBM VGA 8×16 etc.) — CC BY-SA 4.0. Redistributable
  *with* attribution and share-alike, which is fine for a GPL app but adds a licence file and
  a NOTICE obligation to packaging. Usable, but VT323 (OFL) gets 90% of the look with none of
  the paperwork. Prefer VT323.

---

## 2. Reference research

### 2.1 Retro / 80s–90s

What actually makes something read as retro-screen, distinct from "old-looking":

- **Phosphor colour, not just "green".** P1 phosphor gives the green monochrome look; P3 gives
  amber ([Monochrome monitor, Wikipedia](https://en.wikipedia.org/wiki/Monochrome_monitor)).
  The IBM 5151 actually used **P39**, a long-persistence green chosen for text legibility
  ([IBM 5151, Wikipedia](https://en.wikipedia.org/wiki/IBM_5151)); DEC shipped VT220s with
  amber CRTs. The *look* is not the hue alone — it is a **single saturated hue on near-black,
  with a soft same-hue bloom around every glyph**, because long-persistence phosphor smears.
  That bloom is `\bord` + `\blur` with `\3c` equal to `\1c`, which `ClockTheme::Segment`
  already does in `src/clock.rs` (`glow: true`). Fresco has quietly shipped the core CRT
  technique for a while; it just is not dressed as a theme.
- **Scanlines.** The one CRT signature Fresco cannot get from colour alone — and it *is*
  reachable, via `\iclip` with a `\p1` drawing of horizontal bars. Masking the text with a
  striped path punches evenly-spaced gaps through the glyphs. Static in screen space, which is
  correct: a real scanline grid does not move either. Cost is a few dozen contours confined to
  the widget's band, not the whole screen.
- **Synthwave / outrun.** The canonical palette is well fixed by the most-installed artefact
  of the style, the **SynthWave '84** VS Code theme
  ([robb0wen/synthwave-vscode](https://github.com/robb0wen/synthwave-vscode)): background
  `#262335`, cyan `#03edf9` / `#36f9f6`, hot pink `#ff7edb`, yellow `#fede5d`, red `#fe4450`,
  coral `#f97e72`, mint `#72f1b8`. Its identity is *neon on deep indigo-violet*, plus italic
  extruded display type. The **horizon gradient is not reachable** — no gradients — but the
  **perspective grid is**, via `\p1` lines under `\frx` (a real 3D tilt), and italic extrusion
  is reachable via `\fax` shear plus an offset duplicate event.
- **7-segment / VFD / nixie.** A seven-segment display's tell is the **fixed digit grid** and
  the faintly-visible *off* segments; a nixie's is a warm neon-orange glow with visible depth;
  a VFD's is a distinctive blue-green. Fresco can do the grid (monospace + `\fsp`), the glow
  (`\bord`+`\blur`+`\3c`=`\1c`), and — with DSEG's dedicated "all segments on" characters —
  even the ghost segments, by drawing a dim `88:88` event *behind* the live time. That last
  one is the detail that sells the whole theme.
- **Terminal monochrome.** Amber vs green, fixed-pitch, no anti-aliased curves, wide tracking,
  all-caps labels. VT323 is the free face that reads as this instantly.

### 2.2 Blade Runner / cyberpunk

**The two films do not share a palette, and conflating them is the most common mistake.**

- **1982 (Jordan Cronenweth).** Cool cyan-heavy streets against warm golden interiors; neon in
  saturated blues, reds and greens over "a backdrop of desaturated grays and browns"; deep
  blacks that "retain texture"
  ([Color Culture](https://colorculture.org/blade-runner-cinematography-analysis/)). Ridley
  Scott's own summary of the aesthetic was "night, wet, smoke". The *warmth* is
  sodium-vapour-and-haze; the on-screen graphics language is lo-res and typographic.
  Typography, verified frame by frame by
  [Typeset in the Future](https://typesetinthefuture.com/2016/06/19/bladerunner/): Goudy Old
  Style for the opening crawl, **Eurostile Bold Extended** for the Spinner's `CAUTION`
  readout, OCR-A on the videophone, Berthold Block Heavy on the Bradbury Building. The
  in-world computer face is wide, extended, all-caps and mechanical — that is the thing to
  reproduce.
- **2049 (Roger Deakins).** A sharply different, colder grade: "blue-green and off-white",
  deep blue-green shadows, yellowish off-white highlights, "monochromatic" while keeping
  contrast, "fairly subdued" saturation
  ([RK Color's look-build breakdown](https://www.rkcolor.com/blog/look-builds-blade-runner-2049/)).
  Against that base, single hard hue washes: the Las Vegas orange fog, Wallace's yellow, and
  Joi's magenta-purple bloom, which Deakins achieved with practical light diffused through fog
  ([StudioBinder](https://www.studiobinder.com/blog/blade-runner-2049-cinematography-analysis/),
  [Deakins at the ASC](https://theasc.com/article/deakins-blade-runner-2049/)).
- **Neon typography.** Every credible fake-neon tutorial describes the same layer stack: a
  hot, nearly-white **core**, a saturated **tube colour** just outside it, and a wide soft
  **bloom** beyond that. This maps one-to-one onto technique (b) above. What does *not* map is
  the tube's specular highlight and the coloured light spill onto the wall behind — both need
  gradients.
- **CJK signage.** Dense mixed-script signage is a load-bearing part of the look. **Noto Sans
  CJK is already installed** on the target machine and is OFL, so a theme can set a
  Japanese/Chinese secondary label with zero bundling cost. Worth using sparingly — a decorative
  label, not the lyric itself.
- **Industrial stencil / "dirty future".** Tracked-out all-caps small labels, stencil cuts,
  monospaced serial numbers. Free stencil faces exist (Saira Stencil One, Stardos Stencil,
  Black Ops One — all OFL) but all need bundling.

### 2.3 Gamified / HUD

The conventions that read as "game UI" instantly, filtered for what survives a vector-only,
flat-fill renderer:

- **Corner brackets / frame ticks.** Four L-shaped strokes at the corners of an invisible box.
  Pure `\p1` line work. Reads as HUD immediately and costs almost nothing. This is the highest
  value-per-byte idea in this whole document.
- **Segmented bars.** A background track, a filled portion, notch dividers, an outline. All
  rectangles. **Survives the constraint completely** — and the visualiser already draws
  rectangles with rounded caps in `src/visualizer.rs` (`Path::bar`, `Cap::Top`). A notched
  "XP bar" style is a small addition to that module, not a new one.
- **Skewed parallelogram panels.** `\p1` quads with sheared sides, or `\fax` on a text run.
  Trivially buildable.
- **All-caps condensed labels with wide tracking.** `\fsp` plus a condensed family.
- **Persona-5-style graphic overlay.** Red/black/white, extreme diagonals, heavy italic
  condensed type, cut-paper irregularity. The palette and the diagonals are buildable
  (`\frz` rotation, `\fax` shear, `\p1` slashes); the *cut-paper texture* is not. The
  ransom-note per-character variation **is** reachable, because ASS allows an override block
  before any character — alternating `\frz-4` / `\frz3` / `\fscy110` per glyph gives exactly
  that hand-cut jitter. It is the one place where the "no animation" limit costs nothing,
  because the effect was never animated.
- **Arcade / 8-bit.** Outlined pixel type with a chunky shadow offset by exactly one "pixel":
  `\xshad` + `\yshad` set to the same integer, `\bord` hard, `\blur0`. Needs a pixel font
  (Press Start 2P or Silkscreen, both OFL, both need bundling). Pixel fonts must be used at
  **integer multiples of their design size** or they blur — a real constraint on the size
  slider.
- **Achievement toast / quest log.** The *layouts* are buildable (left rail + accent bar +
  two-line text). The slide-in and the progress fill are not. A toast that never slides in is
  just a card; ship it as a card and do not pretend.

### 2.4 Legibility over arbitrary moving video — the actual hard problem

A wallpaper is a moving photograph. The same text must survive a blown-out sky and a night
interior, and the technique choice matters far more than the palette.

| Technique | Works over bright? | Works over dark? | Expressible in ASS? |
|---|---|---|---|
| Hard outline / stroke | yes | yes | **yes** — `\bord` + `\3c` |
| Drop shadow alone | partially | partially | yes — `\shad`/`\xshad`/`\yshad` + `\4c` |
| Blurred halo (contrasting luminance) | yes | yes | **yes** — `\bord` + `\blur` + `\3c` |
| Semi-transparent plate / scrim | **yes** | **yes** | **yes** — `\p1` rect event + `\1a` |
| Contrast-adaptive text colour | yes | yes | **no** — needs frame sampling; also flickers |
| Double outline (dark blur + tight light stroke) | yes | yes | **yes** — two stacked events |

The accessibility and broadcast consensus, and it is consistent across sources:

- **A shadow on its own is not enough.** As Ian Hamilton puts it, "Drop shadow by itself is
  not recommended, as it leaves half of the letter without anything separating it from the
  background… As a minimum there should be a prominent black stroke"
  ([ian-hamilton.com](https://ian-hamilton.com/how-to-do-subtitles-well-basics-and-good-practices/)).
  Fresco already does the right thing here — `lyrics::override_tags` emits both a
  size-proportional `\bord` *and* a softened `\4a&H80&` shadow.
- **A box beats a stroke when you cannot control the background.** WCAG's technique
  [G18](https://www.w3.org/WAI/WCAG22/Techniques/general/G18) says it directly: where the
  background varies in luminance or is patterned, "the background around the letters can be
  chosen or shaded so that the letters maintain [the] contrast ratio with the background
  behind them even if they do not have that contrast ratio with the entire background."
  Shading the background *is* the sanctioned answer, and it is exactly technique (c).
  Failure [F83](https://www.w3.org/WAI/WCAG22/Techniques/failures/F83) is precisely
  "background images that do not provide sufficient contrast".
- **Light-on-dark beats dark-on-light** for this case, which is why almost every theme below
  puts light type on a dark halo or plate rather than the reverse.
- **Adaptive colour is a trap.** Sampling the frame and inverting the text would technically
  work, but the text would then change colour as the video plays — flicker that is both
  distracting and, at the wrong rate, an accessibility hazard. It also demands a per-frame
  read-back, which contradicts the roadmap's "redraw only when content changes" rule outright.
  **Rejected.**

**Rule for every theme in §3:** a theme may express its identity through fill colour, family,
tracking and glow, but the *outline or plate is not negotiable*. Where a theme's authentic
colour is low-contrast (amber phosphor on black is fine; hot pink on white is not), the theme
carries a black or near-black halo underneath its coloured glow. That is the double-outline
row in the table above, and it is what lets a neon theme stay readable over a snow scene.

---

## 3. The themes

Ten proposals. Each states which widgets it dresses, its exact palette, the tags that produce
it, its font and licence status, and a legibility note. Colours are `#RRGGBB` as the config
already stores them (`hex_to_ass_colour` handles the BGR flip).

---

### T1 — **Phosphor**
*A 1979 terminal left running on your desktop.*

- **Buildable now.** Uses only tags already emitted by `clock.rs`; no new renderer capability.
- **Reference:** [Monochrome monitor](https://en.wikipedia.org/wiki/Monochrome_monitor),
  [IBM 5151](https://en.wikipedia.org/wiki/IBM_5151) (P39 long-persistence green).
- **Font:** `DejaVu Sans Mono` — **stock, already installed, no bundling.** VT323 (OFL,
  [Google Fonts](https://fonts.google.com/specimen/VT323)) is the upgrade if bundling happens;
  it is the single highest-return font to bundle in this document.
- **Palette:** two variants, and offering both is the whole charm.
  - *Green:* fill `#33FF66`, glow `#33FF66`, halo `#001A08`
  - *Amber:* fill `#FFB000`, glow `#FFB000`, halo `#1A0F00`
- **Tags:** `\fnDejaVu Sans Mono\b1\fsp{9% of size}` then a **two-event stack** — event 1
  `{…\1a&HFF&\3c&H080A00&\bord10\blur6}` (dark halo, invisible fill) and event 2
  `{…\1c&H66FF33&\3c&H66FF33&\bord5\blur4}` (the phosphor bloom). The dark under-halo is what
  makes it survive a bright frame; without it, green-on-white is unreadable.
- **Widgets:** clock (its natural home — it is `ClockTheme::Segment` finished), lyrics,
  visualiser (bars tinted to the same green, `rounded: false`, gap wide so they read as cells).
- **Legibility:** strong. Saturated hue over a wide dark blurred halo is the double-outline
  pattern, and the halo does the work regardless of what is behind it.

---

### T2 — **Cathode**
*Phosphor, plus the scanlines. The one that makes people say "how did you do that".*

- **Buildable now**, but needs one new primitive: an `\iclip` striped mask generated in the
  same `\p1` grammar `visualizer.rs` already has (`Path::rect`, `Path::pt`). ~30 lines.
- **Reference:** as T1, plus the persistent-phosphor + scanline pairing that defines every CRT
  shader.
- **Font:** as T1.
- **Palette:** as T1, with the mask providing the texture instead of colour.
- **Tags:** T1's stack, plus `\iclip(1, m 0 0 l W 0 l W 2 l 0 2 …)` repeated on a 4-unit pitch
  across the widget's band. Applies to fill *and* outline, so the whole glyph is striped, which
  is correct.
- **Widgets:** clock and lyrics. **Not** the visualiser — striping bars that are already
  striped by their own gaps reads as noise.
- **Legibility:** *weakest theme in this document, and it must be labelled as such.* Removing
  a third of every glyph costs real contrast. Mitigations: pitch no finer than 4 units at
  PlayRes 1080, mask only the fill (leave the halo unmasked by putting the halo in an unclipped
  event), and gate the theme behind a larger minimum size. Ship it, but do not default to it.

---

### T3 — **Outrun**
*Neon on deep indigo. Miami, 1984, at 3am.*

- **Buildable now** via the stacked-event glow (technique b). The horizon grid is a stretch
  goal — see below.
- **Reference:** [SynthWave '84](https://github.com/robb0wen/synthwave-vscode), whose palette
  is the de-facto standard for the style.
- **Font:** `Orbitron` (SIL OFL, [Google Fonts](https://fonts.google.com/specimen/Orbitron)) —
  **needs bundling.** Fallback if bundling is deferred: `Inter` bold with heavy `\fsp` and a
  `\fax0.15` shear, which gets a surprising distance.
- **Palette:** hot pink `#FF7EDB`, cyan `#03EDF9`, yellow `#FEDE5D`, ground `#262335`,
  under-halo `#0A0614`.
- **Tags:** three stacked events — `{\1a&HFF&\3c&H140A0A&\bord16\blur12}` (dark bed),
  `{\1a&HFF&\3c&HDB7EFF&\bord8\blur7}` (pink bloom), `{\1c&HFFFFFF&\3c&HDB7EFF&\bord2\blur1}`
  (white-hot core in a pink tube). Add `\fax0.18` for the italic-extruded feel and a
  `\xshad3\yshad3\4c&HF9ED03&` cyan offset for the chromatic split.
- **Widgets:** all three. The visualiser is where this theme is *best* — bars in cyan with a
  pink glow is the reference image.
- **Stretch:** the perspective grid, as `\p1` lines under `\frx60\org(960,1080)`. Buildable,
  genuinely striking, and the one place `\frx` earns its keep. Costs a static drawing, no
  animation — which is fine, since the reference grid scrolls and ours would not, so ship it
  as a static horizon or not at all.
- **Legibility:** good *because* of the dark bed layer. Pink and cyan alone over a bright
  frame would fail; the `#0A0614` bloom underneath is doing all the safety work. Do not let a
  future "simplify the theme" pass delete it.

---

### T4 — **Nixie**
*Warm orange glow in a glass tube. The most beautiful clock we can draw.*

- **Buildable now.** Clock only.
- **Reference:** nixie tube displays — neon-orange cathode glow, fixed digit cells, visible
  unlit digits behind the lit one.
- **Font:** `DSEG14 Classic` or `DSEG7 Classic` (keshikan, **SIL OFL**,
  [github.com/keshikan/DSEG](https://github.com/keshikan/DSEG)) — **needs bundling**, and it
  is worth it: DSEG is the only free family that draws true segment shapes rather than
  imitating them. Stock fallback: `DejaVu Sans Mono` at heavy `\fsp`.
- **Palette:** lit `#FF6E1A`, glow `#FF6E1A`, ghost segments `#2A1206`, bed `#140800`.
- **Tags:** **three events.** Event 1 a dark bed `{\1a&HFF&\3c&H001020&\bord12\blur8}`;
  event 2 the *ghost* — the string `88:88` in `#2A1206` at the identical `\pos`, `\bord0`;
  event 3 the live time with `{\1c&H1A6EFF&\3c&H1A6EFF&\bord7\blur6}`. The ghost layer is the
  entire theme. Without it this is just an orange clock.
- **Widgets:** clock. Optionally the visualiser as amber bars.
- **Legibility:** good. Orange has high luminance contrast against most night footage and the
  dark bed covers the daylight case. The ghost digits deliberately sit *below* legibility
  thresholds — they are texture, not information, so that is correct rather than a violation.

---

### T5 — **Spinner** *(Blade Runner 1982)*
*Warm sodium haze, wide mechanical caps, a machine that speaks in all-caps.*

- **Buildable now.**
- **Reference:** [Typeset in the Future's Blade Runner
  breakdown](https://typesetinthefuture.com/2016/06/19/bladerunner/) (Eurostile Bold Extended
  on the Spinner readout, OCR-A on the videophone);
  [Color Culture on Cronenweth's palette](https://colorculture.org/blade-runner-cinematography-analysis/)
  (cyan streets, golden interiors, desaturated grey-brown ground).
- **Font:** `Michroma` (SIL OFL, [Google Fonts](https://fonts.google.com/specimen/Michroma))
  — **needs bundling**; it is the closest free face to Eurostile Extended. **Eurostile itself
  is commercial and must not be shipped.** Stock fallback: `Inter` with `\fscx125\fsp` to fake
  the extension — passable, not the same.
- **Palette:** amber `#F0A63C`, warm white `#FFEBCF`, haze `#6E4A22`, bed `#14100A`.
- **Tags:** all-caps text, `\fsp{14% of size}`, `\fscx115` for the extended feel,
  `\1c&H3CA6F0&\3c&H224A6E&\bord6\blur5` — a *warm* halo rather than a black one, which is the
  sodium-vapour trick, over a `\4c&H000000&\4a&H60&` shadow for the dark-frame case. A small
  `\fs{30%}` label line above the time (`LAPD · SECTOR 4`) in `#6E4A22` sells it.
- **Widgets:** clock (primary), lyrics.
- **Legibility:** acceptable but the weakest of the "safe" set, because the warm halo has
  less luminance separation from a warm frame than black would. Mitigation: keep the
  `\4a&H60&` shadow — it is the black component, and it is doing the work when the wallpaper
  is a sunset.

---

### T6 — **Wallace** *(Blade Runner 2049)*
*Cold, still, expensive. Blue-green and off-white, with one hard hue.*

- **Buildable now.** This is the most *restrained* theme here and probably the best-looking.
- **Reference:** [RK Color's 2049 look-build](https://www.rkcolor.com/blog/look-builds-blade-runner-2049/)
  ("blue-green and off-white", deep blue-green shadows, yellowish off-white highlights,
  "monochromatic", "fairly subdued");
  [StudioBinder](https://www.studiobinder.com/blog/blade-runner-2049-cinematography-analysis/)
  on Joi's purple bloom;
  [Deakins at the ASC](https://theasc.com/article/deakins-blade-runner-2049/).
- **Font:** `Inter` — **stock, no bundling.** Light weight, very wide tracking. (`Saira`
  Light, OFL, is the upgrade.)
- **Palette:** off-white `#EDEFEA`, teal shadow `#123A3A`, accent variants — Vegas orange
  `#E08C2A`, Joi magenta `#C86AD8`, Wallace yellow `#D8C24A`.
- **Tags:** `\b0\fsp{22% of size}` (extreme tracking is the whole look),
  `\1c&HEAEFED&\3c&H3A3A12&\bord5\blur3` — a *teal-black* halo rather than pure black, which
  is what makes it read as graded rather than as a subtitle. One accent word or the date line
  in the chosen hue at `\fs{28%}`.
- **Widgets:** all three. The visualiser as thin, wide-gapped, un-rounded teal bars at low
  opacity is exactly the film's UI language.
- **Legibility:** **best in this document.** Near-white type, wide tracking, dark cool halo —
  this is the subtitle-guidelines answer wearing a costume. If only one theme ships, this is
  the one that can be the *default*.

---

### T7 — **Neon Kanji**
*A wet street full of signs. Dense, saturated, layered.*

- **Buildable now**, and notable for needing **zero font bundling** — Noto Sans CJK is already
  installed.
- **Reference:** the 1982 street signage language (neon in "saturated blues, reds and greens"
  over desaturated ground); the fake-neon layer stack (core + tube + bloom).
- **Font:** `Noto Sans CJK JP` (SIL OFL) — **stock on the target machine.** Latin runs stay in
  the same family for consistency.
- **Palette:** tube magenta `#FF2D95`, tube cyan `#00E5FF`, core `#FFFFFF`, bed `#0C0410`.
- **Tags:** the full three-layer neon stack from technique (b), plus a small vertical CJK
  label as a *second* `\pos`'d event at `\frz0\fsp{20%}` in the opposite tube colour. Two hues
  in one widget is what makes it read as signage rather than as one glowing word.
- **Widgets:** lyrics (primary — a lyric line as a neon sign is the strongest single image in
  this plan), clock.
- **Legibility:** good with the dark bed, and the decorative CJK label is explicitly *not*
  required to be legible — it is ornament, sized and dimmed accordingly. Do not put
  information in it.

---

### T8 — **HUD**
*Corner brackets, tracked caps, a segmented meter. Your desktop as a heads-up display.*

- **Buildable now**, and it is the cheapest striking theme in the document: the brackets are
  eight line segments.
- **Reference:** the shared vocabulary of game HUDs — corner frame ticks, segmented meters,
  all-caps condensed labels with wide tracking, a left accent rail.
- **Font:** `Inter` bold, all-caps — **stock.** (`Rajdhani` or `Chakra Petch`, both OFL, are
  the upgrade and would sharpen it considerably.)
- **Palette:** accent `#43E8C3`, ink `#EAFFFA`, rail `#0F1A18`, bed `#050A09` — or simply
  **follow the app accent**, which is what this theme should default to; the `accent_hex`
  table in `src/daemon/mod.rs` already provides six.
- **Tags:** a `\p1` event drawing four corner brackets and a 3-unit left rail in the accent,
  then the text event with `\b1\fsp{12%}\1c&HFAFFEA&\3c&H090A05&\bord4\shad0`. The visualiser
  becomes a **notched meter**: `Cap::Square`, wide `gap_px`, plus a `\p1` outline track — a
  small extension to `Path` in `src/visualizer.rs`, not a new module.
- **Widgets:** all three, and this is the only theme that makes the three read as *one system*
  — same brackets, same rail, same accent.
- **Legibility:** very good. The rail and brackets are decoration; the text carries a
  conventional dark stroke. The plate variant (a `\p1` rect at `\1a&H60&` behind the text) is
  the most legible configuration available anywhere in this document.

---

### T9 — **Arcade**
*Pixel type, one-pixel shadow, three colours. Insert coin.*

- **Buildable now**, with one caveat below.
- **Reference:** arcade marquee and 8-bit title-screen conventions — outlined pixel letterforms
  and a chunky shadow offset by exactly one design pixel.
- **Font:** `Press Start 2P` (SIL OFL, [Google Fonts](https://fonts.google.com/specimen/Press+Start+2P))
  or `Silkscreen` (SIL OFL) — **needs bundling.** No stock fallback exists; a pixel theme in
  Inter is not a pixel theme, so if bundling is refused, **cut this theme rather than
  degrade it.**
- **Palette:** ink `#FFFFFF`, shadow `#E4003A`, outline `#101018`, accent `#FFD400`.
- **Tags:** `\bord3\blur0\3c&H181010&` (hard, *unblurred* — blur destroys pixel type),
  `\xshad4\yshad4\4c&H3A00E4&\4a&H00&` for the offset block shadow, `\fsp0`.
- **Widgets:** clock (primary), lyrics. Visualiser as square-capped bars with zero rounding.
- **Caveat:** pixel fonts only look right at **integer multiples of their design size**. The
  size control must snap to a multiple for this theme, or the glyphs interpolate into mush.
  That is a real constraint on the shared size slider and the first place the "one theme sets
  everything" model has to actually override a user value.
- **Legibility:** good at large sizes, poor below ~24pt. Enforce a higher minimum for this
  theme specifically.

---

### T10 — **Cutout** *(Persona-style)*
*Red, black, white, and everything at an angle.*

- **Buildable now** for the typography; the paper texture is **not buildable, ever**, and the
  theme is designed around that absence rather than apologising for it.
- **Reference:** the Persona 5 UI language — red/black/white, extreme diagonals, heavy italic
  condensed type, ransom-note irregularity.
- **Font:** `Anton` or `Archivo Black` (both SIL OFL) — **needs bundling.** Stock fallback:
  `Inter` Black with `\fscx88` to condense. Works, less punchy.
- **Palette:** red `#E60012`, ink `#0B0B0B`, paper `#F5F2EA`.
- **Tags:** the interesting one. Per-character override blocks give the hand-cut jitter —
  `{\frz-4}A{\frz3\fscy108}B{\frz-2}C…` — plus a whole-line `\fax0.2` shear and `\frz-6`
  rotation. A `\p1` red slash quad behind the text as event 1. `\1c` ink on `\3c` paper with a
  fat `\bord`, inverting the usual light-on-dark.
- **Widgets:** lyrics (it wants running text), clock as the numerals only.
- **Legibility:** **dark-on-light is the wrong polarity for video**, and this theme knowingly
  chooses it because the reference demands it. Mitigation: the paper `\bord` must be genuinely
  fat (≥8% of size) so it functions as a plate rather than an outline, and the whole thing
  should sit on a `\p1` red quad, which then *is* a plate. With the quad it is fine; without
  it, it fails over bright footage. **Ship it with the quad mandatory.**

---

### Marked "Needs W2 surface" — proposed and rejected for now

Recorded so they are not re-proposed:

- **VHS tracking** — the rolling distortion band is animation. Static, it is just a smear.
- **Karaoke word-fill** — `\kf` never advances (mpv renders at t=0). Already documented in
  `lyrics.rs`.
- **Album-art disc / vinyl** — bitmaps, which ASS does not have at all.
- **Synthwave sunset gradient** — no gradients. The flat-band approximation looked like a flag.
- **Achievement toast slide-in / XP bar fill** — animation.
- **Neon buzz / CRT flicker** — animation.
- **Contrast-adaptive text colour** — needs per-frame read-back; contradicts the power model
  and flickers.

---

## 4. How the user should meet all this

The maintainer's ask is "make the user feel easy to set up". Today it is the opposite of easy,
and the numbers say so plainly. `add_lyrics_group` in `src/gui/window.rs` builds a master
switch plus **eight** rows; `add_clock_group` builds a master switch plus **eight** more. That
is **eighteen controls, two independent style pickers, two independent position pickers and
two separate "follow accent" switches**, and the visualiser has not shipped its group yet. A
user who wants "make it look like Blade Runner" currently has to derive it from a combo box
labelled *Style* and a spin button labelled *Text size*.

### The proposal: one picker, one size, one position, one switch per widget

**Replace both Style/Theme combos with a single visual theme picker shared by all widgets.**
A theme is a bundle: it sets family, weight, tracking, fill, outline, glow, plate and the
visualiser's bar geometry together. That is already the bargain `LyricStylePreset` and
`ClockTheme` each make privately — the change is making it **one** bargain across the widget
layer instead of three.

**Structure.** The GUI already has exactly the right pattern for this: the library grid built
around `gtk4::FlowBox` with poster cards (`library_card`, and the hover-video preview in
`super::hover_preview`). Reuse it.

```
Widgets                                            [preferences page]
├─ Show widgets on my wallpaper            [switch]   ← ONE master switch
│
├─ Theme                                              ← adw::PreferencesGroup
│   └─ FlowBox of theme cards (2–3 per row)
│       each card: a 16:9 still of THIS theme rendered over a real
│       wallpaper frame, the theme name, and a "font not installed"
│       badge when `fc-list "<family>" family` comes back empty
│   └─ [ Surprise me ]  button in the group header
│
├─ Position                                [3×3 grid]  ← ONE grid, not two combos
│   └─ Clock and lyrics get sensible *offsets* within the chosen corner
│      automatically; the theme decides the relationship.
│
├─ Size                                    [slider]    ← ONE slider, S / M / L / XL
│
└─ What to show                                        ← three switches, nothing else
    ├─ Clock                               [switch]
    ├─ Lyrics                              [switch]
    └─ Visualiser                          [switch]
```

That is **seven controls total**, down from eighteen-plus.

**Live preview.** The theme cards should not be shipped screenshots. Render each card by
pushing the theme's ASS to a hidden mpv instance over a still frame of the user's *current
wallpaper* — Fresco already spawns and controls mpv, already has the thumbnailer
(`ffmpegthumbnailer` is in the Flatpak manifest), and already fades thumbnails in on first map
in `library_card`. Failing that, a static PNG per theme rendered at build time is an
acceptable v1; a wrong-looking preview is worse than a generic one.

**"Surprise me."** Picks a random theme weighted by legibility rating, applies it immediately,
and leaves the picker open. This is the single cheapest thing on this list and it is how most
people will actually discover the good themes.

### What I would specifically REMOVE

From `add_lyrics_group`:

| Row | Verdict |
|---|---|
| `style_row` (Style combo) | **Remove.** Replaced by the theme picker. |
| `anchor_row` (Position combo) | **Remove.** Replaced by the shared 3×3 grid. |
| `size_row` (Text size spin) | **Remove.** Replaced by the shared S/M/L/XL slider. |
| `margin_row` (Margin spin) | **Move to Advanced.** Real need (panels, docks) but not a first-run decision. |
| `accent_row` (Follow accent) | **Remove.** Themes that follow the accent say so; themes with an authored palette should not be overridable into ugliness. Keep the behaviour, delete the switch. |
| `next_row` (Show next line) | **Keep**, reworded. It changes *content*, not appearance. |
| `offset_row` (Sync offset) | **Keep.** The error is in the `.lrc` data; nothing else can fix it. |
| `folder_row` (Lyrics folder) | **Keep.** Functional, not cosmetic. |
| `enable` | **Merge** into the "What to show → Lyrics" switch. |

From `add_clock_group`:

| Row | Verdict |
|---|---|
| `theme_row` | **Remove.** Theme picker. |
| `anchor_row`, `size_row` | **Remove.** Shared controls. |
| `margin_row` | **Move to Advanced.** |
| `accent_row` | **Remove**, as above. |
| `hour_row` (24-hour) | **Keep.** Locale/preference, not styling. |
| `date_row` (Show date) | **Keep**, but the existing "Minimal never shows one and Stacked always does" caveat becomes a theme property and the row should grey out rather than lie. |
| `seconds_row` (Show seconds) | **Keep**, and keep the power warning verbatim — it is the only row in the dialog that changes how often Fresco wakes the machine, and `clock::tick_secs` enforces the difference. |
| `enable` | **Merge** into "What to show → Clock". |

Net: **eleven cosmetic controls deleted, five functional ones kept, two moved to Advanced.**
Everything removed is subsumed by a theme, and every theme is a tested constant in the
renderer — which also means the hostile-config tests in `lyrics.rs`, `clock.rs` and
`visualizer.rs` keep protecting a *smaller* surface of user-supplied values, not a larger one.

**One caution.** The config keys should **not** be deleted, only the GUI rows. `config.toml`
is hand-editable and documented; `Lyrics::font_size_pt` and friends should keep parsing and
keep working for anyone who has set them. The theme sets them; a hand-edited file may still
override them. That is the same contract `ClockStyle` already honours through serde defaults.

---

## 5. What I would ship first

The maintainer has limited time. One striking theme beats six mediocre ones, and the ordering
below is by *(visual impact) ÷ (work + risk)*, not by how much I like them.

**1. T6 Wallace.** Ship this first and ship it as the **default**. It needs **no font
bundling** (Inter is installed), it is the most legible theme in the document, it is the
best-looking one, and it is achievable by changing three constants and adding `\fsp` to the
lyric renderer. It is roughly an afternoon. If nothing else in this plan happens, Fresco is
still visibly better.

**2. T1 Phosphor.** Second because it is nearly free: `ClockTheme::Segment` in `src/clock.rs`
already emits `\bord`+`\blur` with `\3c` = `\1c` and already uses DejaVu Sans Mono. What is
missing is the dark under-halo (which needs the multi-event payload, technique a) and the
amber/green variant pair. High delight, small diff, no bundling.

**3. The multi-event payload itself.** Not a theme — the *enabling change*. Relaxing the
"no newlines" invariant in `lyrics::render_ass` and `clock::render_ass`, with every event
fully specifying its own tags and a test asserting exactly that, is what unlocks T3, T4, T7
and T10. Do it once, properly, with the same paranoia the existing renderers already show
about hostile input. **This is the highest-leverage item on the list and it is not a feature.**

**4. T8 HUD.** First theme that makes all three widgets read as one system, uses stock Inter,
and introduces the `\p1` bracket/rail primitive that T3's grid and T10's slash quad both reuse.
Also the natural home for the accent-follow behaviour we are deleting the switch for.

**5. The GUI collapse (§4).** Deliberately *after* three or four themes exist. Building a
visual theme picker with one theme in it is wasted work, and the picker's shape depends on
what themes actually turn out to need (T9's integer-size snap is the example — you cannot
design the size control until you know one theme constrains it).

**6. T3 Outrun** and **7. T7 Neon Kanji.** The two loudest themes, both gated on item 3.
Outrun needs Orbitron bundled; Neon Kanji needs nothing. **Ship Neon Kanji before Outrun**
purely because it has no packaging dependency — that inverts the order you would guess from
looking at them.

**8. T5 Spinner**, **9. T4 Nixie**, **10. T2 Cathode**, **11. T9 Arcade**, **12. T10 Cutout.**
Each is gated on bundling a font (Michroma / DSEG / — / Press Start 2P / Anton) or, for
Cathode, on the `\iclip` mask primitive. All are genuinely good; none is worth doing before
the five items above.

**Do the font bundling as one change, not five.** A single `fonts` module in the Flatpak
manifest and one `/usr/share/fonts/fresco/` directory in the deb and AUR recipes, carrying
Orbitron + VT323 + DSEG + Press Start 2P + Michroma + Anton, is one packaging review across
three targets. Doing it per-theme means paying that review five times. Total weight is under
2 MB and all six are SIL OFL.
