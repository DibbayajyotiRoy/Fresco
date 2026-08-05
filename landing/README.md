# Fresco landing page

Marketing site for [Fresco](https://github.com/DibbayajyotiRoy/fresco), live
wallpapers for Linux. Built with Next.js (App Router), React 19, Tailwind v4,
and shadcn/ui. Optimized for SEO and GEO (generative-engine optimization).

## Develop

This project uses **pnpm** (or bun), not npm.

```bash
pnpm install
pnpm dev      # http://localhost:3000
pnpm build    # production build
pnpm start    # serve the production build
```

## Deploy to Vercel

1. Import the `DibbayajyotiRoy/fresco` repo into Vercel.
2. **Set the project Root Directory to `landing`** (this app lives in a
   subfolder of the Fresco repo).
3. Add an environment variable `SITE_URL` set to your production domain (used
   for canonical URLs, sitemap, robots, OpenGraph, and JSON-LD).
4. Optional: add `GITHUB_TOKEN` (a public-read token) to raise the GitHub API
   rate limit used for live download counts.

## Languages

The home page ships in English, Japanese, Portuguese (Brazil), Spanish,
German, French, and Simplified Chinese.

- English keeps the bare URLs (`/`, `/alternatives/...`). `src/middleware.ts`
  rewrites those onto the `en` segment internally, so nothing that was already
  indexed or linked ever moves.
- Every other language lives under its own prefix: `/ja`, `/pt-br`, `/es`,
  `/de`, `/fr`, `/zh-cn`.
- A visitor landing on `/` is redirected (307) to their `Accept-Language`
  match, unless the `fresco.locale` cookie records a deliberate pick from the
  language switcher. Deeper pages are never auto-redirected.
- The competitor comparisons under `/alternatives/*` are English only: they
  target English search queries. `/<locale>/alternatives/*` redirects to the
  canonical English URL rather than 404ing.

### Adding or editing a language

All copy lives in `src/lib/i18n/dictionaries/`. `en.ts` is the source of truth
and its shape *is* the `Dictionary` type, so adding a key there makes the other
six fail to compile until they carry it too. `src/lib/content.ts` holds only
what is language-independent (the comparison matrix, distro names, formats, the
verbatim testimonial).

To add a language: add it to `LOCALES` and `LOCALE_META` in
`src/lib/i18n/config.ts`, add a case to `matchLocale()`, register a loader in
`src/lib/i18n/index.ts`, and write the dictionary. hreflang tags, the sitemap
cluster, JSON-LD, and the switcher all follow from `LOCALES` automatically.

Deliberately never translated: the community testimonial (quoting a named
person), YouTube video titles (they must match the uploads), and product nouns
(Fresco, Wallpaper Engine, X11, Wayland, layer-shell, mpv, GPL-3.0, `.deb`).

## What it does for discovery

Live data: the download count, star count, and latest version are read from
the GitHub Releases API server-side and revalidated hourly, so the real
numbers are in the server-rendered HTML.

### SEO

- Per-page metadata, canonical URL, keyword set tuned from real search demand
  (live wallpaper linux, wallpaper engine alternative, hidamari/komorebi
  alternative, animated wallpaper pop os, mpvpaper gui), with per-language
  keywords on top of the shared set.
- A full `hreflang` cluster plus `x-default` on every locale, mirrored in the
  sitemap.
- OpenGraph and Twitter cards, with a dynamic OG image at `/opengraph-image`.
- `sitemap.xml` and `robots.txt` (all AI crawlers allowed on purpose).
- JSON-LD: `SoftwareApplication` (with a live download counter), `WebSite`,
  `Person`, `FAQPage`, and a `VideoObject` per demo, emitted per locale with
  `inLanguage` and locale-scoped `@id`s.
- Answer-first copy, a competitor comparison table, and an FAQ that mirrors how
  people actually phrase these questions.

### GEO (via AHTML)

This site dogfoods [AHTML](https://github.com/DibbayajyotiRoy/AHTML) to publish
agent-readable representations from the same source:

- `/llms.txt` - a markdown summary for LLMs and IDE agents
- `/.well-known/ahtml.json` - the site manifest and policy
- `/ahtml` and `/ahtml?fmt=json` - a typed semantic snapshot of the page
- `/ahtml/mcp.json` - MCP tools
- `/ahtml/openapi.json` - OpenAPI 3.1

## Assets to add

The hero and feature cards currently use placeholder photography
(`picsum.photos`). For the real product page, replace them with actual
screenshots or a short screen recording of Fresco running a live wallpaper.
