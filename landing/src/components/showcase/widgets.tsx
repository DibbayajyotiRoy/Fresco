"use client";

import { Fragment, memo, type CSSProperties } from "react";
import {
  DISC,
  GLYPH_DOTS,
  INK,
  NOS,
  NP,
  VIZ,
  clockLabel,
  clockTime,
  dayRemaining,
  glyphAdvance,
  litDots,
  lu,
  textRow,
} from "@/components/showcase/desk";

/*
 * DOM replicas of Fresco's four on-wallpaper widgets, drawn the way
 * src/widgetkit draws them: same geometry (desk.ts), same dark palette, same
 * layer order. Pure decoration; the viewscreen that holds them is aria-hidden.
 */

type Vars = CSSProperties & Record<`--${string}`, string>;

/* ---- NOS clock ---------------------------------------------------------- */

function DotMatrix({ text }: { text: string }) {
  let x = 0;
  const glyphs = [...text].map((ch, i) => {
    const at = x;
    x += glyphAdvance(ch);
    const id = ch === ":" ? "c" : ch;
    return GLYPH_DOTS[ch] ? <use key={i} href={`#sc-dm-${id}`} x={at} /> : null;
  });
  return (
    <svg
      viewBox={`0 0 ${NOS.cols} 7`}
      className="absolute"
      style={{
        left: lu(NOS.heroX),
        top: lu(NOS.heroY),
        width: lu(NOS.cols * NOS.pitch),
        height: lu(7 * NOS.pitch),
      }}
      fill={INK.primary}
    >
      <defs>
        {Object.entries(GLYPH_DOTS).map(([ch, dots]) => (
          <g key={ch} id={`sc-dm-${ch === ":" ? "c" : ch}`}>
            {dots.map(([c, r]) => (
              <circle key={`${c}-${r}`} cx={c + 0.5} cy={r + 0.5} r={0.39} />
            ))}
          </g>
        ))}
      </defs>
      {glyphs}
    </svg>
  );
}

/**
 * The NOS clock: a squircle, a ring of dots carrying the day's progress, the
 * weekday and date, dot-matrix numerals and the "left today" caption with
 * its legend dot. `now` is null on the server and until the screen is first
 * seen, which draws the card and an unlit ring without a time.
 */
export function NosClock({ now }: { now: Date | null }) {
  const lit = now ? litDots(now) : 0;
  const label = now ? clockLabel(now) : null;
  return (
    <div className="relative" style={{ width: lu(NOS.side), height: lu(NOS.side) }}>
      <div className="sc-e2 absolute inset-0" style={{ borderRadius: lu(NOS.radius) }} />
      <svg
        viewBox={`0 0 ${NOS.side} ${NOS.side}`}
        className="absolute inset-0 size-full overflow-visible"
      >
        <defs>
          <linearGradient id="sc-nos-fill" gradientUnits="userSpaceOnUse" {...NOS.fill}>
            {/* Gradient with the scrim composited in, over the E2 shadow. */}
            <stop offset="0" stopColor="#0b0d13" stopOpacity={0.95} />
            <stop offset="1" stopColor="#06080c" stopOpacity={0.95} />
          </linearGradient>
        </defs>
        <path
          d={NOS.squircle}
          fill="url(#sc-nos-fill)"
          stroke="rgba(255,255,255,0.14)"
          strokeWidth={1}
          vectorEffect="non-scaling-stroke"
        />
        {NOS.ring.map((p, i) => {
          const d = i + 1 === lit ? NOS.head : i < lit ? NOS.dot : NOS.unlit;
          return (
            <circle
              key={i}
              cx={p.x}
              cy={p.y}
              r={Math.round(d * 500) / 1000}
              fill={i < lit ? INK.nosRed : INK.nosDim}
            />
          );
        })}
      </svg>

      {label && (
        // Date first in the DOM, drawn last: when "WEEKDAY · DATE" is wider
        // than the ring's chord the weekday wraps onto a clipped second line
        // and the date survives alone, as `fit_micro` decides.
        <div
          className="sc-nos-micro absolute"
          style={{
            ...textRow({
              x: NOS.side / 2 - NOS.microW / 2,
              capTop: NOS.microY,
              size: NOS.micro,
              weight: 600,
              micro: true,
              width: NOS.microW,
              color: INK.tertiary,
            }),
          }}
        >
          <span>{label.date}</span>
          <span>{`${label.weekday} · `}</span>
        </div>
      )}

      {now && <DotMatrix text={clockTime(now)} />}

      {now && (
        <div
          className="absolute inset-x-0 flex items-center justify-center"
          style={{
            top: lu(NOS.captionY - (0.5 - 0.3638) * NOS.micro),
            height: lu(NOS.micro),
          }}
        >
          <span
            className="shrink-0 rounded-full"
            style={{
              width: lu(NOS.dot),
              height: lu(NOS.dot),
              marginRight: lu(4),
              background: INK.nosRed,
            }}
          />
          <span
            className="truncate"
            style={{
              ...textRow({ capTop: 0, size: NOS.micro, weight: 500, color: INK.secondary }),
              top: undefined,
              // A taller line box, centred like the short one, so "today"
              // keeps its descender inside the truncating box.
              lineHeight: 1.4,
              maxWidth: lu(NOS.captionW - 2 * NOS.dot),
            }}
          >
            {dayRemaining(now)}
          </span>
        </div>
      )}
    </div>
  );
}

