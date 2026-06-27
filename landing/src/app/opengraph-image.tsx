import { ImageResponse } from "next/og";

export const alt = "Fresco - Live wallpapers for Linux";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

/**
 * Dynamic OpenGraph card in the Linear-style dark system: near-black canvas,
 * a single lavender-blue accent, system fonts only (no external fetch).
 * No em-dashes in any text.
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
          backgroundColor: "#010102",
          color: "#f7f8f8",
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
              background: "#5e6ad2",
            }}
          />
          <div style={{ display: "flex", fontSize: 30, color: "#d0d6e0" }}>
            Fresco
          </div>
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 80,
            fontWeight: 700,
            letterSpacing: "-0.035em",
            lineHeight: 1.05,
            marginTop: 28,
            maxWidth: 940,
          }}
        >
          Finally, live wallpapers that just work on Linux.
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 33,
            color: "#8a8f98",
            marginTop: 26,
            maxWidth: 960,
          }}
        >
          Video, GIF, image, slideshow, and playlist wallpapers. Hardware
          accelerated, near zero CPU, on X11 and Wayland.
        </div>

        <div
          style={{
            display: "flex",
            fontSize: 24,
            color: "#828fff",
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
