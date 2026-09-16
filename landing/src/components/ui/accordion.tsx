"use client";

import * as React from "react";
import * as AccordionPrimitive from "@radix-ui/react-accordion";
import { ChevronDown } from "lucide-react";

import { cn } from "@/lib/utils";

/*
 * Radix accordion (FAQ). Radix owns keyboard behaviour (Enter/Space toggle,
 * arrow keys between triggers, Home/End). Visuals: hairline separators, a
 * 16px question, a chevron that turns over when open. No height animation:
 * the answer rises in with transform/opacity only (`animate-rise-in`).
 */
const Accordion = AccordionPrimitive.Root;

const AccordionItem = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Item>
>(({ className, ...props }, ref) => (
  <AccordionPrimitive.Item
    ref={ref}
    className={cn("border-b border-hairline", className)}
    {...props}
  />
));
AccordionItem.displayName = "AccordionItem";

const AccordionTrigger = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Trigger>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Header className="flex">
    <AccordionPrimitive.Trigger
      ref={ref}
      className={cn(
        "group/trigger flex flex-1 cursor-pointer items-center justify-between gap-6 py-5 text-left text-lg font-medium text-ink",
        className,
      )}
      {...props}
    >
      {children}
      <ChevronDown
        aria-hidden
        className="size-4 shrink-0 text-ink-faint transition-[transform,color] duration-200 ease-out group-hover/trigger:text-ink group-data-[state=open]/trigger:rotate-180 group-data-[state=open]/trigger:text-ink motion-reduce:transition-none"
      />
    </AccordionPrimitive.Trigger>
  </AccordionPrimitive.Header>
));
AccordionTrigger.displayName = AccordionPrimitive.Trigger.displayName;

const AccordionContent = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Content ref={ref} className="overflow-hidden" {...props}>
    <div
      className={cn(
        "animate-rise-in max-w-2xl pb-6 pr-2 text-base leading-6 text-ink-subtle sm:pr-10",
        className,
      )}
    >
      {children}
    </div>
  </AccordionPrimitive.Content>
));
AccordionContent.displayName = AccordionPrimitive.Content.displayName;

export { Accordion, AccordionItem, AccordionTrigger, AccordionContent };