/* ---- Now playing -------------------------------------------------------- */

/**
 * The lyric card as the daemon feeds it: label, title, artist, the current
 * line and the next one. The daemon passes no artwork, position or chip to
 * this card, so the art tile is the drawn note on a well with the empty
 * source badge over its corner, and there is no progress row.
 */
export const NowPlaying = memo(function NowPlaying({
  lyric,
  next,
}: {
  lyric: string;
  next: string;
}) {
  const b = NP.badge;
  return (
    <div className="relative" style={{ width: lu(NP.w), height: lu(NP.h) }}>
      <div className="sc-card" style={{ borderRadius: lu(NP.radius) }} />
      <div
        className="absolute"
        style={{
          inset: lu(2),
          borderRadius: lu(NP.radius - 2),
          backgroundImage: NP.scrim,
          backgroundOrigin: "border-box",
        }}
      />

      <div
        className="sc-well-e1 absolute"
        style={{
          left: lu(NP.pad),
          top: lu(NP.pad),
          width: lu(NP.art),
          height: lu(NP.art),
          borderRadius: lu(NP.artRadius),
        }}
      >
        <svg
          viewBox={`0 0 ${NP.art} ${NP.art}`}
          className="absolute inset-0 size-full"
          fill={INK.tertiary}
        >
          {NP.note.map((q, i) => (
            <rect key={i} x={q.x} y={q.y} width={q.w} height={q.h} rx={q.r} />
          ))}
        </svg>
      </div>
      {/* Badge cutout, then the badge: no icon and no name, as on the desktop. */}
      <div
        className="absolute rounded-full"
        style={{
          left: lu(NP.badgeC - b / 2 - 2),
          top: lu(NP.badgeC - b / 2 - 2),
          width: lu(b + 4),
          height: lu(b + 4),
          background: "rgb(23 27 36 / 0.72)",
        }}
      />
      <div
        className="sc-well-e1 absolute rounded-full"
        style={{
          left: lu(NP.badgeC - b / 2),
          top: lu(NP.badgeC - b / 2),
          width: lu(b),
          height: lu(b),
        }}
      />

      <span
        className="sc-line"
        style={textRow({
          x: NP.colX,
          capTop: NP.labelY,
          size: 11,
          weight: 600,
          micro: true,
          lh: 1.4,
          width: NP.colW,
          color: INK.tertiary,
        })}
      >
        NOW PLAYING
      </span>
      <span
        className="sc-line"
        style={textRow({
          x: NP.colX,
          capTop: NP.titleY,
          size: NP.title,
          weight: 600,
          lh: 1.4,
          width: NP.colW,
          color: INK.primary,
        })}
      >
        Night Drive
      </span>
      <span
        className="sc-line"
        style={textRow({
          x: NP.colX,
          capTop: NP.artistY,
          size: NP.body,
          weight: 500,
          lh: 1.4,
          width: NP.colW,
          color: INK.secondary,
        })}
      >
        Fresco Demo
      </span>

      <div
        className="sc-hair absolute"
        style={{
          left: lu(NP.pad),
          top: lu(NP.dividerY),
          width: lu(NP.w - 2 * NP.pad),
          background: "rgb(255 255 255 / 0.1)",
        }}
      />

      {/* One line each: every placeholder line fits the card's width, so the
          renderer's two-line wrap never engages and the height above holds. */}
      <span
        className="sc-line"
        style={textRow({
          x: NP.pad,
          capTop: NP.lyricY,
          size: NP.L,
          weight: 500,
          lh: 1.4,
          width: NP.w - 2 * NP.pad,
          color: INK.primary,
        })}
      >
        {lyric}
      </span>
      <span
        className="sc-line"
        style={textRow({
          x: NP.pad,
          capTop: NP.nextY,
          size: NP.L,
          weight: 500,
          lh: 1.4,
          width: NP.w - 2 * NP.pad,
          color: INK.tertiary,
        })}
      >
        {next}
      </span>
    </div>
  );
});

/* ---- Visualiser --------------------------------------------------------- */

/**
 * The panel treatment (width at 40%, under the 45% bare threshold): glass
 * card, a sunk well, 32 rainbow bars with rounded caps and peak caps, and the
 * gridline floor. Bars and caps are CSS transform loops (showcase.css),
 * paused unless the screen is on screen and motion is allowed.
 */
