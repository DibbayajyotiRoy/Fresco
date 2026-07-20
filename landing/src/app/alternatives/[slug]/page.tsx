import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArrowLeft, Check, Download, Github, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { SiteFooter } from "@/components/site-footer";
import { ALTERNATIVES, getAlternative } from "@/lib/alternatives";
import { COMPARISON, type CompareCell } from "@/lib/content";
import { GITHUB_URL, RELEASES_URL } from "@/lib/site";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.dibbayajyoti.com";

export function generateStaticParams() {
  return ALTERNATIVES.map((a) => ({ slug: a.slug }));
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const alt = getAlternative(slug);
  if (!alt) return {};
  const url = `${SITE_URL}/alternatives/${alt.slug}`;
  return {
    // Absolute: metaTitle already carries the Fresco brand, so skip the
    // "%s | Fresco" template to avoid a duplicated brand in the title.
    title: { absolute: alt.metaTitle },
    description: alt.metaDescription,
    alternates: { canonical: `/alternatives/${alt.slug}` },
    openGraph: {
      title: alt.metaTitle,
      description: alt.metaDescription,
      url,
      siteName: "Fresco",
      type: "article",
      images: [{ url: "/opengraph-image", width: 1200, height: 630 }],
    },
    twitter: {
      card: "summary_large_image",
      title: alt.metaTitle,
      description: alt.metaDescription,
      images: ["/opengraph-image"],
    },
  };
}

function CompareValue({ value }: { value: CompareCell }) {
  if (value === true)
    return <Check className="size-4 text-ink" aria-label="Yes" />;
  if (value === false)
    return <X className="size-4 text-ink-faint" aria-label="No" />;
  return <span className="text-xs text-ink-subtle">{value}</span>;
}

export default async function AlternativePage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const alt = getAlternative(slug);
  if (!alt) notFound();

  const rivalIndex = COMPARISON.tools.indexOf(alt.tool);
  const url = `${SITE_URL}/alternatives/${alt.slug}`;

  const jsonLd = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "SoftwareApplication",
        name: "Fresco",
        applicationCategory: "UtilitiesApplication",
        operatingSystem: "Linux",
        description: alt.lead,
        url,
        downloadUrl: RELEASES_URL,
        offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        isAccessibleForFree: true,
      },
      {
        "@type": "BreadcrumbList",
        itemListElement: [
          { "@type": "ListItem", position: 1, name: "Home", item: SITE_URL },
          {
            "@type": "ListItem",
            position: 2,
            name: `${alt.tool} alternative`,
            item: url,
          },
        ],
      },
      {
        "@type": "FAQPage",
        mainEntity: alt.faq.map(({ q, a }) => ({
          "@type": "Question",
          name: q,
          acceptedAnswer: { "@type": "Answer", text: a },
        })),
      },
    ],
  };

  return (
    <>
      <main>
        <article className="mx-auto max-w-3xl px-5 pb-20 pt-14 sm:pt-20">
          {/* Visible breadcrumb, mirroring the BreadcrumbList JSON-LD. */}
          <nav aria-label="Breadcrumb">
            <ol className="flex items-center gap-2 text-sm text-ink-subtle">
              <li>
                <Link
                  href="/"
                  className="inline-flex items-center gap-1.5 transition-colors hover:text-ink"
                >
                  <ArrowLeft className="size-4" aria-hidden />
                  Home
                </Link>
              </li>
              <li aria-hidden className="text-ink-faint">
                /
              </li>
              <li className="text-ink-muted">{alt.tool} alternative</li>
            </ol>
          </nav>

          <h1 className="mt-6 text-balance font-serif text-display text-ink">
            {alt.h1}
          </h1>
          <p className="mt-5 text-pretty text-lg text-ink-subtle">{alt.lead}</p>

          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <Button asChild size="lg" className="font-medium">
              <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
                <Download />
                Download .deb
              </a>
            </Button>
            <Button asChild size="lg" variant="secondary" className="font-medium">
              <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
                <Github />
                View on GitHub
              </a>
            </Button>
          </div>

          <div className="mt-12 flex flex-col gap-5 text-pretty leading-relaxed text-ink-muted">
            {alt.body.map((p, i) => (
              <p key={i}>{p}</p>
            ))}
          </div>

          {/* Why switch */}
          <div className="mt-12 grid gap-4 sm:grid-cols-3">
            {alt.reasons.map((r) => (
              <Card key={r.title} className="p-5 shadow-none">
                <h2 className="text-sm font-semibold tracking-tight text-ink">
                  {r.title}
                </h2>
                <p className="mt-1.5 text-sm text-ink-subtle">{r.body}</p>
              </Card>
            ))}
          </div>

          {/* Focused comparison: Fresco vs this one competitor */}
          {rivalIndex > 0 && (
            <div className="mt-14">
              <h2 className="font-serif text-2xl text-ink">
                Fresco vs {alt.tool}
              </h2>
              <div className="mt-6 overflow-hidden rounded-md border border-hairline">
                <table className="w-full border-collapse text-sm">
                  <thead>
                    <tr className="border-b border-hairline">
                      <th className="instrument-label px-4 py-3 text-left font-semibold">
                        Feature
                      </th>
                      <th className="instrument-label border-l border-hairline bg-accent/[0.06] px-4 py-3 text-center font-semibold !text-accent">
                        Fresco
                      </th>
                      <th className="instrument-label border-l border-hairline px-4 py-3 text-center font-semibold">
                        {alt.tool}
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {COMPARISON.rows.map((row) => (
                      <tr
                        key={row.label}
                        className="border-b border-hairline last:border-0"
                      >
                        <th
                          scope="row"
                          className="px-4 py-3 text-left font-normal text-ink-subtle"
                        >
                          {row.label}
                        </th>
                        <td className="border-l border-hairline bg-accent/[0.06] px-4 py-3 text-center">
                          <span className="inline-flex justify-center">
                            <CompareValue value={row.values[0]} />
                          </span>
                        </td>
                        <td className="border-l border-hairline px-4 py-3 text-center">
                          <span className="inline-flex justify-center">
                            <CompareValue value={row.values[rivalIndex]} />
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* FAQ */}
          <div className="mt-14">
            <h2 className="font-serif text-2xl text-ink">
              {alt.tool} alternative FAQ
            </h2>
            <dl className="mt-6 flex flex-col divide-y divide-hairline border-t border-hairline">
              {alt.faq.map((item) => (
                <div key={item.q} className="py-5">
                  <dt className="text-base font-medium text-ink">{item.q}</dt>
                  <dd className="mt-2 text-sm text-ink-subtle">{item.a}</dd>
                </div>
              ))}
            </dl>
          </div>

          {/* Closing CTA */}
          <div className="mt-14 rounded-md border border-hairline bg-surface p-8 text-center">
            <h2 className="font-serif text-xl text-ink">
              Try Fresco. It is free.
            </h2>
            <p className="mx-auto mt-2 max-w-md text-sm text-ink-subtle">
              Free and open source under the GPL-3.0 license. Install the .deb
              and set your first live wallpaper in a minute.
            </p>
            <div className="mt-5 flex justify-center">
              <Button asChild size="lg" className="font-medium">
                <a href={RELEASES_URL} target="_blank" rel="noopener noreferrer">
                  <Download />
                  Download .deb
                </a>
              </Button>
            </div>
          </div>
        </article>
      </main>
      <SiteFooter />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />
    </>
  );
}
