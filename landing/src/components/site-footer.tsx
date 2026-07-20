import Image from "next/image";
import Link from "next/link";
import { SoundToggle } from "@/components/sound-toggle";
import { GITHUB_URL, LICENSE_URL } from "@/lib/site";

const FOOTER_LINKS = [
  { href: GITHUB_URL, label: "GitHub" },
  { href: LICENSE_URL, label: "License" },
];

export function SiteFooter() {
  return (
    <footer id="site-footer" className="border-t border-hairline py-10">
      <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-6 px-5 sm:flex-row">
        <div className="flex items-center gap-2">
          <Image
            src="/logo.png"
            width={24}
            height={24}
            alt=""
            className="rounded-[5px]"
          />
          <span className="font-serif text-lg text-ink">Fresco</span>
          <span className="instrument-label ml-2">rust + gtk4 + mpv</span>
        </div>

        <nav className="flex items-center gap-6">
          {FOOTER_LINKS.map((link) => (
            <Link
              key={link.label}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              className="font-mono text-meta uppercase tracking-widest text-ink-subtle transition-colors hover:text-ink"
            >
              {link.label}
            </Link>
          ))}
          <SoundToggle />
        </nav>

        <p className="font-mono text-meta tracking-wide text-ink-faint">
          © {new Date().getFullYear()} Fresco · GPL-3.0
        </p>
      </div>
    </footer>
  );
}
