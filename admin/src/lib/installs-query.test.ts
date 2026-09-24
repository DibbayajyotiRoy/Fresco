import assert from "node:assert/strict";
import { test } from "node:test";

import { applyInstallsQuery, parseInstallsQuery } from "./installs-query.ts";
import type { Install } from "./types.ts";

const NOW = Date.parse("2026-09-24T12:00:00.000Z");
const DAY_MS = 24 * 60 * 60 * 1000;
const isoDaysAgo = (d: number) => new Date(NOW - d * DAY_MS).toISOString();

function install(overrides: Partial<Install>): Install {
  return {
    install_id: "id",
    version: "1.1.40",
    distro: "ubuntu",
    compositor: null,
    session: null,
    backend: null,
    decode: null,
    source: null,
    channel: "deb",
    country: "US",
    minimal: false,
    city: null,
    region: null,
    monitor_count: 1,
    first_seen: isoDaysAgo(10),
    last_seen: isoDaysAgo(1),
    ...overrides,
  };
}

test("parseInstallsQuery: defaults on empty/garbage input", () => {
  const q = parseInstallsQuery({});
  assert.equal(q.status, "all");
  assert.equal(q.page, 1);
  assert.equal(q.pageSize, 25);
  assert.equal(q.sort, "last_seen");
  assert.equal(q.dir, "desc");

  const garbage = parseInstallsQuery({ status: "bogus", page: "-3", pageSize: "999", sort: "nope" });
  assert.equal(garbage.status, "all");
  assert.equal(garbage.page, 1);
  assert.equal(garbage.pageSize, 25);
  assert.equal(garbage.sort, "last_seen");
});

test("applyInstallsQuery: filters by status and search prefix, paginates", () => {
  const installs = [
    install({ install_id: "aa1", last_seen: isoDaysAgo(1) }), // active
    install({ install_id: "aa2", last_seen: isoDaysAgo(90) }), // lapsed
    install({ install_id: "bb1", last_seen: isoDaysAgo(1) }), // active
  ];
  const query = parseInstallsQuery({ status: "active", q: "aa" });
  const page = applyInstallsQuery(installs, query, NOW, null);
  assert.equal(page.total, 1);
  assert.equal(page.rows[0].install_id, "aa1");
});

test("applyInstallsQuery: page size and page count", () => {
  const installs = Array.from({ length: 30 }, (_, i) =>
    install({ install_id: `id${i}`, last_seen: isoDaysAgo(i) })
  );
  const query = parseInstallsQuery({ pageSize: "10", page: "2" });
  const page = applyInstallsQuery(installs, query, NOW, null);
  assert.equal(page.total, 30);
  assert.equal(page.pageCount, 3);
  assert.equal(page.rows.length, 10);
});

test("applyInstallsQuery: sort by install_id ascending", () => {
  const installs = [
    install({ install_id: "c" }),
    install({ install_id: "a" }),
    install({ install_id: "b" }),
  ];
  const query = parseInstallsQuery({ sort: "install_id", dir: "asc" });
  const page = applyInstallsQuery(installs, query, NOW, null);
  assert.deepEqual(page.rows.map((r) => r.install_id), ["a", "b", "c"]);
});
