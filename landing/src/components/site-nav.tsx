import Image from "next/image";
import Link from "next/link";
import { Star } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/theme-toggle";
import { LanguageSwitcher } from "@/components/language-switcher";
import { getGitHubStats } from "@/lib/github";
import { GITHUB_URL, RELEASES_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";
import { LOCALE_META, localePath, type Locale } from "@/lib/i18n/config";

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
    <header className="sticky top-0 z-50 w-full border-b border-hairline bg-paper/95 backdrop-blur">
      <nav className="mx-auto flex h-14 max-w-6xl items-center justify-between gap-6 px-5">
        <Link
          href={home}
          className="flex items-center gap-2.5 rounded-sm"
          aria-label={dict.nav.home}
        >
          <Image
            src="/logo.png"
            width={26}
            height={26}
            alt=""
            priority
            className="rounded-[6px]"
          />
          <span className="font-serif text-xl text-ink">Fresco</span>
        </Link>

        <div className="hidden items-center gap-6 md:flex">
          {links.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="text-sm text-ink-subtle transition-colors hover:text-ink"
            >
              {link.label}
            </Link>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={
              stars === null ? dict.nav.star : dict.nav.starWithCount(stars)
            }
            className="hidden h-8 items-center gap-1.5 rounded-sm border border-hairline px-2.5 font-mono text-meta tabular-nums text-ink-subtle transition-colors hover:border-hairline-strong hover:text-ink sm:inline-flex"
          >
            <Star className="size-3.5" aria-hidden />
            {stars === null ? (
              <span className="text-ink-faint">—</span>
            ) : (
              stars
            )}
          </a>
          <LanguageSwitcher locale={locale} label={dict.language.change} />
          <ThemeToggle label={dict.theme.toggle} />
          <Button asChild size="sm" className="font-medium">
            <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
              {dict.nav.cta}
            </a>
          </Button>
        </div>
      </nav>
    </header>
  );
}
