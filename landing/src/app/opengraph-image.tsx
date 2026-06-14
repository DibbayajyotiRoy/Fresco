import { ImageResponse } from "next/og";

export const alt = "Fresco - Live wallpapers for Linux";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

/**
 * Dynamic OpenGraph card. Dark ground with the app's coral-orange sunset glow,
 * system fonts only (no external fetch). No em-dashes in any text.
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
          backgroundColor: "#181618",
          backgroundImage:
            "radial-gradient(60% 70% at 75% 10%, rgba(215,109,119,0.34), transparent 60%), radial-gradient(50% 60% at 10% 100%, rgba(58,28,113,0.5), transparent 55%)",
          color: "#f3f2f8",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 18 }}>
          <div
            style={{
              display: "flex",
              width: 26,
              height: 26,
              borderRadius: 8,
              background: "linear-gradient(135deg, #3a1c71, #d76d77)",
            }}
          />
          <div style={{ display: "flex", fontSize: 30, color: "#c9c8d6" }}>
            Fresco
          </div>
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 84,
            fontWeight: 700,
            letterSpacing: "-0.03em",
            lineHeight: 1.05,
            marginTop: 28,
            maxWidth: 900,
          }}
        >
          Live wallpapers for Linux.
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 34,
            color: "#a6a6bd",
            marginTop: 24,
            maxWidth: 940,
          }}
        >
          Video, GIF, image, slideshow, and playlist wallpapers. Hardware
          accelerated, near zero CPU.
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 24,
            color: "#d76d77",
            marginTop: 40,
          }}
        >
          Free and open source. A Wallpaper Engine alternative.
        </div>
      </div>
    ),
    { ...size },
  );
}
