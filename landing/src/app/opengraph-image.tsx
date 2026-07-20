import { ImageResponse } from "next/og";

export const alt = "Fresco - Finally, a Linux wallpaper that just works.";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

/**
 * OpenGraph card in the Warm Terminal system: stone-950 paper, single sky
 * accent, serif display headline (italic sky clause matching the live hero),
 * Inter subhead, mono eyebrow. System fonts only; no external fetches.
 */
export default function Image() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          padding: "80px",
          backgroundColor: "#0c0a09",
          color: "#e5e5e5",
          fontFamily: "system-ui, sans-serif",
          borderTop: "6px solid #38bdf8",
        }}
      >
        <div
          style={{
            display: "flex",
            fontSize: 22,
            letterSpacing: "0.18em",
            textTransform: "uppercase",
            color: "#8a8a8a",
            fontFamily:
              "ui-monospace, SFMono-Regular, Menlo, Monaco, monospace",
          }}
        >
          fresco · gpl-3.0 · linux
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 80,
            color: "#e5e5e5",
            fontFamily: "ui-serif, Georgia, serif",
            lineHeight: 1.06,
            letterSpacing: "-0.02em",
            marginTop: 28,
            maxWidth: 1000,
          }}
        >
          Finally, a Linux wallpaper{" "}
          <span
            style={{
              fontStyle: "italic",
              color: "#38bdf8",
              marginLeft: 14,
            }}
          >
            that just works.
          </span>
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 30,
            color: "#a1a1a1",
            marginTop: 28,
            maxWidth: 980,
            lineHeight: 1.3,
          }}
        >
          Set any video, GIF, or image as your desktop. Hardware-accelerated,
          near zero CPU, on X11 and Wayland. Close the app; the daemon keeps
          it playing.
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 16,
            marginTop: 48,
          }}
        >
          <div
            style={{
              display: "flex",
              fontSize: 22,
              letterSpacing: "0.18em",
              textTransform: "uppercase",
              color: "#38bdf8",
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, monospace",
            }}
          >
            x11 / wayland / multi-monitor / day + night / gpl-3.0
          </div>
        </div>
      </div>
    ),
    { ...size },
  );
}