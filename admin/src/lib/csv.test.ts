import assert from "node:assert/strict";
import { test } from "node:test";

import { buildCsv, csvFilename, csvRow } from "./csv.ts";

test("csvRow: quotes cells containing commas, quotes or newlines", () => {
  assert.equal(csvRow(["a", "b"]), "a,b\r\n");
  assert.equal(csvRow(["a,b", "c"]), '"a,b",c\r\n');
  assert.equal(csvRow(['say "hi"']), '"say ""hi"""\r\n');
  assert.equal(csvRow(["line1\nline2"]), '"line1\nline2"\r\n');
});

test("csvRow: prefixes formula-injection leading characters with an apostrophe", () => {
  assert.equal(csvRow(["=cmd|'/C calc'!A1"]), "'=cmd|'/C calc'!A1\r\n");
  assert.equal(csvRow(["+1234"]), "'+1234\r\n");
  assert.equal(csvRow(["-1234"]), "'-1234\r\n");
  assert.equal(csvRow(["@SUM(A1)"]), "'@SUM(A1)\r\n");
  // A plain negative number as a normal value (e.g. -1 rating) is still
  // prefixed — correct and safe: spreadsheets read '-1 as text "-1", the cell
  // still displays -1 to a human, and no formula can execute from it.
  assert.equal(csvRow([-1]), "'-1\r\n");
});

test("csvRow: null/undefined become empty cells, not the string 'null'", () => {
  assert.equal(csvRow([null, undefined, "x"]), ",,x\r\n");
});

test("buildCsv: starts with a UTF-8 BOM and includes the header", () => {
  const csv = buildCsv(["a", "b"], [[1, 2]]);
  assert.equal(csv.charCodeAt(0), 0xfeff);
  assert.ok(csv.includes("a,b\r\n"));
  assert.ok(csv.includes("1,2\r\n"));
});

test("csvFilename: dated, per source name", () => {
  const ms = Date.parse("2026-09-24T00:00:00Z");
  assert.equal(csvFilename("installs", ms), "fresco-installs-2026-09-24.csv");
});