export const Visualiser = memo(function Visualiser() {
  const { area } = VIZ;
  return (
    <div className="relative" style={{ width: lu(VIZ.w), height: lu(VIZ.h) }}>
      <div className="sc-card" style={{ borderRadius: lu(VIZ.radius) }} />
      <div
        className="sc-well absolute"
        style={{
          inset: lu(VIZ.bed),
          borderRadius: lu(VIZ.bedRadius),
        }}
      />
      <div
        className="absolute overflow-hidden"
        style={{
          left: lu(area.x),
          top: lu(area.y),
          width: lu(area.w),
          height: lu(area.h),
        }}
      >
        {VIZ.bars.map((b, i) => {
          const common = { left: lu(b.x), width: lu(VIZ.bw), "--d": b.dur, "--dl": b.delay };
          return (
            <Fragment key={i}>
              <span
                className="sc-bar sc-anim"
                style={
                  {
                    ...common,
                    borderRadius: lu(3),
                    background: b.color,
                    "--k0": b.bar[0],
                    "--k1": b.bar[1],
                    "--k2": b.bar[2],
                    "--k3": b.bar[3],
                  } as Vars
                }
              />
              <span
                className="sc-cap sc-anim"
                style={
                  {
                    ...common,
                    height: lu(2),
                    borderRadius: lu(1),
                    "--k0": b.cap[0],
                    "--k1": b.cap[1],
                    "--k2": b.cap[2],
                    "--k3": b.cap[3],
                  } as Vars
                }
              />
            </Fragment>
          );
        })}
      </div>
      <div
        className="sc-hair absolute"
        style={{
          left: lu(area.x),
          top: lu(area.y + area.h),
          width: lu(area.w),
          background: "rgb(255 255 255 / 0.09)",
        }}
      />
    </div>
  );
});

/* ---- Disc --------------------------------------------------------------- */

/**
 * The record: artwork turning at 33 1/3 rpm with the spindle hole punched
 * through it; rim darkening, grooves and the paper label are radially
 * symmetric and the rim bevel and specular sweep are fixed to the room, so
 * all of those sit still on top and only the artwork turns.
 */
export const Disc = memo(function Disc() {
  const { R } = DISC;
  return (
    <div className="relative" style={{ width: lu(DISC.size), height: lu(DISC.size) }}>
      <div className="sc-e3 absolute inset-0 rounded-full" />
      <div className="sc-disc-art sc-anim absolute inset-0 rounded-full" />
      <svg
        viewBox={`0 0 ${DISC.size} ${DISC.size}`}
        className="absolute inset-0 size-full overflow-visible"
        fill="none"
      >
        <defs>
          <radialGradient id="sc-disc-rim" cx={R} cy={R} r={R} gradientUnits="userSpaceOnUse">
            <stop offset="0.82" stopColor="#000" stopOpacity={0} />
            <stop offset="1" stopColor="#000" stopOpacity={0.35} />
          </radialGradient>
          <linearGradient id="sc-disc-hi" gradientUnits="userSpaceOnUse" x1={0} y1={0} x2={DISC.size} y2={DISC.size}>
            <stop offset="0" stopColor="#fff" stopOpacity={0.22} />
            <stop offset="1" stopColor="#fff" stopOpacity={0} />
          </linearGradient>
          <linearGradient id="sc-disc-lo" gradientUnits="userSpaceOnUse" x1={DISC.size} y1={DISC.size} x2={0} y2={0}>
            <stop offset="0" stopColor="#000" stopOpacity={0.35} />
            <stop offset="1" stopColor="#000" stopOpacity={0} />
          </linearGradient>
          <linearGradient id="sc-disc-spec" gradientUnits="userSpaceOnUse" x1={0} y1={0} x2={DISC.size * 0.45} y2={DISC.size * 0.45}>
            <stop offset="0" stopColor="#fff" stopOpacity={0.1} />
            <stop offset="1" stopColor="#fff" stopOpacity={0} />
          </linearGradient>
          <filter id="sc-disc-e1" x="-20%" y="-20%" width="140%" height="140%">
            <feDropShadow dx={0} dy={1} stdDeviation={1.5} floodColor="#000" floodOpacity={0.28} />
          </filter>
        </defs>
        <circle cx={R} cy={R} r={R} fill="url(#sc-disc-rim)" />
        {DISC.grooves.map((g) => (
          <Fragment key={g}>
            <circle cx={R} cy={R} r={g} stroke="rgba(0,0,0,0.1)" strokeWidth={1} />
            <circle cx={R} cy={R} r={g + 1} stroke="rgba(255,255,255,0.05)" strokeWidth={1} />
          </Fragment>
        ))}
        <path d={DISC.label} fill="rgba(4,6,10,0.55)" fillRule="evenodd" filter="url(#sc-disc-e1)" />
        <circle
          cx={R}
          cy={R}
          r={DISC.labelR}
          stroke="rgba(255,255,255,0.14)"
          strokeWidth={1}
          vectorEffect="non-scaling-stroke"
        />
        <path d={DISC.bevelHi} stroke="url(#sc-disc-hi)" strokeWidth={1} />
        <path d={DISC.bevelLo} stroke="url(#sc-disc-lo)" strokeWidth={1} />
        <circle cx={R} cy={R} r={R} fill="url(#sc-disc-spec)" />
        <circle cx={R} cy={R} r={DISC.holeR + 0.5} stroke="rgba(0,0,0,0.4)" strokeWidth={1} />
      </svg>
    </div>
  );
});
