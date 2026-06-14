"use server";

import { revalidatePath } from "next/cache";

import { getSupabaseAdmin } from "@/lib/supabase-admin";

export type ActionResult = { ok: true } | { ok: false; error: string };

const MISSING = "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

function parsePayload(formData: FormData) {
  const title = String(formData.get("title") ?? "").trim();
  const body = String(formData.get("body") ?? "").trim();
  const urlRaw = String(formData.get("url") ?? "").trim();
  const published = formData.get("published") === "on";

  return {
    title,
    body,
    url: urlRaw.length > 0 ? urlRaw : null,
    published,
  };
}

export async function createNotification(
  formData: FormData
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const payload = parsePayload(formData);
  if (!payload.title) return { ok: false, error: "Title is required." };
  if (!payload.body) return { ok: false, error: "Body is required." };

  const { error } = await supabase.from("notifications").insert(payload);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/notifications");
  revalidatePath("/");
  return { ok: true };
}

export async function updateNotification(
  id: string,
  formData: FormData
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const payload = parsePayload(formData);
  if (!payload.title) return { ok: false, error: "Title is required." };
  if (!payload.body) return { ok: false, error: "Body is required." };

  const { error } = await supabase
    .from("notifications")
    .update(payload)
    .eq("id", id);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/notifications");
  revalidatePath("/");
  return { ok: true };
}

export async function setPublished(
  id: string,
  published: boolean
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const { error } = await supabase
    .from("notifications")
    .update({ published })
    .eq("id", id);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/notifications");
  revalidatePath("/");
  return { ok: true };
}

export async function deleteNotification(id: string): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const { error } = await supabase.from("notifications").delete().eq("id", id);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/notifications");
  revalidatePath("/");
  return { ok: true };
}
