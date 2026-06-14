import Link from "next/link";
import { Wordmark } from "@/components/wordmark";
import { GITHUB_URL, LICENSE_URL } from "@/lib/site";

const FOOTER_LINKS = [
  { href: GITHUB_URL, label: "GitHub" },
  { href: LICENSE_URL, label: "License" },
];

export function SiteFooter() {
  return (
    <footer className="py-12">
      <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-6 px-5 sm:flex-row">
        <div className="flex items-center gap-2">
          <Wordmark className="h-6 w-6" />
          <span className="text-sm font-semibold tracking-tight">Fresco</span>
          <span className="ml-2 text-xs text-muted-foreground">
            Made with Rust + GTK4
          </span>
        </div>

        <nav className="flex items-center gap-6">
          {FOOTER_LINKS.map((link) => (
            <Link
              key={link.label}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-muted-foreground transition-colors hover:text-foreground"
            >
              {link.label}
            </Link>
          ))}
        </nav>

        <p className="text-xs text-muted-foreground">
          © {new Date().getFullYear()} Fresco · GPL-3.0
        </p>
      </div>
    </footer>
  );
}
