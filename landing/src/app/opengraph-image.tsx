import { ImageResponse } from "next/og";

export const alt = "Fresco - Live wallpaper, decoded.";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

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
          fresco operator console · gpl-3.0 · linux
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 88,
            fontStyle: "italic",
            color: "#38bdf8",
            fontFamily: "ui-serif, Georgia, serif",
            lineHeight: 1.04,
            letterSpacing: "-0.02em",
            marginTop: 28,
            maxWidth: 1000,
          }}
        >
          Live wallpaper, decoded.
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 32,
            color: "#a1a1a1",
            marginTop: 26,
            maxWidth: 980,
            lineHeight: 1.3,
          }}
        >
          Nine missions talk directly to the page itself. Scroll, cast the
          install, flip the theme, decoder any question.
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 16,
            marginTop: 56,
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
            boot / specs / brief / ritual / init / cast / lore / night / send
          </div>
        </div>
      </div>
    ),
    { ...size },
  );
}