import assert from "node:assert/strict";
import { test } from "node:test";
import { initialFilter, parseLibraryPage, parseDetails, parseTagPage, parseThumbnails } from "../../src/api/library-contracts.ts";
import type { LibraryPage, LibraryCursor, LibraryFilter } from "../../src/api/library-contracts.ts";
import { LibraryController, MAX_SELECTION, PAGE_SIZE, ROW_HEIGHT, toggleSelection, virtualRows } from "../../src/library/model.ts";

const item = { id: "m", fileName: "file.png", mediaType: "image" as const, importedAt: 1, likeCount: 2, width: 100, height: 90, duration: null, customTags: [{ id: "t", name: "Tag" }], customTagCount: 1 };
const cursor: LibraryCursor = { generation: 1, signature: "sig", id: "m", key: "1" };
const page = (name = "m", next: LibraryCursor | null = null): LibraryPage => ({ items: [{ ...item, id: name }], nextCursor: next, generation: 1 });
test("library contracts reject malformed, oversized and unsafe thumbnail responses", () => {
  assert.equal(parseLibraryPage(page()).items[0]?.likeCount, 2);
  assert.equal(parseDetails({ media: item, filePath: "/synthetic.png", fileSize: 100, videoCodec: null, audioCodec: null, container: null, frameRate: null }).media.id, "m");
  assert.equal(parseTagPage({ items: [{ id: "root", name: "Root", parentId: null, hasChildren: true, attached: false }], nextAfter: null }).items.length, 1);
  assert.equal(parseThumbnails([{ mediaId: "m", status: "ready", path: `/cache/thumbnails/${"a".repeat(64)}.jpg`, error: null }]).length, 1);
  for (const v of [null, { ...page(), items: [{ ...item, likeCount: -1 }] }, { ...page(), items: Array(201).fill(item) }, { ...page(), nextCursor: {} }]) assert.throws(() => parseLibraryPage(v));
  for (const path of ["/original.png", "/cache/thumbnails/../private.jpg", null]) assert.throws(() => parseThumbnails([{ mediaId: "m", status: "ready", path, error: null }]));
});
test("100,000 media virtual range stays bounded at top, middle, end and after resize", () => {
  for (const columns of [1, 3, 6, 10]) for (const position of [0, 5000 * ROW_HEIGHT, 100000 * ROW_HEIGHT]) {
    const r = virtualRows(100000, columns, position, 720);
    assert.ok(r.start <= r.end); assert.ok(r.end <= 100000);
    assert.ok(r.end - r.start <= columns * 8, JSON.stringify(r));
    assert.equal(r.totalHeight, Math.ceil(100000 / columns) * ROW_HEIGHT);
  }
  assert.deepEqual(virtualRows(0, 0, 0, 720), { start: 0, end: 0, offset: 0, totalHeight: 0 });
});
test("selection is stable across pages, bounded and supports removal at capacity", () => {
  let selected = new Set<string>(); for (let i = 0; i < 1000; i++) selected = toggleSelection(selected, `m${i}`);
  assert.equal(selected.size, MAX_SELECTION); selected = toggleSelection(selected, "m1"); assert.equal(selected.size, MAX_SELECTION - 1);
  selected = toggleSelection(selected, "other-page"); assert.equal(selected.size, MAX_SELECTION); assert.ok(selected.has("m0"));
});
test("query controller pages incrementally, releases old media and resets filters", async () => {
  const calls: { filter: LibraryFilter; cursor: LibraryCursor | null; limit: number }[] = [];
  const controller = new LibraryController(async (filter, at, limit) => { calls.push({ filter, cursor: at, limit }); return at ? page("second") : page("first", cursor); });
  await controller.refresh(); assert.equal(controller.getSnapshot().items[0]?.id, "first");
  await controller.next(); assert.deepEqual(controller.getSnapshot().items.map(m => m.id), ["second"]); assert.equal(controller.getSnapshot().page, 2);
  await controller.previous(); assert.equal(controller.getSnapshot().items[0]?.id, "first"); assert.equal(controller.getSnapshot().page, 1);
  await controller.change({ ...initialFilter, systemTagId: "parent", search: "%" }); assert.equal(controller.getSnapshot().previous, false);
  assert.ok(calls.every(c => c.limit === PAGE_SIZE)); assert.equal(calls.at(-1)?.cursor, null);
});
test("late query responses and responses after unmount cannot replace current results", async () => {
  const pending: ((page: LibraryPage) => void)[] = [];
  const controller = new LibraryController(() => new Promise(resolve => pending.push(resolve)));
  const old = controller.refresh(); const current = controller.change({ ...initialFilter, search: "new" });
  pending[1]!(page("new")); await current; pending[0]!(page("old")); await old;
  assert.equal(controller.getSnapshot().items[0]?.id, "new");
  const unmounted = controller.refresh(); controller.cancel(); pending[2]!(page("late")); await unmounted;
  assert.ok(!controller.getSnapshot().items.some(i => i.id === "late"));
});
test("stale cursor and storage errors remain explicit and recover with a fresh query", async () => {
  let failure = true;
  const controller = new LibraryController(async () => { if (failure) throw { code: "stale_cursor", message: "请刷新", retryable: true }; return page("recovered"); });
  await controller.refresh(); assert.equal(controller.getSnapshot().error?.code, "stale_cursor"); assert.deepEqual(controller.getSnapshot().items, []);
  failure = false; await controller.refresh(); assert.equal(controller.getSnapshot().error, null); assert.equal(controller.getSnapshot().items[0]?.id, "recovered");
});

test("only Shuffle Again replaces the seed, which survives page, filter and reload changes", async () => {
  let persisted: string | null = null;
  let writes = 0;
  const storage = { getItem: () => persisted, setItem: (_key: string, seed: string) => { persisted = seed; writes++; } };
  const controller = new LibraryController(async (_filter, at) => at ? page("second") : page("first", cursor), storage);
  await controller.change({ ...initialFilter, sort: "random" });
  const initialSeed = controller.getSnapshot().filter.random.seed;
  await controller.next();
  await controller.previous();
  await controller.refresh();
  await controller.change({ ...initialFilter, sort: "random", search: "clip" });
  assert.equal(controller.getSnapshot().filter.random.seed, initialSeed);
  assert.equal(writes, 0);
  await controller.shuffle();
  const shuffled = controller.getSnapshot().filter.random.seed;
  assert.match(shuffled, /^[a-f0-9]{32}$/);
  assert.notEqual(shuffled, initialSeed);
  await controller.change({ ...initialFilter, sort: "name" });
  await controller.change({ ...initialFilter, sort: "random" });
  assert.equal(controller.getSnapshot().filter.random.seed, shuffled);
  assert.equal(writes, 1);
  const reloaded = new LibraryController(async () => page(), storage);
  assert.deepEqual(reloaded.getSnapshot().filter.random, { seed: shuffled, version: 1 });
});
