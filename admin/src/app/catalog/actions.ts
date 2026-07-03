"use server";

import { revalidatePath } from "next/cache";

import { getSupabaseAdmin } from "@/lib/supabase-admin";

export type ActionResult = { ok: true } | { ok: false; error: string };

const MISSING = "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

function parsePayload(formData: FormData) {
  const str = (k: string) => String(formData.get(k) ?? "").trim();
  const tags = str("tags")
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
  return {
    title: str("title"),
    content_type: str("content_type") || "video",
    category: str("category") || "other",
    tags,
    media_url: str("media_url"),
    thumb_url: str("thumb_url") || null,
    size_bytes: Number(str("size_bytes")) || 0,
    license: str("license"),
    author: str("author"),
    source_url: str("source_url") || null,
    published: formData.get("published") === "on",
  };
}

export async function createCatalogItem(
  formData: FormData
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const payload = parsePayload(formData);
  if (!payload.title) return { ok: false, error: "Title is required." };
  if (!payload.media_url) return { ok: false, error: "Media URL is required." };
  if (!payload.license)
    return { ok: false, error: "License is required (attribution is a launch requirement)." };

  const { error } = await supabase.from("catalog_items").insert(payload);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/catalog");
  return { ok: true };
}

export async function setCatalogPublished(
  id: string,
  published: boolean
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const { error } = await supabase
    .from("catalog_items")
    .update({ published })
    .eq("id", id);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/catalog");
  return { ok: true };
}

export async function deleteCatalogItem(id: string): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const { error } = await supabase.from("catalog_items").delete().eq("id", id);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/catalog");
  return { ok: true };
}
