import { SeverityBadge } from "@/components/badges";

/** Sentiment as a status atom: color says the answer, mono label carries the
 *  word. Severity lane only — never chrome. */
export function SentimentBadge({ rating }: { rating: number }) {
  return rating > 0 ? (
    <SeverityBadge severity="ok" label="up" />
  ) : (
    <SeverityBadge severity="error" label="down" />
  );
}
