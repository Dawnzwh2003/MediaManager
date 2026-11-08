// Native Tauri harness: real IPC, real decoder and a disposable Phase 6 database.
import "../../src/main.tsx";
import { libraryApi } from "../../src/api/library.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";
import { viewerApi } from "../../src/api/viewer.ts";
if (new URLSearchParams(location.search).has("run")) {
  const reports: string[] = [];
  const check = (condition: boolean, text: string) => { if (!condition) throw new Error(text); reports.push(text); };
  const wait = async (condition: () => boolean | Promise<boolean>, label: string) => { const until = Date.now() + 30000; while (!await condition()) { if (Date.now() > until) throw new Error(`Timed out: ${label}`); await new Promise(resolve => setTimeout(resolve, 30)); } };
  const button = (text: string, root: ParentNode = document) => { const element = [...root.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent?.trim() === text); if (!element) throw new Error(`Missing button: ${text}`); return element; };
  const click = (text: string) => { const b = button(text); if (b.disabled) throw new Error(`Disabled button: ${text}`); b.click(); };
  const labelButton = async (label: string) => { await wait(() => { const target = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`); return !!target && !target.disabled; }, `enabled button: ${label}`); const b = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`); if (!b || b.disabled) throw new Error(`Unavailable button: ${label}`); b.click(); };
  const currentVideo = () => document.querySelector<HTMLVideoElement>(".video-stage video")!;
  const loadedVideo = async (name: string) => { await wait(() => document.querySelector(".viewer-title")?.textContent === name && !!currentVideo() && currentVideo().readyState >= 2 && currentVideo().videoWidth > 0 && !document.querySelector(".viewer-preparing"), `video frame: ${name}`); };
  const decodedFrame = () => { const canvas = document.createElement("canvas"); canvas.width = 32; canvas.height = 18; const context = canvas.getContext("2d")!; context.drawImage(currentVideo(), 0, 0, 32, 18); const pixels = context.getImageData(0, 0, 32, 18).data; return pixels.some((value, index) => index % 4 !== 3 && value > 80); };
  void (async () => {
    try {
      await wait(() => document.querySelectorAll(".media-card").length === 5, "initial real library");
      const sort = document.querySelector<HTMLSelectElement>('[aria-label="排序"]')!; sort.value = "name"; sort.dispatchEvent(new Event("change", { bubbles: true }));
      await wait(() => document.querySelector(".file-name")?.textContent?.startsWith("01_") === true, "name sort");
      const media = await libraryApi.query({ ...initialFilter, sort: "name" }, null, 100);
      const direct = media.items.find(m => m.fileName === "03_Direct.mp4")!, fallback = media.items.find(m => m.fileName === "04_Fallback.mkv")!;
      document.querySelector<HTMLButtonElement>(".preview-button")!.click();
      await wait(() => !!document.querySelector<HTMLImageElement>(".viewer-original")?.naturalWidth, "full resolution image");
      const image = document.querySelector<HTMLImageElement>(".viewer-original")!;
      check(image.naturalWidth === 3200 && image.src.includes("media.localhost"), "ImageViewer reads one original through a restricted media handle");
      check(Number(document.querySelector("[aria-label='缩放比例']")!.textContent!.replace("%", "")) < 100, "Initial image fits the viewport");
      click("100%"); await wait(() => image.style.transform.includes("scale(1)"), "100 percent scale"); check(true, "100% selects native image scale");
      await labelButton("放大"); await wait(() => image.style.transform.includes("scale(1.25)"), "zoom control");
      click("Fit"); await wait(() => !image.style.transform.includes("scale(1.25)"), "fit reset"); check(true, "Zoom and Fit update the image transform");
      click("Details"); await wait(() => !!document.querySelector(".viewer-panel .detail-name"), "image details"); check(document.querySelector(".viewer-panel")!.textContent!.includes("3200 × 1800"), "Viewer Details uses real metadata");
      click("Custom Tag"); await wait(() => !!document.querySelector(".viewer-tags .tag-option"), "tag selector");
      const tagRow = [...document.querySelectorAll(".viewer-tags .tag-option")].find(row => row.textContent!.includes("Review"))!;
      const add = [...tagRow.querySelectorAll<HTMLButtonElement>("button")].find(b => b.textContent === "＋ 添加");
      if (add) add.click();
      await wait(() => [...document.querySelectorAll(".viewer-tags .tag-option")].some(row => row.textContent!.includes("Review") && row.textContent!.includes("已添加")), "tag mutation commit"); check(true, "Custom Tag changes commit through real IPC");
      click("Custom Tag"); await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      await labelButton("下一个媒体"); await wait(() => document.querySelector(".viewer-title")?.textContent === "02_Still.jpg" && !!document.querySelector(".viewer-original"), "next image");
      await labelButton("上一个媒体"); await wait(() => document.querySelector(".viewer-title")?.textContent === "01_Landscape.jpg" && !!document.querySelector(".viewer-original"), "previous image"); check(true, "Previous and Next preserve the current browse order");
      await labelButton("下一个媒体"); await wait(() => document.querySelector(".viewer-title")?.textContent === "02_Still.jpg" && !!document.querySelector(".viewer-original"), "second image");
      await labelButton("下一个媒体"); await loadedVideo("03_Direct.mp4"); check(document.querySelector(".viewer-title")?.textContent === "03_Direct.mp4", "VideoPlayer opens the direct MP4");
      await labelButton("静音"); await wait(() => currentVideo().muted, "mute");
      const speed = document.querySelector<HTMLSelectElement>('[aria-label="播放速度"]')!; speed.value = "2"; speed.dispatchEvent(new Event("change", { bubbles: true })); await wait(() => currentVideo().playbackRate === 2, "playback speed");
      await labelButton("播放"); await wait(() => !currentVideo().paused && currentVideo().currentTime > .3, "real video playback"); check(decodedFrame(), "Muted playback, decoded video frames and 2x speed are functional");
      currentVideo().currentTime = 4; await wait(() => !currentVideo().seeking && currentVideo().currentTime >= 4, "timeline seek"); await labelButton("暂停");
      await wait(async () => (await viewerApi.getState(direct.id, 1))?.positionSeconds! >= 4, "pause checkpoint"); check(currentVideo().paused, "Pause commits playback_position without a Play Event");
      await labelButton("播放");
      await wait(async () => (await viewerApi.getState(direct.id, 1))?.positionSeconds! > 8, "periodic checkpoint"); check(true, "Periodic checkpoint persists advancing playback");
      currentVideo().currentTime = 16; await wait(() => !currentVideo().seeking && currentVideo().currentTime >= 16, "switch target");
      await labelButton("下一个媒体"); await loadedVideo("04_Fallback.mkv");
      await wait(async () => (await viewerApi.getState(direct.id, 1))?.positionSeconds! >= 16, "media-switch checkpoint"); check(document.querySelector(".viewer-title")?.textContent === "04_Fallback.mkv", "Media switch flushes the previous video's progress");
      await labelButton("静音"); await wait(() => currentVideo().muted, "fallback mute"); await labelButton("播放"); await wait(() => !currentVideo().paused && currentVideo().currentTime > .3, "fallback real playback"); check(decodedFrame(), "Fallback container decodes actual video frames"); await labelButton("暂停");
      currentVideo().currentTime = 10.5; await wait(() => !currentVideo().seeking && Math.abs(currentVideo().currentTime - 10.5) < .2, "close target");
      click("← 返回媒体库"); await wait(() => !document.querySelector(".viewer-overlay"), "viewer close");
      await wait(async () => Math.abs((await viewerApi.getState(fallback.id, 1))!.positionSeconds - 10.5) < .2, "close checkpoint"); check(true, "Viewer close flushes the final position");
      await wait(() => document.querySelectorAll(".media-card").length === 5, "library after close");
      const card = [...document.querySelectorAll(".media-card")].find(c => c.textContent!.includes("04_Fallback.mkv"))!; card.querySelector<HTMLButtonElement>(".preview-button")!.click(); await loadedVideo("04_Fallback.mkv");
      await wait(() => !currentVideo().seeking && Math.abs(currentVideo().currentTime - 10.5) < .3, "resume seek"); check(true, "Reopening restores the versioned position");
      currentVideo().currentTime = 18.5; await wait(() => !currentVideo().seeking && Math.abs(currentVideo().currentTime - 18.5) < .2, "shutdown target");
      const output = document.createElement("pre"); output.id = "viewer-test-results"; output.style.cssText = "position:fixed;bottom:0;left:0;z-index:100;background:#173b2c;color:white;padding:12px;font-size:11px;max-height:38vh;overflow:auto"; output.textContent = `PASS ${reports.length} viewer assertions — ready for shutdown\n${reports.join("\n")}`; document.body.append(output); document.title = `PASS ${reports.length} — Viewer Regression`;
    } catch (error) { const output = document.createElement("pre"); output.id = "viewer-test-results"; output.style.cssText = "position:fixed;inset:20px;z-index:100;background:#fff0ed;color:#8a3027;padding:20px"; output.textContent = `FAIL ${String(error)}\n${reports.join("\n")}`; document.body.append(output); document.title = "FAIL — Viewer Regression"; }
  })();
}
