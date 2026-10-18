import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_DASHBOARD_ORDER, DASHBOARD_LAYOUT_KEY, normalizeDashboardOrder,
  moveDashboardSection, readDashboardOrder, saveDashboardOrder,
} from "../src/utils/dashboard-layout.ts";

test("new or malformed preferences use the complete default order", () => {
  for (const value of [null, undefined, {}, "maps", 12]) {
    assert.deepEqual(normalizeDashboardOrder(value), [...DEFAULT_DASHBOARD_ORDER]);
  }
  assert.deepEqual(readDashboardOrder({ getItem: () => "{broken" }), [...DEFAULT_DASHBOARD_ORDER]);
});

test("deduplicates known sections, rejects unknown values and appends missing sections", () => {
  const result = normalizeDashboardOrder(["maps", "maps", "removed", null, "services"]);
  assert.deepEqual(result, ["maps", "services", "overview", "control", "technical", "management", "connections"]);
});

test("maps can move above overview, and back below services without mutating input", () => {
  const original = [...DEFAULT_DASHBOARD_ORDER];
  const moved = moveDashboardSection(original, "maps", "overview", "before");
  assert.equal(moved[0], "maps");
  assert.deepEqual(moveDashboardSection(moved, "maps", "services", "after"), original);
  assert.deepEqual(original, [...DEFAULT_DASHBOARD_ORDER]);
});

test("all source/target/edge combinations preserve every section exactly once", () => {
  for (const source of DEFAULT_DASHBOARD_ORDER) for (const target of DEFAULT_DASHBOARD_ORDER) {
    for (const edge of ["before", "after"]) {
      const next = moveDashboardSection(DEFAULT_DASHBOARD_ORDER, source, target, edge);
      assert.deepEqual([...next].sort(), [...DEFAULT_DASHBOARD_ORDER].sort());
      if (source === target) assert.deepEqual(next, [...DEFAULT_DASHBOARD_ORDER]);
      else assert.equal(next.indexOf(source), next.indexOf(target) + (edge === "after" ? 1 : -1));
    }
  }
});

test("layout survives reload and reset without touching other preferences", () => {
  const data = new Map([["other-preference", "unchanged"]]);
  const storage = { getItem: (key) => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
  const moved = moveDashboardSection(DEFAULT_DASHBOARD_ORDER, "maps", "overview", "before");
  assert.equal(saveDashboardOrder(storage, moved), true);
  assert.deepEqual(readDashboardOrder(storage), moved);
  assert.equal(data.get("other-preference"), "unchanged");
  assert.ok(data.has(DASHBOARD_LAYOUT_KEY));
  saveDashboardOrder(storage, DEFAULT_DASHBOARD_ORDER);
  assert.deepEqual(readDashboardOrder(storage), [...DEFAULT_DASHBOARD_ORDER]);
});

test("blocked storage fails safely instead of breaking the dashboard", () => {
  const blocked = { getItem() { throw Error("blocked"); }, setItem() { throw Error("full"); } };
  assert.deepEqual(readDashboardOrder(blocked), [...DEFAULT_DASHBOARD_ORDER]);
  assert.equal(saveDashboardOrder(blocked, DEFAULT_DASHBOARD_ORDER), false);
});

test("external or removed section IDs cannot enter an order", () => {
  assert.deepEqual(moveDashboardSection(DEFAULT_DASHBOARD_ORDER, "external", "maps", "before"), [...DEFAULT_DASHBOARD_ORDER]);
  assert.deepEqual(moveDashboardSection(DEFAULT_DASHBOARD_ORDER, "maps", "external", "after"), [...DEFAULT_DASHBOARD_ORDER]);
});
