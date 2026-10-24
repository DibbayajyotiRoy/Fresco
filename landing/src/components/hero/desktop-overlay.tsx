import { Folder, FileText, Trash2, Grid2x2, Wifi, Volume2, BatteryFull } from "lucide-react";
import type { Dictionary } from "@/lib/i18n";

/**
 * Purely decorative desktop chrome drawn over the demo video: a translucent
 * top panel and a column of generic desktop icons, so the video reads as
 * "your Linux desktop, wallpaper playing behind your icons" instead of "a
 * video in a player". No real app screenshots, no other products' branding,
 * no Fresco UI — just the generic bits every Linux DE panel/desktop has.
 *
 * aria-hidden: none of this is interactive or informative; screen readers
 * already get the real description from DemoVideo's aria-label.
 * A mock LTR desktop: `dir="ltr"` keeps it unmirrored on RTL pages.
 */
export function DesktopOverlay({ dict }: { dict: Dictionary["hero"] }) {
  return (
    <div aria-hidden dir="ltr" className="pointer-events-none absolute inset-0 select-none">
      {/* Vignette: the source clip is mostly pale, so the icon labels and
          panel text need a darker patch behind them to stay legible. */}
      <div
        className="absolute inset-0"
        style={{
          background:
            "linear-gradient(135deg, rgb(0 0 0 / 0.32) 0%, rgb(0 0 0 / 0.16) 22%, transparent 46%)",
        }}
      />

      {/* Top panel: translucent dark in both themes (it sits on the video,
          not on the page background), so it never depends on the theme. */}
      <div className="absolute inset-x-0 top-0 flex h-7 items-center gap-2 bg-black/45 px-2.5 text-white backdrop-blur-[2px] sm:h-8 sm:px-3">
        <Grid2x2 className="size-3 shrink-0 opacity-90 sm:size-3.5" />
        <span className="truncate text-[10px] font-medium tracking-wide opacity-90 sm:text-[11px]">
          {dict.desktopActivities}
        </span>
        <span className="flex-1 text-center text-[10px] font-medium tabular-nums opacity-90 sm:text-[11px]">
          {dict.desktopClock}
        </span>
        <div className="flex shrink-0 items-center gap-1.5 sm:gap-2">
          <Wifi className="size-3 opacity-80 sm:size-3.5" />
          <Volume2 className="size-3 opacity-80 sm:size-3.5" />
          <BatteryFull className="size-3 opacity-80 sm:size-3.5" />
        </div>
      </div>

      {/* Desktop icons: top-left, well clear of the centre where the clip's
          subject sits. Labels get their own dark chip (not just a
          text-shadow) so they hold up over the brightest frames. */}
      <div className="absolute left-2 top-10 flex flex-col items-start gap-2.5 sm:left-4 sm:top-12 sm:gap-3.5">
        <DesktopIcon icon={Folder} label={dict.desktopIconHome} />
        <DesktopIcon icon={Folder} label={dict.desktopIconPictures} />
        <DesktopIcon icon={FileText} label={dict.desktopIconDocument} />
        <DesktopIcon icon={Trash2} label={dict.desktopIconTrash} />
      </div>
    </div>
  );
}

function DesktopIcon({
  icon: Icon,
  label,
}: {
  icon: typeof Folder;
  label: string;
}) {
  return (
    <div className="flex flex-col items-center gap-1">
      <Icon
        className="size-5 text-white drop-shadow-[0_1px_3px_rgb(0_0_0_/_0.85)] sm:size-6"
        strokeWidth={1.75}
      />
      <span
        className="hidden max-w-[4.5rem] truncate rounded bg-black/45 px-1.5 py-0.5 text-[9px] font-medium leading-none text-white sm:inline-block sm:text-[10px]"
        style={{ textShadow: "0 1px 2px rgb(0 0 0 / 0.9)" }}
      >
        {label}
      </span>
    </div>
  );
}
