import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import type { Dictionary } from "@/lib/i18n";

export function Faq({ dict }: { dict: Dictionary }) {
  return (
    <section id="faq" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto grid max-w-6xl gap-10 px-5 lg:grid-cols-[1fr_1.4fr]">
        <div>
          <p className="instrument-label">{dict.faq.kicker}</p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            {dict.faq.title}
          </h2>
          <p className="mt-4 max-w-md text-pretty text-ink-subtle">
            {dict.faq.lead}
          </p>
        </div>

        <Accordion type="single" collapsible className="w-full">
          {dict.faq.items.map((item) => (
            <AccordionItem key={item.q} value={item.q}>
              <AccordionTrigger className="text-left text-base text-ink">
                {item.q}
              </AccordionTrigger>
              <AccordionContent className="text-ink-subtle">
                {item.a}
              </AccordionContent>
            </AccordionItem>
          ))}
        </Accordion>
      </div>
    </section>
  );
}
