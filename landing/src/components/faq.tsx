import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { SplitWords } from "@/components/motion/split-words";
import type { Dictionary } from "@/lib/i18n";

/**
 * FAQ: title and lead in a sticky left column, the accordion on the right.
 * Q&A strings and their order mirror the FAQPage JSON-LD, so never reorder or
 * filter them here alone. No bottom border: the footer's top rule follows.
 */
export function Faq({ dict }: { dict: Dictionary }) {
  return (
    <section id="faq" aria-labelledby="faq-title" className="py-24 sm:py-32">
      <div className="wrap">
        <div className="mx-auto grid max-w-6xl gap-10 lg:grid-cols-12 lg:gap-16">
          <div className="lg:col-span-4">
            <div className="lg:sticky lg:top-28">
              <h2
                id="faq-title"
                data-reveal="words"
                className="font-display text-section text-ink"
              >
                <SplitWords text={dict.faq.title} />
              </h2>
              <p
                data-reveal="fade"
                data-delay="0.15"
                className="mt-5 max-w-md text-lg text-ink-subtle"
              >
                {dict.faq.lead}
              </p>
            </div>
          </div>

          <div className="lg:col-span-8">
            <Accordion
              type="single"
              collapsible
              data-reveal="stagger"
              className="w-full border-t border-hairline"
            >
              {dict.faq.items.map((item) => (
                <AccordionItem key={item.q} value={item.q}>
                  <AccordionTrigger>{item.q}</AccordionTrigger>
                  <AccordionContent>{item.a}</AccordionContent>
                </AccordionItem>
              ))}
            </Accordion>
          </div>
        </div>
      </div>
    </section>
  );
}
