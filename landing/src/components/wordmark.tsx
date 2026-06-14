/**
 * Fresco mark: the official app icon (monitor playing a live wallpaper, with a
 * play triangle), rendered inline so it keeps its own colors and gradients.
 * Size is controlled via the `className` prop (e.g. "h-7 w-7").
 */
export function Wordmark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={className}
    >
      {/* Background */}
      <rect width="64" height="64" rx="14" fill="#1c1c2e" />
      {/* Monitor frame */}
      <rect x="6" y="10" width="52" height="34" rx="4" fill="#2d2d44" />
      {/* Video frame (live wallpaper) */}
      <rect x="9" y="13" width="46" height="28" rx="2" fill="#3a1c71" />
      {/* Gradient sky */}
      <defs>
        <linearGradient id="fresco-sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#3a1c71" />
          <stop offset="100%" stopColor="#d76d77" />
        </linearGradient>
      </defs>
      <rect x="9" y="13" width="46" height="28" rx="2" fill="url(#fresco-sky)" />
      {/* Mountains */}
      <polygon points="9,41 22,24 35,41" fill="#2d1b5e" opacity="0.85" />
      <polygon points="28,41 40,28 55,41" fill="#1c1040" opacity="0.9" />
      {/* Play triangle (subtle) */}
      <polygon points="27,24 27,32 34,28" fill="white" opacity="0.35" />
      {/* Monitor stand */}
      <rect x="28" y="44" width="8" height="6" rx="1" fill="#2d2d44" />
      <rect x="22" y="50" width="20" height="3" rx="2" fill="#2d2d44" />
    </svg>
  );
}
