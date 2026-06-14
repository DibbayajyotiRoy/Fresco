import Link from "next/link";
import { Button } from "@/components/ui/button";
import { RELEASES_URL } from "@/lib/site";
import { Wordmark } from "@/components/wordmark";

const NAV_LINKS = [
  { href: "#features", label: "Features" },
  { href: "#compare", label: "Compare" },
  { href: "#whats-new", label: "What's new" },
  { href: "#download", label: "Download" },
];

export function SiteNav() {
  return (
    <header className="sticky top-0 z-50 w-full border-b border-border/60 bg-background/80 backdrop-blur-md">
      <nav className="mx-auto flex h-16 max-w-6xl items-center justify-between gap-6 px-5">
        <Link
          href="#top"
          className="flex items-center gap-2 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
          aria-label="Fresco home"
        >
          <Wordmark className="h-7 w-7" />
          <span className="text-base font-semibold tracking-tight">
            Fresco
          </span>
        </Link>

        <div className="hidden items-center gap-7 md:flex">
          {NAV_LINKS.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="text-sm text-muted-foreground transition-colors hover:text-foreground"
            >
              {link.label}
            </Link>
          ))}
        </div>

        <Button asChild size="sm" className="font-medium">
          <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
            Get Fresco
          </a>
        </Button>
      </nav>
    </header>
  );
}
