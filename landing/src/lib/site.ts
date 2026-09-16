export const GITHUB_URL = "https://github.com/DibbayajyotiRoy/fresco";
export const RELEASES_URL =
  "https://github.com/DibbayajyotiRoy/fresco/releases/latest";
export const FLATHUB_URL = "https://flathub.org/apps/io.github.dibbayajyotiroy.Fresco";
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

export const FLATPAK_INSTALL =
  "flatpak install flathub io.github.dibbayajyotiroy.Fresco";

export const AUTHOR_NAME = "Dibbayajyoti Roy";
export const PORTFOLIO_URL = "https://dibbayajyoti.com/";

/**
 * Opt-in telemetry cohort, shown in the stats band as "1,000+" / "100+".
 * Hand-maintained floors (the static site reads no analytics): distinct
 * install ids and distinct countries in the `installs` table, counted the way
 * the admin Usage page counts them. Floors stay true as the numbers grow;
 * raise them when the next round threshold is crossed.
 *
 * Snapshot 2026-09-15: 1,015 installs all-time (933 active in the last 30
 * days), 101 countries.
 */
export const COHORT = {
  users: 1000,
  countries: 100,
} as const;
