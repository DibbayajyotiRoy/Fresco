// Regenerates the two geography artefacts the Usage globe needs:
//
//   public/geo/countries.geojson   world polygons, tagged with ISO alpha-2
//   src/lib/countries.generated.ts alpha-2 -> { name, lat, lon }
//
// Run it when you want to refresh the source data; the outputs are committed,
// so a normal build never touches the network.
//
//   node scripts/build-geo.mjs
//
// Why generated rather than a runtime dependency: the globe needs country
// polygons keyed by the same two-letter code the telemetry stores, and no
// single package ships that. world-atlas keys by numeric ISO, so the numeric
// -> alpha-2 join has to happen somewhere. Doing it here means the browser
// downloads one already-joined file instead of three plus a topojson decoder.

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const SOURCES = {
  // Natural Earth 1:110m, the coarsest tier — right for a globe a few hundred
  // pixels across, and small enough to ship.
  topology: "https://unpkg.com/world-atlas@2/countries-110m.json",
  // [alpha2, alpha3, numeric, ...] — supplies the numeric -> alpha-2 join.
  codes: "https://unpkg.com/i18n-iso-countries@7/codes.json",
  // Official English names for every alpha-2, including the ~70 countries too
  // small to appear in the 110m polygons but which still show up in telemetry.
  names: "https://unpkg.com/i18n-iso-countries@7/langs/en.json",
  // Fallback coordinates. Natural Earth's 110m tier has no polygon for city
  // states and small territories, and Fresco has real users in several of them
  // (Hong Kong, for one), so without this they would be named but unplottable.
  latlng:
    "https://raw.githubusercontent.com/mledoze/countries/master/dist/countries.json",
};

