// Production App and IPC, with request/DOM/event-loop observation only.
import "../../src/main.tsx";
import { libraryApi } from "../../src/api/library.ts";
import { reportsApi } from "../../src/api/reports.ts";
import { activityApi } from "../../src/api/activity.ts";
import { timeWindow } from "../../src/reports/model.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";

if (new URLSearchParams(location.search).has("run")) {
  const size = Number(new URLSearchParams(location.search).get("size"));
  const results: string[] = [];
  const metrics = { size, documentTimeOrigin: performance.timeOrigin, firstGridMs: 0, maxCards: 0, maxThumbnailBatch: 0, uniqueThumbnailIds: 0, maxPage: 0, maxPayloadBytes: 0, queryCount: 0, randomFirstMs: 0, maxHeartbeatGapMs: 0, heapBytes: null as number | null };
  const thumbs = new Set<string>();
  const originalQuery = libraryApi.query, originalThumbnails = libraryApi.thumbnails;
  let latestSort = "", lastBeat = performance.now();
  libraryApi.query = async (filter, cursor, limit) => {
    const started = performance.now();
    const page = await originalQuery(filter, cursor, limit);
    metrics.queryCount++;
    metrics.maxPage = Math.max(metrics.maxPage, limit, page.items.length);
    metrics.maxPayloadBytes = Math.max(metrics.maxPayloadBytes, JSON.stringify(page).length);
    if (filter.sort === "random" && !metrics.randomFirstMs) metrics.randomFirstMs = performance.now() - started;
    latestSort = filter.sort;
    return page;
  };
  libraryApi.thumbnails = async (ids, retry) => {
    metrics.maxThumbnailBatch = Math.max(metrics.maxThumbnailBatch, ids.length);
    ids.forEach(id => thumbs.add(id));
    return originalThumbnails(ids, retry);
  };
  const observer = new MutationObserver(() => { metrics.maxCards = Math.max(metrics.maxCards, document.querySelectorAll(".media-card").length); });
  observer.observe(document.documentElement, { subtree: true, childList: true });
  const heartbeat = setInterval(() => { const now = performance.now(); metrics.maxHeartbeatGapMs = Math.max(metrics.maxHeartbeatGapMs, now - lastBeat); lastBeat = now; }, 16);
  const wait = async (condition: () => boolean, label: string) => { const end = performance.now() + 30000; while (!condition()) { if (performance.now() > end) throw new Error(`Timeout: ${label}`); await new Promise(resolve => setTimeout(resolve, 25)); } };
  const check = (value: boolean, label: string) => { if (!value) throw new Error(label); results.push(label); };
  const button = (label: string) => [...document.querySelectorAll<HTMLButtonElement>("button")].find(node => node.textContent?.trim() === label);
  const click = async (label: string) => { await wait(() => !!button(label) && !button(label)!.disabled, label); button(label)!.click(); };
  const frames = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  const settle = async (action: () => void | Promise<void>, label: string) => {
    const previous = metrics.queryCount;
    await action();
    await wait(() => metrics.queryCount > previous && !document.querySelector<HTMLButtonElement>('[aria-label="刷新媒体库"]')?.disabled, label);
    await frames();
  };
  const page = (n: number) => wait(() => document.querySelector(".collection-toolbar .muted.small")?.textContent === `本页 ${n} 项` && !document.querySelector<HTMLButtonElement>('[aria-label="刷新媒体库"]')?.disabled, `page ${n}`);
  const input = (selector: string, text: string) => { const node = document.querySelector<HTMLInputElement>(selector)!; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(node, text); node.dispatchEvent(new Event("input", { bubbles: true })); };
  const choose = (selector: string, value: string) => { const node = document.querySelector<HTMLSelectElement>(selector)!; node.value = value; node.dispatchEvent(new Event("change", { bubbles: true })); };
  const names = () => [...document.querySelectorAll(".file-name")].slice(0, 3).map(node => node.textContent).join("|");
  void (async () => {
    try {
      await page(100); metrics.firstGridMs = performance.now(); await frames();
      const totals = await reportsApi.dashboard(timeWindow("today"));
      check(totals.totalMedia === size, "The connected SQLite fixture contains the requested media count");
      check(document.querySelectorAll(".media-card").length < 100, "Initial Grid renders only visible rows");
      const scroll = document.querySelector<HTMLDivElement>(".grid-scroll")!;
      scroll.scrollTop = 10000000; scroll.dispatchEvent(new Event("scroll", { bubbles: true })); await frames();
      check(document.querySelectorAll(".media-card").length < 100 && scroll.scrollTop > 0, "Deep scrolling keeps Grid DOM bounded");
      for (let n = 2; n <= 11; n++) { await click("下一页 →"); await wait(() => document.querySelector(".library-footer")?.textContent?.includes(`第 ${n} 页`) === true, `next ${n}`); await page(100); }
      for (let n = 10; n >= 8; n--) { await click("← 上一页"); await wait(() => document.querySelector(".library-footer")?.textContent?.includes(`第 ${n} 页`) === true, `previous ${n}`); await page(100); }
      check(true, "Paging replaces a bounded page, including deep next/previous navigation");
      choose('[aria-label="搜索范围"]', "filename"); input('[aria-label="搜索媒体"]', "rare-"); await page(Math.floor(size / 997));
      await wait(() => names().includes("rare-"), "sparse search result rendered");
      check(names().includes("rare-"), "Sparse filename search is performed in SQLite");
      input('[aria-label="搜索媒体"]', "no-match-phase11");
      await wait(() => document.querySelector(".library-empty h2")?.textContent === "没有匹配的媒体", "empty search");
      check(document.querySelectorAll(".media-card").length === 0, "No-match search releases the previous page");
      input('[aria-label="搜索媒体"]', ""); await page(100);
      choose('[aria-label="排序"]', "random"); await wait(() => latestSort === "random", "random query"); await page(100); await frames();
      const order = names();
      await settle(() => document.querySelector<HTMLButtonElement>('[aria-label="刷新媒体库"]')!.click(), "random refresh completed"); await page(100);
      check(names() === order, "Random refresh keeps the same stable seed and order");
      await settle(() => click("Travel"), "first tag query completed"); await page(100); await settle(() => click("Ideas"), "AND query completed"); await page(100);
      check(true, "AND tag filters load a bounded SQL page");
      await settle(() => choose('[aria-label="Custom Tag 匹配"]', "or"), "OR query completed"); await page(100);
      check(true, "OR tag filters load a bounded SQL page");
      document.querySelector<HTMLButtonElement>(".all-media")!.click(); await page(100);
      choose('[aria-label="排序"]', "recent"); await wait(() => latestSort === "recent", "recent query"); await page(100);
      const first = await originalQuery({ ...initialFilter }, null, 1);
      await activityApi.addLike(first.items[0]!.id, crypto.randomUUID());
      await click("下一页 →");
      await wait(() => document.querySelector(".library-empty h2")?.textContent === "媒体库已有更新", "stale cursor");
      await click("重新加载"); await page(100);
      check(true, "A concurrent event invalidates old cursors and refresh recovers");
      metrics.uniqueThumbnailIds = thumbs.size;
      metrics.heapBytes = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? null;
      check(metrics.maxPage === 100 && metrics.maxPayloadBytes < 100000, "Responses are paginated and bounded independently of catalog size");
      check(metrics.maxCards < 100 && metrics.maxThumbnailBatch <= 100 && thumbs.size < size / 2, "Grid and thumbnail requests never expand to the full catalog");
      check(metrics.maxHeartbeatGapMs < 1000, "The UI event loop stays responsive throughout querying and scrolling");
      finish(`PASS ${results.length} performance assertions`, false);
    } catch (error) { finish(`FAIL ${String(error)}`, true); }
  })();
  function finish(summary: string, failed: boolean) {
    clearInterval(heartbeat); observer.disconnect(); libraryApi.query = originalQuery; libraryApi.thumbnails = originalThumbnails;
    const output = document.createElement("pre"); output.id = "phase11-results";
    output.style.cssText = `position:fixed;bottom:0;left:0;z-index:100;padding:12px;font-size:11px;background:${failed ? "#a14739" : "#173c2c"};color:white;max-width:90vw;max-height:42vh;overflow:auto;white-space:pre-wrap`;
    output.textContent = `${summary}\n${JSON.stringify(metrics)}\n${results.join("\n")}`; document.body.append(output); document.title = summary;
  }
}
