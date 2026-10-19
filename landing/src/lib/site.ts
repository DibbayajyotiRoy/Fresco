export const GITHUB_URL = "https://github.com/DibbayajyotiRoy/fresco";
export const RELEASES_URL =
  "https://github.com/DibbayajyotiRoy/fresco/releases/latest";
export const LICENSE_URL =
  "https://github.com/DibbayajyotiRoy/fresco/blob/main/LICENSE";

export const INSTALL_ONELINER =
  "curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | bash";

/**
 * The version placed on the clipboard: install.sh reads FRESCO_SOURCE and
 * persists it for the app's opt-in telemetry (install attribution only; no
 * other tracking). Displayed commands stay the shorter INSTALL_ONELINER.
 */
export const INSTALL_ONELINER_COPY =
  "curl -fsSL https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh | FRESCO_SOURCE=website bash";

export const APT_INSTALL = "sudo apt install ./fresco_*.deb";

export const AUTHOR_NAME = "Dibbayajyoti Roy";
export const PORTFOLIO_URL = "https://dibbayajyoti.com/";

/**
 * Opt-in telemetry cohort, shown in the stats band as "2,500+" / "127+".
 * Hand-maintained floors (the static site reads no analytics): distinct
 * install ids and distinct countries in the `installs` table, counted the way
 * the admin Usage page counts them. Floors stay true as the numbers grow;
 * raise them when the next round threshold is crossed.
 *
 * Snapshot 2026-10-16: 2,500+ users in 127 countries, the figures README.md
 * states.
 */
export const COHORT = {
  users: 2500,
  countries: 127,
} as const;
