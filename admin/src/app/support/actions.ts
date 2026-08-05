"use server";

import { revalidatePath } from "next/cache";

import { getSupabaseAdmin } from "@/lib/supabase-admin";

export type ActionResult = { ok: true } | { ok: false; error: string };

const MISSING = "Set SUPABASE_SERVICE_ROLE_KEY in .env.local";

/**
 * Reply to one anonymous support thread.
 *
 * Goes through the `support_reply` RPC rather than a direct insert so that the
 * thread's flags and status move in the same transaction as the message, and
 * so the "who may post as the maintainer" rule lives in one place. The RPC is
 * deliberately NOT granted to anon: only a caller holding the service_role key
 * can post as the maintainer, so extracting the key from the shipped app is
 * not enough to impersonate you.
 */
export async function replyToThread(
  ticket: string,
  body: string
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const trimmed = body.trim();
  if (!trimmed) return { ok: false, error: "Write something first." };
  if (trimmed.length > 4000) {
    return { ok: false, error: "Replies are capped at 4000 characters." };
  }

  const { error } = await supabase.rpc("support_reply", {
    p_ticket: ticket,
    p_body: trimmed,
  });
  if (error) return { ok: false, error: error.message };

  revalidatePath("/support");
  return { ok: true };
}

/** Close a thread, or reopen it. Maintainer-side workflow only — the user
 *  never sees a status, and a new message from them reopens it anyway. */
export async function setThreadStatus(
  ticket: string,
  status: "open" | "answered" | "closed"
): Promise<ActionResult> {
  const supabase = getSupabaseAdmin();
  if (!supabase) return { ok: false, error: MISSING };

  const { error } = await supabase
    .from("support_threads")
    .update({ status, unread_for_maintainer: false })
    .eq("ticket", ticket);
  if (error) return { ok: false, error: error.message };

  revalidatePath("/support");
  return { ok: true };
}
