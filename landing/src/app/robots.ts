import type { MetadataRoute } from "next";

const SITE_URL = process.env.SITE_URL ?? "https://fresco.app";

/**
 * Fresco is open source and WANTS to be cited by AI answer engines, so every
 * crawler (including AI training and on-demand search bots) is allowed.
 */
export default function robots(): MetadataRoute.Robots {
  const aiBots = [
    "GPTBot",
    "ChatGPT-User",
    "OAI-SearchBot",
    "ClaudeBot",
    "Claude-User",
    "Claude-SearchBot",
    "PerplexityBot",
    "Perplexity-User",
    "Google-Extended",
    "Applebot-Extended",
    "Amazonbot",
    "DuckAssistBot",
    "Meta-ExternalAgent",
    "cohere-ai",
    "Bytespider",
    "CCBot",
    "MistralAI-User",
  ];

  return {
    rules: [
      { userAgent: "*", allow: "/" },
      ...aiBots.map((userAgent) => ({ userAgent, allow: "/" })),
    ],
    sitemap: `${SITE_URL}/sitemap.xml`,
    host: SITE_URL,
  };
}
