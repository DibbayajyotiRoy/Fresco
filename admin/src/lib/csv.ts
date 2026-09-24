/**
 * CSV encoding for the /api/export routes. Pure and dependency-free — this
 * repo has no CSV library and one row-encoder is not worth adding one for.
 */

/**
 * Cells whose first character is one of these are formula-injection vectors
 * in Excel/Sheets/LibreOffice: opening the CSV lets an attacker-controlled
 * install_id, comment or version string run a formula on whoever opens the
 * export. Prefixed with a leading apostrophe, which every major spreadsheet
 * treats as "force text" and never shows in the cell.
 */
const FORMULA_PREFIXES = ["=", "+", "-", "@", "\t", "\r"];

function escapeCsvCell(value: unknown): string {
  let s = value === null || value === undefined ? "" : String(value);
  if (FORMULA_PREFIXES.some((p) => s.startsWith(p))) {
    s = `'${s}`;
  }
  const needsQuoting = /[",\n\r]/.test(s);
  if (needsQuoting) {
    s = `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

/** One CSV row (CRLF-terminated, per RFC 4180) from an ordered array of cells. */
export function csvRow(cells: unknown[]): string {
  return cells.map(escapeCsvCell).join(",") + "\r\n";
}

/** UTF-8 BOM so Excel on Windows opens the file as UTF-8 instead of guessing
 *  the system codepage and mangling non-ASCII country/city names. */
export const CSV_BOM = "﻿";

/** Build a full CSV document (BOM + header + rows) from a header row and an
 *  array of already-ordered cell arrays. Fine for the row counts this admin
 *  deals with (tens of thousands); true streaming was not worth the
 *  complexity at that size — see the route handlers for the pagination that
 *  keeps memory bounded on the Supabase side instead. */
export function buildCsv(header: string[], rows: unknown[][]): string {
  let out = CSV_BOM + csvRow(header);
  for (const row of rows) out += csvRow(row);
  return out;
}

/** `Content-Disposition` filename with today's date, e.g.
 *  "fresco-installs-2026-09-24.csv". */
export function csvFilename(name: string, nowMs: number = Date.now()): string {
  const date = new Date(nowMs).toISOString().slice(0, 10);
  return `fresco-${name}-${date}.csv`;
}

export function csvResponse(body: string, filename: string): Response {
  return new Response(body, {
    status: 200,
    headers: {
      "Content-Type": "text/csv; charset=utf-8",
      "Content-Disposition": `attachment; filename="${filename}"`,
      "Cache-Control": "no-store",
    },
  });
}
