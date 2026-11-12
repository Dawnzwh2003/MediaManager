// Product App, real SQLite and Tauri IPC. Run only with the isolated Phase 10 seed.
import "../../src/main.tsx";
import { reportsApi } from "../../src/api/reports.ts";
import { activityApi } from "../../src/api/activity.ts";
import { libraryApi } from "../../src/api/library.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";
import { viewerApi } from "../../src/api/viewer.ts";
import { timeWindow } from "../../src/reports/model.ts";
import type { Period, ReportKind } from "../../src/api/report-contracts.ts";

if (new URLSearchParams(location.search).has("run")) {
  const results: string[] = [];
  const wait = async (condition: () => boolean, name: string) => {
    const end = Date.now() + 30000;
    while (!condition()) { if (Date.now() > end) throw new Error(`Timeout: ${name}`); await new Promise(resolve => setTimeout(resolve, 40)); }
  };
  const check = (value: boolean, name: string) => { if (!value) throw new Error(name); results.push(name); };
  const button = (text: string) => [...document.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent?.trim() === text);
  const click = async (text: string) => { await wait(() => !!button(text) && !button(text)!.disabled, text); button(text)!.click(); };
  const settled = () => wait(() => !document.querySelector('[role="status"]') && !!document.querySelector(".reports-workspace .report-list"), "report loaded");
  const choose = (value: ReportKind) => { const select = document.querySelector<HTMLSelectElement>('[aria-label="History 分类"]')!; select.value = value; select.dispatchEvent(new Event("change", { bubbles: true })); };
  const visibleIds = () => [...document.querySelectorAll<HTMLElement>(".reports-workspace .report-row")].map(e => e.dataset.mediaId);
  void (async () => {
    try {
      await wait(() => !!document.querySelector(".grid-scroll"), "initial library");
      // Replays keep committed history; reset only this synthetic video's recovery
      // position through the product API, with no activation or Play creation.
      const videos = await libraryApi.query({ ...initialFilter, mediaType: "video" }, null, 100);
      const clip = videos.items.find(item => item.fileName === "Synthetic clip.mp4");
      if (!clip) throw new Error("Phase 10 synthetic clip is missing");
      const recovery = await viewerApi.open(clip.id);
      try { await viewerApi.save({ token: recovery.token, sequence: 1, expectedVersion: recovery.state!.stateVersion, positionSeconds: 7.5, durationSeconds: 30 }); }
      finally { await viewerApi.close(recovery.token); }
      const baseline = await reportsApi.dashboard(timeWindow("today"));
      await click("Dashboard"); await settled();
      for (const [label, value] of [["Total Media", baseline.totalMedia], ["Images", baseline.images], ["Videos", baseline.videos], ["Total Likes", baseline.totalLikes], ["Views Today", baseline.viewsToday]] as const) {
        check(document.querySelector(`[data-metric="${label}"]`)?.textContent === value.toLocaleString(), `Dashboard ${label} matches SQLite`);
      }
      check(document.querySelectorAll(".dashboard-panel").length === 5 && document.querySelectorAll(".dashboard-panel .report-row").length <= 30, "Five dashboard previews are bounded");
      await click("History"); await settled();
      for (const kind of ["recentlyViewed", "recentlyPlayed", "mostViewed", "mostPlayed", "mostLiked"] as const) {
        choose(kind);
        for (const [period, label] of [["today", "Today"], ["week", "Last 7 Days"], ["month", "Last 30 Days"], ["all", "All Time"]] as const satisfies readonly (readonly [Period, string])[]) {
          await click(label);
          const expected = await reportsApi.query({ kind, window: timeWindow(period) }, null, 100);
          await wait(() => document.querySelector(".report-caption .muted")?.textContent === `本页 ${expected.items.length} 项`
            && visibleIds().every((id, i) => id === expected.items[i]?.id)
            && [...document.querySelectorAll(".report-value strong")].every((node, i) => node.textContent === expected.items[i]?.count.toLocaleString()), `${kind}/${period}`);
        }
        check(true, `${kind}: all four ranges show SQLite ordering and counts`);
      }
      choose("recentlyViewed");
      await wait(() => document.querySelector(".report-caption .muted")?.textContent === "本页 100 项", "history first page");
      check(document.querySelectorAll(".report-row").length < 100, "History mounts only visible rows");
      await click("下一页 →"); await wait(() => document.querySelector(".report-caption .muted")?.textContent === "本页 12 项", "history second page");
      await click("← 上一页"); await wait(() => document.querySelector(".report-caption .muted")?.textContent === "本页 100 项", "history previous page");
      check(true, "History keyset next/previous preserves both pages");
      const target = (await reportsApi.query({kind:"recentlyViewed",window:timeWindow("all")},null,1)).items[0]!;
      const previous = await activityApi.statistics(target.id);
      document.querySelector<HTMLButtonElement>(`.report-row[data-media-id="${target.id}"] .report-open`)!.click();
      await wait(() => !!document.querySelector(".viewer-overlay[data-activation-id]"), "history activation");
      await click("← 返回媒体库"); await wait(() => !document.querySelector(".viewer-overlay"), "history close");
      check((await activityApi.statistics(target.id)).viewCount === previous.viewCount + 1, "Opening History records one activation View");
      await click("Continue Watching"); await settled();
      check(document.querySelectorAll(".report-row").length === 1 && document.querySelector(".report-row")?.textContent?.includes("Synthetic clip.mp4") === true, "Continue Watching excludes the completed video");
      check(document.querySelector<HTMLProgressElement>(".report-row progress")?.value === 25, "Continue Watching displays 7.5 / 30 seconds and 25 percent");
      document.querySelector<HTMLButtonElement>(".report-open")!.click();
      await wait(() => !!document.querySelector(".viewer-overlay[data-activation-id]") && !!document.querySelector<HTMLVideoElement>("video")?.videoWidth && (document.querySelector<HTMLVideoElement>("video")?.readyState ?? 0) >= 2, "resume decoded video");
      const video = document.querySelector<HTMLVideoElement>("video")!;
      await wait(() => Math.abs(video.currentTime - 7.5) < 0.2, "resumed position");
      check(true, "Continue Watching opens a decoded video at saved position");
      video.currentTime = 29;
      video.dispatchEvent(new Event("seeked"));
      await click("← 返回媒体库"); await wait(() => !document.querySelector(".viewer-overlay"), "save progress");
      await wait(() => document.querySelector(".report-empty")?.textContent?.includes("没有未完成的视频") === true, "completed video removed");
      check(true, "Seek/save completion removes video without requiring a Play event");
      await click("Dashboard"); await settled();
      const after = await reportsApi.dashboard(timeWindow("today"));
      check(after.viewsToday === baseline.viewsToday + 2 && after.totalLikes === baseline.totalLikes, "Refreshing Dashboard reflects new Views without extra Likes");
      show(`PASS ${results.length} report assertions`, false);
    } catch (error) { show(`FAIL ${String(error)}`, true); }
  })();
  function show(summary: string, failed: boolean) {
    const output = document.createElement("pre"); output.id = "reports-test-results";
    output.style.cssText = `position:fixed;bottom:0;left:0;z-index:100;padding:12px;font-size:11px;background:${failed ? "#a14739" : "#173c2c"};color:white;max-width:88vw;max-height:38vh;overflow:auto;white-space:pre-wrap`;
    output.textContent = `${summary}\n${results.join("\n")}`; document.body.append(output); document.title = summary;
  }
}
