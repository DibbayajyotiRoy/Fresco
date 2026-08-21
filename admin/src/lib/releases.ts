import type { Release, ReleaseAsset } from "@/lib/types";

/**
 * A release's download counter, split by what was actually fetched.
 *
 * The bare total is not a user count: the one-liner installer pulls
 * `install.sh` and then the `.deb` it points at, so one install can tick two
 * counters. Keeping the halves apart is the only way the panel can show the
 * number without implying it means installs.
 */
export type AssetSplit = {
  /** The package itself — the closest thing to an install. */
  deb: number;
  /** The one-liner installer script, which then fetches a `.deb`. */
  script: number;
  /** Anything else published on the release (checksums, tarballs, …). */
  other: number;
  /** `deb + script + other` — the same figure as `Release.downloads`. */
  total: number;
};

/** Classify by filename, the only platform signal GitHub gives us here. */
function kindOf(name: string): "deb" | "script" | "other" {
  if (name.endsWith(".deb")) return "deb";
  if (name.endsWith(".sh")) return "script";
  return "other";
}

export function splitAssets(assets: ReleaseAsset[]): AssetSplit {
  const split: AssetSplit = { deb: 0, script: 0, other: 0, total: 0 };
  for (const a of assets) {
    split[kindOf(a.name)] += a.downloads;
    split.total += a.downloads;
  }
  return split;
}

/** The same split summed across many releases. */
export function splitReleases(releases: Release[]): AssetSplit {
  return splitAssets(releases.flatMap((r) => r.assets));
}

/**
 * Middle value of a set, averaging the two middles on an even count.
 *
 * Reported next to the total because the mean is useless here: one launch
 * release carries a third of all downloads and drags the average above every
 * release but itself.
 */
export function median(values: number[]): number {
  if (values.length === 0) return 0;
  const s = [...values].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 === 0 ? Math.round((s[mid - 1] + s[mid]) / 2) : s[mid];
}