async function getJson(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} -> ${res.status} ${res.statusText}`);
  return res.json();
}

/**
 * Minimal TopoJSON decoder — quantised delta-encoded arcs back to absolute
 * lon/lat rings. Inlined rather than depending on topojson-client because this
 * is the only thing we ever need from it and it runs once, offline.
 */
function decodeArcs(topology) {
  const { scale, translate } = topology.transform;
  return topology.arcs.map((arc) => {
    let x = 0;
    let y = 0;
    return arc.map(([dx, dy]) => {
      x += dx;
      y += dy;
      return [x * scale[0] + translate[0], y * scale[1] + translate[1]];
    });
  });
}

/** Stitch an arc index list into one ring. Negative index = that arc reversed. */
function ringFrom(arcIndexes, arcs) {
  const ring = [];
  for (const index of arcIndexes) {
    const arc = index < 0 ? arcs[~index].slice().reverse() : arcs[index];
    // Consecutive arcs share an endpoint; drop the duplicate.
    ring.push(...(ring.length ? arc.slice(1) : arc));
  }
  return ring;
}

function toPolygons(geometry, arcs) {
  if (geometry.type === "Polygon") {
    return [geometry.arcs.map((r) => ringFrom(r, arcs))];
  }
  if (geometry.type === "MultiPolygon") {
    return geometry.arcs.map((poly) => poly.map((r) => ringFrom(r, arcs)));
  }
  return [];
}

/** Signed planar area of a ring (shoelace). Sign encodes winding. */
function ringArea(ring) {
  let sum = 0;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    sum += ring[j][0] * ring[i][1] - ring[i][0] * ring[j][1];
  }
  return sum / 2;
}

/**
 * A representative point for the country, used to place its marker.
 *
 * The centroid of the *largest* ring, not of all rings averaged: averaging puts
 * the United States in the Pacific between Alaska and Florida, and France in
 * the Atlantic among its overseas départements. Largest-ring keeps the marker
 * on the landmass a reader expects.
 */
function representativePoint(polygons) {
  let best = null;
  let bestArea = 0;
  for (const rings of polygons) {
    const outer = rings[0];
    if (!outer || outer.length < 3) continue;
    const area = Math.abs(ringArea(outer));
    if (area > bestArea) {
      bestArea = area;
      best = outer;
    }
  }
  if (!best) return null;

  // Area-weighted centroid of that ring.
  let cx = 0;
  let cy = 0;
  let a = 0;
  for (let i = 0, j = best.length - 1; i < best.length; j = i++) {
    const cross = best[j][0] * best[i][1] - best[i][0] * best[j][1];
    a += cross;
    cx += (best[j][0] + best[i][0]) * cross;
    cy += (best[j][1] + best[i][1]) * cross;
  }
  a *= 3; // 6 * (area/2)
  if (a === 0) return null;
  return { lon: cx / a, lat: cy / a };
}

const round = (n, places = 2) => Number(n.toFixed(places));

async function main() {
  console.log("fetching sources…");
  const [topology, codes, names, latlng] = await Promise.all([
    getJson(SOURCES.topology),
    getJson(SOURCES.codes),
    getJson(SOURCES.names),
    getJson(SOURCES.latlng),
  ]);

  const fallbackPoint = new Map(
    latlng
      .filter((c) => Array.isArray(c.latlng) && c.latlng.length === 2)
      .map((c) => [c.cca2, { lat: c.latlng[0], lon: c.latlng[1] }])
  );

  // "004" and 4 both appear as topology ids depending on the build; key on the
  // number so the join cannot miss on leading zeros.
  const numericToAlpha2 = new Map(
    codes.map(([alpha2, , numeric]) => [Number(numeric), alpha2])
  );

  // en.json values are either a name or [name, ...aliases]; take the first.
  const nameOf = Object.fromEntries(
    Object.entries(names.countries).map(([alpha2, value]) => [
      alpha2,
      Array.isArray(value) ? value[0] : value,
    ])
  );

  const arcs = decodeArcs(topology);
  const features = [];
  const lookup = {};
  const unmatched = [];

  for (const geometry of topology.objects.countries.geometries) {
    const alpha2 = numericToAlpha2.get(Number(geometry.id));
    if (!alpha2) {
      unmatched.push(geometry.properties?.name ?? geometry.id);
      continue;
    }
    const name = nameOf[alpha2] ?? geometry.properties?.name ?? alpha2;
    const polygons = toPolygons(geometry, arcs);
    if (polygons.length === 0) continue;

    // ~1 km precision. Ample for a globe a few hundred pixels wide, and it
    // roughly halves the file the browser has to download.
    const rounded = polygons.map((rings) =>
      rings.map((ring) => ring.map(([lon, lat]) => [round(lon), round(lat)]))
    );
    const point = representativePoint(polygons);

    features.push({
      type: "Feature",
      id: alpha2,
      properties: { iso2: alpha2, name },
      geometry:
        rounded.length === 1
          ? { type: "Polygon", coordinates: rounded[0] }
          : { type: "MultiPolygon", coordinates: rounded },
    });

    if (point) {
      lookup[alpha2] = { name, lat: round(point.lat), lon: round(point.lon) };
    }
  }

  // Countries with no polygon at 110m resolution (city states, small islands)
  // still need a name and a marker position, or a real user in Singapore shows
  // up on the dashboard as the bare string "SG" and never appears on the globe.
  for (const [alpha2, name] of Object.entries(nameOf)) {
    if (lookup[alpha2]) continue;
    const point = fallbackPoint.get(alpha2);
    lookup[alpha2] = point
      ? { name, lat: round(point.lat), lon: round(point.lon) }
      : { name, lat: null, lon: null };
  }

  await mkdir(join(ROOT, "public/geo"), { recursive: true });
  await writeFile(
    join(ROOT, "public/geo/countries.geojson"),
    JSON.stringify({ type: "FeatureCollection", features })
  );

  const entries = Object.entries(lookup).sort(([a], [b]) => a.localeCompare(b));
  const ts = `// GENERATED by scripts/build-geo.mjs — do not edit by hand.
//
// Every ISO 3166-1 alpha-2 code, its English name, and a representative point
// for placing a marker. \`lat\`/\`lon\` are null for countries with no polygon at
// 1:110m resolution (city states, small islands) — they are still named, so
// they render as a name rather than a raw code, they just cannot be plotted.

export type CountryInfo = {
  name: string;
  lat: number | null;
  lon: number | null;
};

export const COUNTRIES: Record<string, CountryInfo> = {
${entries
  .map(
    ([code, v]) =>
      `  ${code}: { name: ${JSON.stringify(v.name)}, lat: ${v.lat}, lon: ${v.lon} },`
  )
  .join("\n")}
};
`;
  await writeFile(join(ROOT, "src/lib/countries.generated.ts"), ts);

  console.log(`  ${features.length} country polygons`);
  console.log(`  ${entries.length} countries named`);
  console.log(`  ${entries.filter(([, v]) => v.lat !== null).length} plottable`);
  if (unmatched.length) {
    // Natural Earth carries a few entries with no ISO code at all (Kosovo,
    // Somaliland, N. Cyprus). Reported, not silently dropped.
    console.log(`  skipped (no ISO alpha-2): ${unmatched.join(", ")}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
