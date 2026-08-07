import { PageSkeleton } from "@/components/skeleton";

/**
 * Shown while the route itself is being fetched, before `page.tsx` gets to
 * emit its shell. Once the shell arrives it takes over and the per-section
 * boundaries handle the rest — so this only has to hold the top of the page:
 * the title and the four Reach cards.
 */
export default function Loading() {
  return <PageSkeleton stats={4} panels={2} />;
}
