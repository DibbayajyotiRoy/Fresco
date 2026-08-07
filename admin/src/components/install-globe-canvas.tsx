"use client";

import * as React from "react";
import Globe, { type GlobeMethods } from "react-globe.gl";
import { Color, MeshPhongMaterial } from "three";

import { COUNTRIES, countryName } from "@/lib/geo";
import { formatNumber } from "@/lib/format";
import type { CountryCount } from "@/components/install-globe";

/**
 * The WebGL half of the installs globe. Split from `install-globe.tsx` so the
 * parent can load it through `next/dynamic({ ssr: false })` while this file
 * keeps a plain static import of react-globe.gl — `next/dynamic` does not
 * forward refs, and the camera and orbit controls are only reachable through
 * one.
 */

type Feature = {
  type: "Feature";
  id: string;
  properties: { iso2: string; name: string };
};

/** Tooltip markup. Escaped because a country name is interpolated into it. */
function tooltip(name: string, count: number) {
  const safe = name.replace(
    /[&<>"]/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string
  );
  return `<div style="font:500 12px system-ui;padding:4px 8px;border-radius:6px;background:#1c1917;color:#fafaf9;white-space:nowrap">${safe} — ${formatNumber(count)} install${count === 1 ? "" : "s"}</div>`;
}

export default function InstallGlobeCanvas({
  counts,
  size,
  dark,
  onHover,
}: {
  counts: CountryCount[];
  size: number;
  dark: boolean;
  onHover: (code: string | null) => void;
}) {
  const globeRef = React.useRef<GlobeMethods | undefined>(undefined);
  const [features, setFeatures] = React.useState<Feature[]>([]);

  // Polygons live in /public rather than in the bundle: 170 KB of coordinates
  // that only this panel needs has no business in the JS every page loads.
  React.useEffect(() => {
    let live = true;
    fetch("/geo/countries.geojson")
      .then((r) => r.json())
      .then((json) => {
        if (live) setFeatures(json.features as Feature[]);
      })
      .catch(() => {
        // A globe with no landmasses is still a globe with markers on it.
      });
    return () => {
      live = false;
    };
  }, []);

  const byCode = React.useMemo(() => {
    const map = new Map<string, number>();
    for (const c of counts) map.set(c.code, c.count);
    return map;
  }, [counts]);

  const max = React.useMemo(
    () => Math.max(1, ...counts.map((c) => c.count)),
    [counts]
  );

  const markers = React.useMemo(
    () =>
      counts
        .map(({ code, count }) => {
          const info = COUNTRIES[code];
          if (!info || info.lat === null || info.lon === null) return null;
          return {
            code,
            count,
            lat: info.lat,
            lng: info.lon,
            name: countryName(code),
          };
        })
        .filter((m): m is NonNullable<typeof m> => m !== null),
    [counts]
  );

  const palette = React.useMemo(
    () =>
      dark
        ? {
            surface: "#1c1917",
            atmosphere: "#38bdf8",
            empty: "rgba(87, 83, 78, 0.6)",
            side: "rgba(56, 189, 248, 0.2)",
            stroke: "#292524",
            marker: "#7dd3fc",
            lit: (t: number) => `rgba(56, 189, 248, ${t.toFixed(3)})`,
          }
        : {
            surface: "#fafaf9",
            atmosphere: "#0284c7",
            empty: "rgba(214, 211, 209, 0.85)",
            side: "rgba(2, 132, 199, 0.18)",
            stroke: "#e7e5e4",
            marker: "#0369a1",
            lit: (t: number) => `rgba(2, 132, 199, ${t.toFixed(3)})`,
          },
    [dark]
  );

  // A plain matte sphere. The alternative is a satellite texture, which on a
  // dashboard is decoration that competes with the only thing being encoded —
  // which countries are lit, and how brightly.
  const globeMaterial = React.useMemo(() => {
    const material = new MeshPhongMaterial();
    material.color = new Color(palette.surface);
    material.shininess = 0;
    return material;
  }, [palette.surface]);

  React.useEffect(() => () => globeMaterial.dispose(), [globeMaterial]);

  // Open over the busiest country rather than over the Atlantic, then drift.
  React.useEffect(() => {
    const globe = globeRef.current;
    if (!globe) return;
    const controls = globe.controls();
    controls.autoRotate = true;
    controls.autoRotateSpeed = 0.4;
    // Zoom is off on purpose: this panel is a fixed-size readout inside a
    // scrolling page, and a wheel event over it should scroll the page.
    controls.enableZoom = false;
    if (markers.length === 0) return;
    const busiest = markers.reduce((a, b) => (b.count > a.count ? b : a));
    globe.pointOfView(
      { lat: busiest.lat, lng: busiest.lng, altitude: 2.1 },
      1200
    );
  }, [markers]);

  const capColor = React.useCallback(
    (feature: object) => {
      const count = byCode.get((feature as Feature).properties.iso2) ?? 0;
      if (count === 0) return palette.empty;
      // Square root, not linear: with one country at 5 and the rest at 1, a
      // linear ramp renders every minor country as effectively unlit.
      return palette.lit(0.4 + 0.6 * Math.sqrt(count / max));
    },
    [byCode, max, palette]
  );

  const polygonAltitude = React.useCallback(
    (feature: object) => {
      const count = byCode.get((feature as Feature).properties.iso2) ?? 0;
      return count === 0 ? 0.006 : 0.015 + 0.08 * Math.sqrt(count / max);
    },
    [byCode, max]
  );

  return (
    <Globe
      ref={globeRef}
      width={size}
      height={size}
      backgroundColor="rgba(0,0,0,0)"
      globeMaterial={globeMaterial}
      showGlobe
      showGraticules
      showAtmosphere
      atmosphereColor={palette.atmosphere}
      atmosphereAltitude={0.15}
      polygonsData={features}
      polygonCapColor={capColor}
      polygonSideColor={() => palette.side}
      polygonStrokeColor={() => palette.stroke}
      polygonAltitude={polygonAltitude}
      polygonsTransitionDuration={400}
      onPolygonHover={(f: object | null) =>
        onHover(f ? (f as Feature).properties.iso2 : null)
      }
      polygonLabel={(f: object) => {
        const code = (f as Feature).properties.iso2;
        const count = byCode.get(code) ?? 0;
        return count === 0 ? "" : tooltip(countryName(code), count);
      }}
      pointsData={markers}
      pointLat="lat"
      pointLng="lng"
      pointColor={() => palette.marker}
      pointAltitude={(d: object) =>
        0.025 + 0.09 * Math.sqrt((d as { count: number }).count / max)
      }
      pointRadius={(d: object) =>
        0.25 + 0.35 * Math.sqrt((d as { count: number }).count / max)
      }
      pointsMerge={false}
      pointLabel={(d: object) => {
        const m = d as { name: string; count: number };
        return tooltip(m.name, m.count);
      }}
    />
  );
}
