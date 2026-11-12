import assert from "node:assert/strict";
import { test } from "node:test";
import { parseReportPage, parseDashboard } from "../../src/api/report-contracts.ts";
import { timeWindow, progress, reportRows } from "../../src/reports/model.ts";

test("report dates use local inclusive calendar days and an exclusive frozen end across DST", () => {
  const old = process.env.TZ;
  process.env.TZ = "America/Toronto";
  try {
    for (const now of [new Date(2026, 2, 10, 14), new Date(2026, 10, 3, 14)]) {
      assert.equal(timeWindow("today", now).start, new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime());
      assert.equal(timeWindow("week", now).start, new Date(now.getFullYear(), now.getMonth(), now.getDate() - 6).getTime());
      assert.equal(timeWindow("month", now).start, new Date(now.getFullYear(), now.getMonth(), now.getDate() - 29).getTime());
      assert.deepEqual(timeWindow("all", now), { start: null, end: now.getTime() + 1 });
    }
  } finally { if (old === undefined) delete process.env.TZ; else process.env.TZ = old; }
});

test("reports reject raw logs, oversized responses and unsafe numbers; dashboard has bounded previews", () => {
  const item = { id: "a", fileName: "a.mp4", mediaType: "video", state: "active", count: 2, lastAt: 10, position: 1, duration: 2 };
  assert.equal(parseReportPage({ items: [item], nextCursor: null }).items[0]?.count, 2);
  for (const value of [{ items: Array(201).fill(item), nextCursor: null }, { items: [{ ...item, count: Number.MAX_SAFE_INTEGER + 1 }], nextCursor: null }, { items: [{ ...item, position: NaN }], nextCursor: null }, { items: [item], nextCursor: { key: -1 } }]) assert.throws(() => parseReportPage(value));
  const dashboard = { totalMedia: 2, images: 1, videos: 1, totalLikes: 3, viewsToday: 2, recentlyAdded: [], recentlyViewed: [], continueWatching: [item], mostLiked: [], mostViewed: [] };
  assert.equal(parseDashboard(dashboard).totalLikes, 3);
  assert.throws(() => parseDashboard({ ...dashboard, totalMedia: 1 }));
  assert.throws(() => parseDashboard({ ...dashboard, mostLiked: Array(7).fill(item) }));
});

test("progress is bounded, unknown durations remain unknown, and history virtualization stays bounded", () => {
  assert.equal(progress({ position: 25, duration: 100 }), 25);
  assert.equal(progress({ position: 110, duration: 100 }), 100);
  assert.equal(progress({ position: 10, duration: null }), null);
  for (const top of [0, 5000, 5000000]) {
    const range = reportRows(100000, top, 600);
    assert.ok(range.end - range.start <= 14);
  }
});
