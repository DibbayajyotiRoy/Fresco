const numberFormatter = new Intl.NumberFormat("en-US");

/** "1234" -> "1,234" */
export function formatNumber(n: number): string {
  return numberFormatter.format(n);
}

const pad = (n: number) => String(n).padStart(2, "0");

/** ISO string -> "2026-07-18" (local). Null-honest: "—". */
export function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** ISO string -> "2026-07-18 14:03:22" local, for mono timestamp columns. */
export function formatDateTime(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return `${formatDate(iso)} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** ISO string -> compact relative: "42s ago" / "8m ago" / "3h ago" / "2d ago". */
export function formatRelative(iso: string | null): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const s = Math.round((Date.now() - t) / 1000);
  if (s < 0) return formatDate(iso);
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.round(h / 24);
  if (d < 60) return `${d}d ago`;
  return formatDate(iso);
}

/** Ids: first 8 chars + "…" (skipped when already short, ≤11). */
export function truncateId(id: string): string {
  return id.length <= 11 ? id : `${id.slice(0, 8)}…`;
}

/** Milliseconds: <1000 -> "842ms"; else "1.24s". */
export function formatMs(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)}ms` : `${(ms / 1000).toFixed(2)}s`;
}

/** Bytes -> "20.0 MB" style, honest about zero. */
export function formatBytes(bytes: number): string {
  if (!bytes) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1_073_741_824) return `${(bytes / 1_048_576).toFixed(1)} MB`;
  return `${(bytes / 1_073_741_824).toFixed(2)} GB`;
}
