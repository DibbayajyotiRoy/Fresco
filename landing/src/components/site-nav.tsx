import Image from "next/image";
import Link from "next/link";
import { Star } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/theme-toggle";
import { getGitHubStats } from "@/lib/github";
import { GITHUB_URL, RELEASES_URL } from "@/lib/site";

const NAV_LINKS = [
  { href: "/#features", label: "Features" },
  { href: "/#compare", label: "Compare" },
  { href: "/#whats-new", label: "What's new" },
  { href: "/#download", label: "Download" },
];

export async function SiteNav() {
  const stats = await getGitHubStats();

  return (
    <header className="sticky top-0 z-50 w-full border-b border-hairline bg-paper/95 backdrop-blur">
      <nav className="mx-auto flex h-14 max-w-6xl items-center justify-between gap-6 px-5">
        <Link
          href="/"
          className="flex items-center gap-2.5 rounded-sm"
          aria-label="Fresco home"
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
          {NAV_LINKS.map((link) => (
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
              stats.stars === null
                ? "Star Fresco on GitHub"
                : `Star Fresco on GitHub (${stats.stars} stars)`
            }
            className="hidden h-8 items-center gap-1.5 rounded-sm border border-hairline px-2.5 font-mono text-meta tabular-nums text-ink-subtle transition-colors hover:border-hairline-strong hover:text-ink sm:inline-flex"
          >
            <Star className="size-3.5" aria-hidden />
            {stats.stars === null ? (
              <span className="text-ink-faint">—</span>
            ) : (
              stats.stars.toLocaleString("en-US")
            )}
          </a>
          <ThemeToggle />
          <Button asChild size="sm" className="font-medium">
            <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
              Get Fresco
            </a>
          </Button>
        </div>
      </nav>
    </header>
  );
}