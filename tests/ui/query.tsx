// Product UI and real Tauri IPC on an isolated synthetic library.
import "../../src/main.tsx";
import { libraryApi } from "../../src/api/library.ts";
import { activityApi } from "../../src/api/activity.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";

if (new URLSearchParams(location.search).has("run")) {
  const results: string[] = [];
  const wait = async (condition: () => boolean, name: string) => {
    const deadline = Date.now() + 30000;
    while (!condition()) {
      if (Date.now() > deadline) throw new Error(`Timeout: ${name}`);
      await new Promise(resolve => setTimeout(resolve, 40));
    }
  };
  const check = (condition: boolean, name: string) => {
    if (!condition) throw new Error(name);
    results.push(name);
  };
  const textButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>("button")]
    .find(button => button.textContent?.trim() === text);
  const click = async (getButton: () => HTMLButtonElement | null | undefined, name: string) => {
    await wait(() => { const button = getButton(); return !!button && !button.disabled; }, name);
    getButton()!.click();
  };
  const choose = (selector: string, value: string) => {
    const select = document.querySelector<HTMLSelectElement>(selector)!;
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  };
  const input = (selector: string, value: string) => {
    const element = document.querySelector<HTMLInputElement>(selector)!;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const count = (amount: number) => wait(() => amount === 0
    ? document.querySelector(".library-empty h2")?.textContent === "没有匹配的媒体"
    : document.querySelector(".collection-toolbar .muted.small")?.textContent === `本页 ${amount} 项`, `page count ${amount}`);
  const visible = () => [...document.querySelectorAll(".file-name")].slice(0, 3).map(node => node.textContent).join("|");
  const sort = (value: string) => choose('[aria-label="排序"]', value);
  const scope = (value: string) => choose('[aria-label="搜索范围"]', value);
  const search = (value: string) => input('[aria-label="搜索媒体"]', value);

  void (async () => {
    try {
      await count(100);
      scope("filename"); search("alpine"); await count(19);
      check(visible().toLowerCase().includes("alpine"), "Filename scope searches the actual catalog");
      scope("systemTag"); search("Landscapes"); await count(100);
      await click(() => textButton("下一页 →"), "directory search next"); await count(10);
      check(true, "System Tag search finds all 110 descendants across pages");
      scope("customTag"); search("旅行"); await count(37);
      check(true, "Custom Tag search finds attached media");
      search(""); await count(100);
      await click(() => textButton("旅行"), "travel filter"); await count(37);
      await click(() => textButton("灵感"), "ideas filter"); await count(8);
      choose('[aria-label="Custom Tag 匹配"]', "or"); await count(52);
      check(true, "Custom Tag AND and OR show the intersection and union");
      await click(() => document.querySelector<HTMLButtonElement>(".all-media"), "clear tag filters"); await count(100);
      choose('[aria-label="媒体类型"]', "video"); await count(1);
      check(visible().includes("Synthetic clip.mp4"), "Media type filters to the single video");
      choose('[aria-label="媒体类型"]', "all"); await count(100);

      sort("recentlyViewed");
      const beforeView = await libraryApi.query({ ...initialFilter, sort: "recentlyViewed" }, null, 3);
      const [, selected, next] = beforeView.items;
      await wait(() => visible() === beforeView.items.map(item => item.fileName).join("|"), "Recently Viewed ordering");
      await click(() => [...document.querySelectorAll<HTMLButtonElement>(".preview-button")]
        .find(button => button.getAttribute("aria-label") === `打开 ${selected!.fileName}`), "open middle media");
      await wait(() => document.querySelector(".viewer-title")?.textContent === selected!.fileName, "middle title");
      await click(() => document.querySelector<HTMLButtonElement>('[aria-label="下一个媒体"]'), "next in Recently Viewed");
      await wait(() => document.querySelector(".viewer-title")?.textContent === next!.fileName, "pre-View neighbor");
      check(true, "Recording a View keeps navigation anchored at the entry sort key");
      await click(() => textButton("← 返回媒体库"), "close viewer");
      await wait(() => !document.querySelector(".viewer-overlay"), "viewer closed");

      const top = await libraryApi.query({ ...initialFilter, sort: "likes" }, null, 1);
      const targetLikes = top.items[0]!.likeCount + 1;
      let liked = await activityApi.addLike(selected!.id, crypto.randomUUID());
      while (liked.likeCount < targetLikes) liked = await activityApi.addLike(selected!.id, crypto.randomUUID());
      await click(() => textButton("筛选"), "advanced filters");
      input('[name="likes-min"]', String(liked.likeCount));
      input('[name="views-min"]', "1");
      input('[name="plays-min"]', "0");
      const day = new Date(selected!.importedAt).toLocaleDateString("sv-SE");
      input('[name="imported-min"]', day); input('[name="imported-max"]', day);
      choose('[name="viewed"]', "viewed");
      await click(() => textButton("应用筛选"), "apply ranges"); await count(1);
      check(visible() === selected!.fileName, "Count ranges, inclusive dates and Viewed combine in SQLite");
      await click(() => textButton("筛选 (5)"), "applied filters");
      await click(() => textButton("清除范围和状态"), "clear ranges"); await count(100);

      sort("random");
      const seedKey = "media-manager.library-random-v1";
      const seed = localStorage.getItem(seedKey) ?? initialFilter.random.seed;
      const randomPage = await libraryApi.query({ ...initialFilter, sort: "random", random: { seed, version: 1 } }, null, 3);
      const order = randomPage.items.map(item => item.fileName).join("|");
      await wait(() => visible() === order, "first random page");
      await click(() => textButton("下一页 →"), "random next"); await count(11);
      await click(() => textButton("← 上一页"), "random previous");
      await wait(() => visible() === order, "return to same random order");
      await click(() => document.querySelector<HTMLButtonElement>('[aria-label="刷新媒体库"]'), "random refresh");
      await wait(() => visible() === order, "refresh keeps random order");
      check((localStorage.getItem(seedKey) ?? initialFilter.random.seed) === seed, "Paging and refreshing reuse the same seed");
      await click(() => textButton("Shuffle Again"), "shuffle");
      const shuffled = localStorage.getItem(seedKey)!;
      await wait(() => !!visible() && visible() !== order, "new random order");
      check(/^[a-f0-9]{32}$/.test(shuffled) && shuffled !== seed, "Shuffle Again persists a new seed and changes the order");
      check(document.querySelectorAll(".media-card").length < 100, "The grid mounts only its virtual rows");
      show(`PASS ${results.length} query assertions`, false);
    } catch (error) {
      show(`FAIL ${String(error)}`, true);
    }
  })();

  function show(summary: string, failed: boolean) {
    const output = document.createElement("pre");
    output.id = "query-test-results";
    output.style.cssText = `position:fixed;bottom:0;left:0;z-index:100;padding:14px;font-size:12px;background:${failed ? "#a14739" : "#173c2c"};color:white;max-width:85vw;white-space:pre-wrap`;
    output.textContent = `${summary}\n${results.join("\n")}`;
    document.body.append(output);
    document.title = summary;
  }
}
