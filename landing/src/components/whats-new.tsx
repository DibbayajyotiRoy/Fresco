import type { Dictionary } from "@/lib/i18n";

/**
 * Retired as a standalone section. The four desktop widgets now render inside
 * Features (components/features.tsx), which keeps the `whats-new` anchor so
 * the nav link still lands on them. Kept, rendering nothing, only so the
 * current page.tsx compiles until its <WhatsNew /> line is removed.
 */
export function WhatsNew(props: { version: string; dict: Dictionary }): null {
  void props;
  return null;
}
