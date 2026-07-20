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
 * Honest cohort telemetry surfaced in the operator HUD. Hand-maintained (no
 * analytics on the static site) — bump alongside releases. Rendered as mono
 * uppercase tallies, em-dash for an unknown value per the data-honisty rule.
 */
export const COHORT = {
  users: "120+",
  deploys: "350+",
} as const;
