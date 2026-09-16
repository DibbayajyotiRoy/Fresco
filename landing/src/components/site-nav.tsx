import Image from "next/image";
import Link from "next/link";
import { Github, Star } from "lucide-react";
import { ThemeToggle } from "@/components/theme-toggle";
import { LanguageSwitcher } from "@/components/language-switcher";
import { MobileMenu } from "@/components/nav/mobile-menu";
import { getGitHubStats } from "@/lib/github";
import { GITHUB_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";
import { LOCALE_META, localePath, type Locale } from "@/lib/i18n/config";
import "@/styles/nav.css";

/**
 * Sticky, solid bar (paper in both themes, no blur): mark + wordmark left,
 * section links centred, then GitHub stars, language, theme and the one
 * filled "Get Fresco" action on the right. Inline links from lg; below that
 * they move into an accessible disclosure menu. Rendered by the locale
 * layout, so it also serves the /alternatives pages (links carry the locale
 * base so they resolve back to the home page from there).
 */
export async function SiteNav({
  locale,
  dict,
}: {
  locale: Locale;
  dict: Dictionary;
}) {
  const stats = await getGitHubStats();
  const home = localePath(locale);
  const base = home === "/" ? "" : home;
  const links = [
    { href: `${base}/#features`, label: dict.nav.features },
    { href: `${base}/#compare`, label: dict.nav.compare },
    { href: `${base}/#whats-new`, label: dict.nav.whatsNew },
    { href: `${base}/#download`, label: dict.nav.download },
  ];
  const stars =
    stats.stars === null
      ? null
      : stats.stars.toLocaleString(LOCALE_META[locale].numberLocale);

  return (
    <header className="sticky top-0 z-50 w-full border-b border-hairline bg-paper">
      <nav className="wrap relative grid h-16 grid-cols-[1fr_auto] items-center gap-4 lg:grid-cols-[1fr_auto_1fr]">
        <Link
          href={home}
          aria-label={dict.nav.home}
          className="flex items-center gap-2.5 justify-self-start rounded-md"
        >
          <Image
            src="/logo.png"
            width={28}
            height={28}
            alt=""
            priority
            className="rounded-[7px]"
          />
          <span className="hidden font-display text-[1.0625rem] leading-none text-ink min-[380px]:inline">
            Fresco
          </span>
        </Link>

        <ul className="hidden items-center gap-1 lg:flex">
          {links.map((link) => (
            <li key={link.href}>
              <Link
                href={link.href}
                className="rounded-md px-3 py-2 text-[14px] font-medium text-ink-subtle transition-colors duration-150 hover:text-ink"
              >
                {link.label}
              </Link>
            </li>
          ))}
        </ul>

        <div className="flex items-center gap-2 justify-self-end">
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={
              stars === null ? dict.nav.star : dict.nav.starWithCount(stars)
            }
            className="nav-press hidden h-9 items-center gap-2 rounded-lg border border-hairline px-3 text-[13px] font-medium tabular-nums text-ink-subtle hover:border-hairline-strong hover:text-ink sm:inline-flex"
          >
            <Github className="size-4" aria-hidden />
            {stars === null ? null : (
              <span className="flex items-center gap-1">
                <Star className="size-3.5 fill-current" aria-hidden />
                {stars}
              </span>
            )}
          </a>
          <LanguageSwitcher locale={locale} label={dict.language.change} />
          <ThemeToggle label={dict.theme.toggle} />
          <a
            href={`${base}/#download`}
            className="nav-press inline-flex h-9 items-center whitespace-nowrap rounded-lg bg-primary px-4 text-[14px] font-medium text-primary-foreground hover:bg-primary/90"
          >
            {dict.nav.cta}
          </a>
          <MobileMenu links={links} label={dict.nav.menu} />
        </div>
      </nav>
    </header>
  );
}
