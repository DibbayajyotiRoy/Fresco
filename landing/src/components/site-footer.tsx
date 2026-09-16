import Image from "next/image";
import { ArrowUpRight } from "lucide-react";
import { SoundToggle } from "@/components/sound-toggle";
import { FooterDownloadLink } from "@/components/finale/footer-download-link";
import { GITHUB_URL, LICENSE_URL } from "@/lib/site";
import type { Dictionary } from "@/lib/i18n";

/**
 * Renders on the home page AND on /alternatives, where PageMotion is not
 * mounted, so nothing in here uses data-reveal: the footer never depends on
 * the orchestrator to become visible.
 */
export function SiteFooter({ dict }: { dict: Dictionary }) {
  const links = [
    { href: GITHUB_URL, label: dict.footer.github },
    { href: LICENSE_URL, label: dict.footer.license },
  ];

  return (
    <footer id="site-footer" className="border-t border-hairline bg-paper">
      <div className="wrap">
        <div className="mx-auto max-w-6xl py-12 sm:py-16">
          <div className="flex flex-col gap-8 md:flex-row md:items-center md:justify-between">
            <div className="flex items-center gap-3">
              <Image
                src="/logo.png"
                width={32}
                height={32}
                alt=""
                className="rounded-lg"
              />
              <span className="font-display text-xl text-ink">Fresco</span>
            </div>

            <div className="flex items-center gap-6">
              <ul className="flex items-center gap-6">
                {links.map((link) => (
                  <li key={link.label}>
                    <a
                      href={link.href}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1 text-base text-ink-subtle transition-colors duration-150 hover:text-ink"
                    >
                      {link.label}
                      <ArrowUpRight className="size-3.5" aria-hidden />
                    </a>
                  </li>
                ))}
              </ul>
              <SoundToggle label={dict.footer.sound} />
            </div>
          </div>

          <div className="mt-10 flex flex-col-reverse gap-4 border-t border-hairline pt-6 sm:flex-row sm:items-center sm:justify-between">
            <p className="text-sm text-ink-faint">
              © {new Date().getFullYear()} Fresco · GPL-3.0
            </p>
            <FooterDownloadLink label={dict.nav.cta} />
          </div>
        </div>
      </div>
    </footer>
  );
}
