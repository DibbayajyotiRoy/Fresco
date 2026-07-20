import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { FAQ } from "@/lib/content";

export function Faq() {
  return (
    <section id="faq" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto grid max-w-6xl gap-10 px-5 lg:grid-cols-[1fr_1.4fr]">
        <div>
          <p className="instrument-label">faq</p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Questions, answered.
          </h2>
          <p className="mt-4 max-w-md text-pretty text-ink-subtle">
            Everything you need to know before setting your first live
            wallpaper on Linux.
          </p>
        </div>

        <Accordion type="single" collapsible className="w-full">
          {FAQ.map((item) => (
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