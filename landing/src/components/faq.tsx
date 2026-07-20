"use client";

import * as React from "react";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { FAQ } from "@/lib/content";
import { dispatchQuest } from "@/lib/game";

export function Faq() {
  const [everOpened, setEverOpened] = React.useState(false);

  return (
    <section id="faq" className="border-b border-hairline py-20 sm:py-28">
      <div className="mx-auto grid max-w-6xl gap-10 px-5 lg:grid-cols-[1fr_1.4fr]">
        <div>
          <p className="instrument-label">LORE · +10 xp on first open</p>
          <h2 className="mt-3 font-serif text-display-sm text-ink">
            Questions, decoded.
          </h2>
          <p className="mt-4 max-w-md text-pretty text-ink-subtle">
            Opening any question below reads the operator&apos;s decoder. The
            first one you crack also lights up a quest.
          </p>
        </div>

        <Accordion
          type="single"
          collapsible
          className="w-full"
          onValueChange={(v) => {
            if (v && !everOpened) {
              setEverOpened(true);
              dispatchQuest("lore");
            }
          }}
        >
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