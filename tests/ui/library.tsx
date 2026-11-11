// Standalone development harness. Never imported by the production entry/build.
import { createRoot } from "react-dom/client";
import { libraryApi } from "../../src/api/library.ts";
import { parseDetails, parseLibraryPage, parseTagPage, parseThumbnails } from "../../src/api/library-contracts.ts";
import { App } from "../../src/app/App.tsx";
import "../../src/app/styles.css";
import type { LibraryCursor, LibraryFilter, LibraryMedia, TagMutation, TagQuery } from "../../src/api/library-contracts.ts";
const tags = new Map([["t1", "旅行"], ["t2", "灵感"], ["t3", "待整理"]]);
const links = new Map<string, Set<string>>();
let generation = 1;
let queries = 0;
let largestBatch = 0;
let pageLimit = 0;
const systems = [{ id: "root", name: "Synthetic Collection", parentId: null, hasChildren: true, attached: false }, { id: "child", name: "Photographs", parentId: "root", hasChildren: true, attached: false }, { id: "leaf", name: "Landscapes", parentId: "child", hasChildren: false, attached: false }];
function item(index: number): LibraryMedia {
  const id = `m${String(index).padStart(6, "0")}`;
  const attached = links.get(id) ?? new Set(index % 3 === 0 ? ["t1"] : []);
  return { id, fileName: `${["Alpine light", "Quiet coast", "Field notes", "Studio study", "Evening walk", "Still moments"][index % 6]}_${String(index).padStart(6, "0")}.${index % 5 === 0 ? "mp4" : "jpg"}`, mediaType: index % 5 === 0 ? "video" : "image", importedAt: 100000 - index, likeCount: index % 24, width: 320, height: 180, duration: index % 5 === 0 ? 12 : null, customTags: [...attached].filter(t => tags.has(t)).map(t => ({ id: t, name: tags.get(t)! })), customTagCount: [...attached].filter(t => tags.has(t)).length };
}
const fixtureInvoke = async (command: string, args: Record<string, unknown>): Promise<unknown> => {
  if (command === "query_library") {
    queries++; const filter = args.filter as LibraryFilter; const cursor = args.cursor as LibraryCursor | null; const limit = Number(args.limit); pageLimit = Math.max(pageLimit, limit);
    if (cursor && cursor.generation !== generation) throw { code: "stale_cursor", message: "媒体库已更新，请刷新当前列表。", retryable: true };
    const items: LibraryMedia[] = [];
    let i = cursor ? Number(cursor.id.slice(1)) + 1 : 0;
    for (; i < 100000 && items.length < limit + 1; i++) {
      const m = item(i); if (filter.mediaType && m.mediaType !== filter.mediaType) continue;
      if (filter.search && !m.fileName.toLowerCase().includes(filter.search.toLowerCase())) continue;
      if (filter.systemTagId === "leaf" && i > 24999) continue;
      const wanted = [...filter.customTagIds, ...(filter.customTagId ? [filter.customTagId] : [])];
      const matches = (id: string) => m.customTags.some(tag => tag.id === id);
      if (wanted.length && !(filter.customTagMode === "and" ? wanted.every(matches) : wanted.some(matches))) continue;
      items.push(m);
    }
    const more = items.length > limit; items.length = Math.min(items.length, limit); const last = items.at(-1);
    return { items, generation, nextCursor: more && last ? { generation, signature: "fixture", id: last.id, key: String(last.importedAt) } : null };
  }
  if (command === "list_library_tags") {
    const q = args.query as TagQuery;
    const all = q.kind === "system" ? systems.filter(t => t.parentId === q.parentId) : [...tags].map(([id, name]) => ({ id, name, parentId: null, hasChildren: false, attached: q.mediaId ? item(Number(q.mediaId.slice(1))).customTags.some(t => t.id === id) : false }));
    const items = all.filter(t => t.id > (q.afterId ?? "") && t.name.includes(q.search)).slice(0, q.limit);
    return { items, nextAfter: null };
  }
  if (command === "get_media_details") return { media: item(Number(String(args.id).slice(1))), filePath: `/synthetic/read-only/${args.id}.jpg`, fileSize: 2048000, videoCodec: "h264", audioCodec: "aac", container: "mp4", frameRate: 30 };
  if (command === "get_thumbnails") {
    const ids = args.ids as string[]; largestBatch = Math.max(largestBatch, ids.length);
    return ids.map(id => ({ mediaId: id, status: "ready", path: `/cache/thumbnails/${Number(id.slice(1)).toString(16).padStart(64, "0")}.jpg`, error: null }));
  }
  if (command === "mutate_custom_tag") {
    const m = args.mutation as TagMutation;
    if (m.action === "create") tags.set(`t${Date.now()}`, m.name);
    else if (m.action === "rename") tags.set(m.id, m.name);
    else if (m.action === "delete") { tags.delete(m.id); for (const set of links.values()) set.delete(m.id); }
    else for (const id of m.mediaIds) { const set = new Set(item(Number(id.slice(1))).customTags.map(t => t.id)); for (const tag of m.tagIds) { if (m.action === "attach") set.add(tag); else set.delete(tag); } links.set(id, set); }
    generation++; return null;
  }
  throw new Error(`Unexpected fixture command: ${command}`);
};
// Mock the application adapter rather than modifying Tauri's immutable native internals.
Object.assign(libraryApi, {
  query: async (filter: LibraryFilter, cursor: LibraryCursor | null, limit: number) => parseLibraryPage(await fixtureInvoke("query_library", { filter, cursor, limit })),
  details: async (id: string) => parseDetails(await fixtureInvoke("get_media_details", { id })),
  tags: async (query: TagQuery) => parseTagPage(await fixtureInvoke("list_library_tags", { query })),
  mutate: async (mutation: TagMutation) => { await fixtureInvoke("mutate_custom_tag", { mutation }); },
  thumbnails: async (ids: string[]) => parseThumbnails(await fixtureInvoke("get_thumbnails", { ids })),
  thumbnailUrl: (path: string) => `/tests/ui/assets/${parseInt(path.split("/").at(-1)!.slice(0, 64), 16) % 6}.jpg`,
});
createRoot(document.getElementById("root")!).render(<App />);

