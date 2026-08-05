import { NextResponse, type NextRequest } from "next/server";
import {
  DEFAULT_LOCALE,
  isLocale,
  LOCALE_COOKIE,
  matchLocale,
} from "@/lib/i18n/config";

/**
 * Locale routing.
 *
 * English keeps the bare, already-indexed URLs (`/`, `/alternatives/...`):
 * those are REWRITTEN onto the `/en` segment so the address bar never changes
 * and no existing link 301s anywhere. Every other language lives under its own
 * prefix and is served directly.
 *
 * Auto-detection only fires on "/" itself, and only when the visitor has not
 * already picked a language. Deeper English-only pages (the competitor
 * comparisons) are never redirected, because they have no translated twin to
 * redirect to. The redirect is a 307 so no search engine records it as the
 * canonical answer for "/"; hreflang tags in the layout do the real work.
 */
export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const segments = pathname.split("/").filter(Boolean);
  const first = segments[0];

  // /en/... is not a URL we publish: fold it back onto the bare path.
  if (first === DEFAULT_LOCALE) {
    const url = request.nextUrl.clone();
    url.pathname = `/${segments.slice(1).join("/")}`;
    return NextResponse.redirect(url, 308);
  }

  // A real locale prefix: serve it as-is, and remember the choice when the
  // visitor arrived by clicking the language switcher.
  if (first && isLocale(first)) {
    // Only "/" is translated. Anything deeper under a locale prefix (the
    // English-only competitor pages) folds back to its canonical URL instead
    // of 404ing on a page that was never generated for that language.
    if (segments.length > 1) {
      const url = request.nextUrl.clone();
      url.pathname = `/${segments.slice(1).join("/")}`;
      return NextResponse.redirect(url, 307);
    }

    const response = NextResponse.next();
    if (request.cookies.get(LOCALE_COOKIE)?.value !== first) {
      response.cookies.set(LOCALE_COOKIE, first, {
        path: "/",
        maxAge: 60 * 60 * 24 * 365,
        sameSite: "lax",
      });
    }
    return response;
  }

  // Unprefixed. Only the home page has translated twins to send people to.
  if (pathname === "/") {
    const saved = request.cookies.get(LOCALE_COOKIE)?.value;
    const preferred =
      saved && isLocale(saved)
        ? saved
        : matchLocale(request.headers.get("accept-language"));

    if (preferred && preferred !== DEFAULT_LOCALE) {
      const url = request.nextUrl.clone();
      url.pathname = `/${preferred}`;
      return NextResponse.redirect(url, 307);
    }
  }

  const url = request.nextUrl.clone();
  url.pathname = `/${DEFAULT_LOCALE}${pathname === "/" ? "" : pathname}`;
  return NextResponse.rewrite(url);
}

export const config = {
  /**
   * Pages only. Everything machine-facing keeps its canonical, locale-less
   * URL: the sitemap, robots, llms.txt, and the AHTML snapshot endpoints are
   * addressed by agents that must not be bounced through a language prefix.
   */
  matcher: [
    "/((?!_next/|api/|ahtml|\\.well-known/|llms\\.txt|robots\\.txt|sitemap\\.xml|favicon|logo\\.png|og\\.png|.*\\.(?:png|jpg|jpeg|gif|svg|webp|ico|mp4|webm|txt|xml|json|webmanifest)$).*)",
  ],
};
