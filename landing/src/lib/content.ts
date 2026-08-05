/**
 * Language-independent marketing data: the shape of the comparison matrix,
 * the tested distros, the supported formats, the field report, the author.
 *
 * All prose moved to src/lib/i18n/dictionaries/*. What stays here is either
 * structural (which cell is a yes, which is a no) or a proper noun that must
 * read identically in every language (distro names, file extensions, the
 * verbatim testimonial). Row and cell IDs below are the join keys into
 * `dict.compare.rows` / `dict.compare.cells`.
 */

export const FORMATS = [
  "mp4",
  "webm",
  "mkv",
  "avi",
  "mov",
  "GIF",
  "jpg",
  "png",
  "webp",
  "image slideshow",
  "video playlist",
];

export const DISTROS = [
  "Pop!_OS 22.04",
  "Pop!_OS 24.04 (COSMIC)",
  "Ubuntu 22.04 / 24.04",
  "Linux Mint 21 / 22",
  "Debian 12",
  "elementary OS 7",
  "Deepin 25 (DDE)",
];

/**
 * Competitor comparison. Cells: true (yes), false (no), or a qualifier ID
 * resolved through `dict.compare.cells`. Sourced from README.md. Komorebi is
 * unmaintained; Wallpaper Engine is a paid, Windows-first product.
 */
export type CompareCellId = "partial" | "manual" | "compositorOff" | "cropOnly" | "workshop";
export type CompareCell = boolean | CompareCellId;

export type CompareRowId =
  | "gui"
  | "x11"
  | "wayland"
  | "hwDecode"
  | "cropRotate"
  | "playlists"
  | "slideshow"
  | "library"
  | "catalog"
  | "perDisplay"
  | "schedules"
  | "maintained"
  | "foss";

export const COMPARISON: {
  tools: string[];
  rows: { id: CompareRowId; values: CompareCell[] }[];
} = {
  tools: ["Fresco", "Hidamari", "Komorebi", "mpvpaper", "Wallpaper Engine"],
  rows: [
    { id: "gui", values: [true, true, true, false, true] },
    { id: "x11", values: [true, true, true, false, "compositorOff"] },
    { id: "wayland", values: [true, "partial", false, true, false] },
    { id: "hwDecode", values: [true, "partial", "partial", true, true] },
    { id: "cropRotate", values: [true, false, false, false, "cropOnly"] },
    { id: "playlists", values: [true, false, false, "manual", true] },
    { id: "slideshow", values: [true, false, false, false, true] },
    { id: "library", values: [true, false, false, false, true] },
    { id: "catalog", values: [true, false, false, false, "workshop"] },
    { id: "perDisplay", values: [true, false, false, "manual", true] },
    { id: "schedules", values: [true, false, false, false, "partial"] },
    { id: "maintained", values: [true, true, false, true, true] },
    { id: "foss", values: [true, true, true, true, false] },
  ],
};

/**
 * A single community field report, quoted with the reviewer's written
 * permission. Verbatim: do not paraphrase, trim, or TRANSLATE the quote. It
 * renders in English on every locale, with only the surrounding labels and
 * the reviewer's role localised. Deliberately not emitted as schema.org
 * Review / AggregateRating markup: one named quote is a testimonial, and
 * review rich-result markup off a single quote is a policy violation.
 */
export const TESTIMONIAL = {
  quote:
    "Easy to use with a clean interface — one of the few live wallpaper apps properly adapted for Deepin 25, installable via .deb and running smoothly with hardware-accelerated playback.",
  author: "柒玖 (deepin forum) / 柒仈玖 (GitHub)",
  environment: [
    { id: "session", value: "X11" },
    { id: "os", value: "Deepin 25 Community Edition, build1" },
    { id: "gpu", value: "Intel Alder Lake-N [Intel Graphics]" },
  ],
} as const;

/** Author / maintainer, used in JSON-LD trust signals. */
export const AUTHOR = {
  name: "Dibbayajyoti Roy",
  github: "https://github.com/DibbayajyotiRoy",
  portfolio: "https://dibbayajyoti.com",
};