// DOM-level regression runner, activated only with ?run=1. Results remain visible for inspection.
if (new URLSearchParams(location.search).has("run")) {
  const reports: string[] = [];
  const wait = async (check: () => boolean) => { const until = Date.now() + 10000; while (!check()) { if (Date.now() > until) throw new Error(`DOM wait timed out after: ${reports.at(-1) ?? "initial mount"}`); await new Promise(r => setTimeout(r, 25)); } };
  const clickText = (text: string) => { const button = [...document.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent?.trim() === text); if (!button) throw new Error(`Missing button ${text}`); button.click(); };
  const assert = (value: boolean, message: string) => { if (!value) throw new Error(message); reports.push(message); };
  const input = (element: HTMLInputElement, value: string) => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(element, value); element.dispatchEvent(new Event("input", { bubbles: true })); };
  void (async () => {
    try {
      await wait(() => document.querySelectorAll(".media-card img").length > 0);
      assert(document.querySelectorAll(".media-card").length < 100, "Virtual grid mounts fewer than one page of cards");
      assert([...document.images].every(img => img.loading === "lazy" && img.src.includes("/tests/ui/assets/")), "Only lazy thumbnail images are loaded");
      const firstId = document.querySelector(".file-name")!.textContent;
      clickText("下一页 →"); await wait(() => document.querySelector(".file-name")?.textContent !== firstId && document.querySelectorAll(".media-card").length > 0);
      assert(document.querySelector(".library-footer")!.textContent!.includes("第 2 页"), "Incremental next page replaces current data");
      clickText("← 上一页"); await wait(() => document.querySelector(".file-name")?.textContent === firstId);
      const scroll = document.querySelector<HTMLDivElement>(".grid-scroll")!; scroll.scrollTop = 3000; scroll.dispatchEvent(new Event("scroll", { bubbles: true })); await new Promise(r => setTimeout(r, 100));
      assert(document.querySelectorAll(".media-card").length < 100, "Virtualization remains bounded after scrolling");
      scroll.scrollTop = 0; scroll.dispatchEvent(new Event("scroll", { bubbles: true })); await new Promise(r => setTimeout(r, 100));
      document.querySelector<HTMLButtonElement>(".file-name")!.click(); await wait(() => !!document.querySelector(".detail-name"));
      assert(document.querySelector(".file-path")!.textContent!.startsWith("/synthetic/"), "Details show metadata without loading the original");
      document.querySelector<HTMLButtonElement>('[aria-label="管理 Custom Tags"]')!.click(); await wait(() => !!document.querySelector('[aria-label="新标签名称"]'));
      input(document.querySelector('[aria-label="新标签名称"]')!, "UI regression tag"); await new Promise(r => setTimeout(r, 50)); clickText("创建"); await wait(() => [...document.querySelectorAll(".tag-modal .tag-option")].some(row => row.textContent!.includes("UI regression tag")));
      assert([...tags.values()].includes("UI regression tag"), "Create tag uses the typed mutation API");
      const row = [...document.querySelectorAll(".tag-modal .tag-option")].find(e => e.textContent!.includes("UI regression tag"))!;
      [...row.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "重命名")!.click(); await wait(() => !!document.querySelector(".rename-form input"));
      input(document.querySelector(".rename-form input")!, "Renamed UI tag"); await new Promise(r => setTimeout(r, 50)); clickText("保存"); await wait(() => [...document.querySelectorAll(".tag-modal .tag-option")].some(row => row.textContent!.includes("Renamed UI tag") && !row.querySelector("input"))); assert([...tags.values()].includes("Renamed UI tag"), "Rename updates the flat tag list");
      const deleteRow = [...document.querySelectorAll(".tag-modal .tag-option")].find(e => e.textContent!.includes("Renamed UI tag"))!;
      [...deleteRow.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "删除")!.click(); await wait(() => [...document.querySelectorAll("button")].some(b => b.textContent === "确认删除")); clickText("确认删除"); await wait(() => !document.querySelector(".tag-modal .tag-selector [role=alert]") && ![...document.querySelectorAll(".tag-modal .tag-option")].some(row => row.textContent!.includes("Renamed UI tag")) && !!document.querySelector(".tag-modal [role=status]")); assert(![...tags.values()].includes("Renamed UI tag"), "Delete removes the tag through the API");
      document.querySelector<HTMLButtonElement>('[aria-label="关闭标签窗口"]')!.click(); await wait(() => !document.querySelector(".tag-modal")); await wait(() => !!document.querySelector(".media-card"));
      const selections = document.querySelectorAll<HTMLInputElement>(".card-select"); selections[0]!.click(); selections[1]!.click(); await wait(() => document.querySelector(".selection-actions")!.textContent!.includes("已选 2/200")); clickText("批量编辑标签"); await wait(() => document.querySelectorAll(".tag-modal .tag-option").length >= 3);
      // Add/remove are explicit operations on the selection; neither is a Like toggle.
      const selectedRow = [...document.querySelectorAll(".tag-modal .tag-option")].find(row => row.textContent!.includes("灵感"))!;
      const add = [...selectedRow.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "全部添加")!; add.click();
      await wait(() => !!document.querySelector(".tag-modal [role=status]") && document.querySelectorAll(".tag-modal .tag-option").length >= 3);
      assert(links.get("m000000")?.has("t2") === true && links.get("m000001")?.has("t2") === true, "Batch add affects both selected media");
      const removeRow = [...document.querySelectorAll(".tag-modal .tag-option")].find(row => row.textContent!.includes("灵感"))!;
      [...removeRow.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "全部移除")!.click();
      await wait(() => document.querySelector(".tag-modal [role=status]")?.textContent?.includes("移除") === true && document.querySelectorAll(".tag-modal .tag-option").length >= 3);
      assert(!links.get("m000000")?.has("t2") && !links.get("m000001")?.has("t2"), "Batch remove affects both selected media");
      document.querySelector<HTMLButtonElement>('[aria-label="关闭标签窗口"]')!.click();
      assert(pageLimit === 100 && largestBatch <= 100, `Requests stay bounded: page=${pageLimit}, thumbnail batch=${largestBatch}`);
      const output = document.createElement("pre"); output.id = "ui-test-results"; output.style.cssText = "position:fixed;bottom:0;left:0;z-index:100;background:#173b2c;color:white;padding:12px;font-size:11px;max-height:35vh;overflow:auto"; output.textContent = `PASS ${reports.length} UI assertions; ${queries} queries\n${reports.join("\n")}`; document.body.append(output);
      document.title = `PASS ${reports.length} — Media Library UI Tests`;
    } catch (error) { const output = document.createElement("pre"); output.id = "ui-test-results"; output.style.cssText = "position:fixed;inset:20px;z-index:100;background:#fff0ed;color:#8a3027;padding:20px"; output.textContent = `FAIL ${String(error)}\n${reports.join("\n")}`; document.body.append(output); document.title = "FAIL — Media Library UI Tests"; }
  })();
}
