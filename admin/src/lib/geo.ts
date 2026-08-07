import { COUNTRIES, type CountryInfo } from "@/lib/countries.generated";

export type { CountryInfo };
export { COUNTRIES };

/**
 * Shorter labels for countries whose official ISO name is too long to sit in a
 * table cell or a legend row. Display only — the code is always the key.
 */
const SHORT_NAMES: Record<string, string> = {
  US: "United States",
  GB: "United Kingdom",
  CN: "China",
  KR: "South Korea",
  KP: "North Korea",
  RU: "Russia",
  IR: "Iran",
  SY: "Syria",
  VE: "Venezuela",
  BO: "Bolivia",
  TZ: "Tanzania",
  MD: "Moldova",
  LA: "Laos",
  VN: "Vietnam",
  BN: "Brunei",
  CD: "DR Congo",
  CG: "Congo",
  CZ: "Czechia",
  MK: "North Macedonia",
  TW: "Taiwan",
  PS: "Palestine",
  VA: "Vatican City",
  FM: "Micronesia",
};

/** Display name for a two-letter code, or the code itself if we don't know it. */
export function countryName(code: string): string {
  return SHORT_NAMES[code] ?? COUNTRIES[code]?.name ?? code;
}

/** Marker position for a code, or null when the code isn't a known country. */
export function countryPoint(
  code: string
): { lat: number; lon: number } | null {
  const info = COUNTRIES[code];
  if (!info || info.lat === null || info.lon === null) return null;
  return { lat: info.lat, lon: info.lon };
}

/**
 * The label used everywhere a country is shown.
 *
 * Two different things arrive as "no country" and both read Unknown: SQL null
 * on `installs.country`, and the '??' sentinel on `daily_country` (whose key
 * columns are NOT NULL, so it needs a placeholder). See supabase/schema.sql.
 */
export function countryLabel(code: string | null): string {
  if (!code || code === "??") return "Unknown";
  const name = countryName(code);
  return name === code ? code : `${name} (${code})`;
}

/** Regional grouping, so the globe can be summarised without a 40-row table. */
const CONTINENT_OF: Record<string, string> = {};
const REGIONS: Record<string, string[]> = {
  "Asia-Pacific": [
    "CN", "JP", "KR", "KP", "TW", "HK", "MO", "MN", "IN", "PK", "BD", "LK",
    "NP", "BT", "MV", "AF", "ID", "MY", "SG", "TH", "VN", "PH", "MM", "KH",
    "LA", "BN", "TL", "AU", "NZ", "PG", "FJ", "SB", "VU", "NC", "PF", "WS",
    "TO", "KI", "FM", "MH", "NR", "PW", "TV", "CK", "GU", "AS", "MP", "NU",
    "NF", "TK", "WF",
  ],
  Europe: [
    "GB", "IE", "FR", "DE", "IT", "ES", "PT", "NL", "BE", "LU", "CH", "AT",
    "PL", "CZ", "SK", "HU", "RO", "BG", "GR", "HR", "SI", "RS", "BA", "ME",
    "MK", "AL", "XK", "SE", "NO", "DK", "FI", "IS", "EE", "LV", "LT", "BY",
    "UA", "MD", "RU", "TR", "CY", "MT", "AD", "MC", "SM", "VA", "LI", "FO",
    "GI", "IM", "JE", "GG", "AX", "SJ",
  ],
  "North America": [
    "US", "CA", "MX", "GT", "BZ", "SV", "HN", "NI", "CR", "PA", "CU", "DO",
    "HT", "JM", "TT", "BS", "BB", "PR", "GL", "BM", "KY", "AG", "DM", "GD",
    "KN", "LC", "VC", "AI", "AW", "BQ", "CW", "SX", "MS", "TC", "VG", "VI",
    "GP", "MQ", "BL", "MF", "PM",
  ],
  "South America": [
    "BR", "AR", "CL", "CO", "PE", "VE", "EC", "BO", "PY", "UY", "GY", "SR",
    "GF", "FK",
  ],
  Africa: [
    "NG", "EG", "ZA", "KE", "ET", "GH", "TZ", "UG", "DZ", "MA", "TN", "LY",
    "SD", "SS", "SN", "CI", "CM", "ZW", "ZM", "AO", "MZ", "MW", "RW", "BI",
    "SO", "NE", "ML", "BF", "TD", "MR", "GN", "BJ", "TG", "SL", "LR", "CF",
    "CG", "CD", "GA", "GQ", "NA", "BW", "LS", "SZ", "MG", "MU", "SC", "KM",
    "DJ", "ER", "CV", "ST", "GW", "GM", "YT", "RE", "SH", "EH",
  ],
  "Middle East": [
    "SA", "AE", "IL", "IQ", "IR", "JO", "KW", "LB", "OM", "QA", "SY", "YE",
    "BH", "PS", "AM", "AZ", "GE", "KZ", "KG", "TJ", "TM", "UZ",
  ],
};
for (const [region, codes] of Object.entries(REGIONS)) {
  for (const code of codes) CONTINENT_OF[code] = region;
}

/** Region for a code. "Elsewhere" for anything unclassified — never a guess. */
export function countryRegion(code: string | null): string {
  if (!code || code === "??") return "Unknown";
  return CONTINENT_OF[code] ?? "Elsewhere";
}
