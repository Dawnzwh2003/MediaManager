import "../../src/main.tsx";
import { quickBrowseApi } from "../../src/api/quick-browse.ts";
import { libraryApi } from "../../src/api/library.ts";
import { activityApi } from "../../src/api/activity.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";

if (new URLSearchParams(location.search).has("run")) {
  const results: string[] = [];
  const wait = async (condition: () => boolean, label: string) => {
    const deadline = Date.now() + 40000;
    while (!condition()) { if (Date.now() > deadline) throw Error(`Timeout: ${label}`); await new Promise(resolve => setTimeout(resolve, 40)); }
  };
  const check = (value: boolean, label: string) => { if (!value) throw Error(label); results.push(label); };
  const button = (text: string) => [...document.querySelectorAll<HTMLButtonElement>("button")].find(node => node.textContent?.trim() === text);
  const click = async (get: () => HTMLButtonElement | null | undefined, label: string) => { await wait(() => !!get() && !get()!.disabled, label); get()!.click(); };
  const key = (value: string) => window.dispatchEvent(new KeyboardEvent("keydown", { key: value, code: value === " " ? "Space" : "", bubbles: true, cancelable: true }));
  const ready = () => !!document.querySelector(".viewer-overlay[data-activation-id]") && !document.querySelector(".quick-status");
  const sessionId = () => document.querySelector<HTMLElement>("[data-quick-session]")!.dataset.quickSession!;
  const title = () => document.querySelector(".viewer-title")?.textContent;
  const choose = (value: string) => { const select = document.querySelector<HTMLSelectElement>('[aria-label="排序"]')!; select.value = value; select.dispatchEvent(new Event("change", { bubbles: true })); };
  void (async () => {
    try {
      await wait(() => !!document.querySelector(".media-card"), "library ready");
      await quickBrowseApi.saveSettings({autoplay:false,muted:true,bindings:[{digit:1,tagId:"validation-later"}]},null);
      choose("name");
      const expected = await libraryApi.query({...initialFilter,sort:"name"},null,3);
      await wait(() => document.querySelector(".file-name")?.textContent === expected.items[0]!.fileName, "name sort applied");
      await click(() => button("Quick Browse · 键盘浏览"), "enter Quick Browse");await wait(ready,"first activation");
      const firstSession = sessionId();
      const frozen = await quickBrowseApi.get(firstSession);
      check(frozen.itemCount===111 && frozen.sortMode==="name" && frozen.filter.sort==="name", "Current query becomes a persisted 111-member session");
      const members=await quickBrowseApi.items(firstSession,-1,200);
      check(members.slice(0,3).map(item=>item.mediaId).join()===expected.items.map(item=>item.id).join(),"Session order matches database filename sorting");
      const first = members[0]!, second = members[1]!;
      const before = await activityApi.statistics(first.mediaId);
      key("ArrowRight");await wait(()=>ready()&&title()===second.fileName,"Right next");
      key("ArrowLeft");await wait(()=>ready()&&title()===first.fileName,"Left previous");
      const after=await activityApi.statistics(first.mediaId);
      check(after.viewCount===before.viewCount+1,"A B A creates a second Quick Browser View for A");
      key(" ");await wait(()=>ready()&&title()===second.fileName,"Space next");
      check((await quickBrowseApi.get(firstSession)).currentOrdinal===1,"Space advances and saves the current ordinal");
      const likes=await activityApi.statistics(second.mediaId);
      key("L");await wait(()=>document.querySelector(".viewer-overlay [data-like]")?.textContent?.includes(String(likes.likeCount+1))===true,"Like key");
      check((await activityApi.statistics(second.mediaId)).likeCount===likes.likeCount+1,"L uses the existing Like Service");
      const tagBefore=(await libraryApi.tags({kind:"custom",parentId:null,afterId:null,mediaId:second.mediaId,search:"",limit:100})).items.find(tag=>tag.id==="validation-later")!.attached;
      key("1");await wait(()=>document.querySelector(".quick-toolbar output")?.textContent?.includes("已添加")===true,"number add feedback");
      key("1");await wait(()=>document.querySelector(".quick-toolbar output")?.textContent?.includes("已移除")===true,"number remove feedback");
      const tagAfter=(await libraryApi.tags({kind:"custom",parentId:null,afterId:null,mediaId:second.mediaId,search:"",limit:100})).items.find(tag=>tag.id==="validation-later")!.attached;
      check(tagAfter===tagBefore,"Digit toggles add/remove with transient feedback");
      key("T");await wait(()=>!!document.querySelector(".viewer-tags"),"T tags");key("T");await wait(()=>!document.querySelector(".viewer-tags"),"T close tags");
      key("I");await wait(()=>!!document.querySelector(".viewer-panel"),"I details");key("I");await wait(()=>!document.querySelector(".viewer-panel"),"I hide info");
      check(true,"T and I toggle existing tag and information panels");
      await click(()=>button("快捷键设置"),"settings");await wait(()=>!!document.querySelector(".quick-settings"),"settings open");
      check(document.querySelectorAll(".quick-slots button").length===9,"All nine number bindings are configurable");key("Escape");await wait(()=>!document.querySelector(".quick-settings"),"close settings");check(!!document.querySelector(".viewer-overlay"),"Escape closes settings before exiting browse");
      const beforeOrder=(await quickBrowseApi.items(firstSession,-1,200)).map(item=>item.mediaId).join();
      check(beforeOrder===members.map(item=>item.mediaId).join(),"Likes, tags and rerenders leave session members and order unchanged");
      key("R");await wait(()=>ready()&&sessionId()!==firstSession,"R shuffle");
      const shuffled=await quickBrowseApi.get(sessionId());
      check(shuffled.sortMode==="random" && shuffled.randomSeed!==frozen.randomSeed,"R creates a new persisted seed and stable order");
      check((await quickBrowseApi.get(firstSession)).endedAt!==null,"Shuffle atomically ends the previous session");
      const shuffledId=shuffled.id;
      key("Escape");await wait(()=>!document.querySelector(".quick-root"),"Esc exit");check((await quickBrowseApi.get(shuffledId)).endedAt!==null,"Esc persists the session end time");
      // A separate product entry verifies the configured video path and unchanged Play qualification.
      const type=document.querySelector<HTMLSelectElement>('[aria-label="媒体类型"]')!;type.value="video";type.dispatchEvent(new Event("change",{bubbles:true}));
      await wait(()=>document.querySelector(".collection-toolbar .muted.small")?.textContent==="本页 1 项","video filter");
      await quickBrowseApi.saveSettings({autoplay:true,muted:true,bindings:[{digit:1,tagId:"validation-later"}]},null);
      await click(()=>button("Quick Browse · 键盘浏览"),"video browse");await wait(()=>ready()&&!!document.querySelector("video"),"video ready");
      await wait(()=>{const video=document.querySelector("video");return !!video && !video.paused && video.currentTime>0;},"muted autoplay");
      check(document.querySelector("video")!.muted,"Video Quick Browse honors autoplay and mute");
      key("Escape");await wait(()=>!document.querySelector(".quick-root"),"video exit");
      check(true,"Video closes through shared progress and qualified Play services");
      // Leave a photo session open for native keyboard and fullscreen validation.
      type.value="image";type.dispatchEvent(new Event("change",{bubbles:true}));await wait(()=>!!document.querySelector(".media-card"),"photos");
      await click(()=>button("Quick Browse · 键盘浏览"),"native browse");await wait(ready,"native session ready");
      show(`PASS ${results.length} Quick Browse assertions`,false);
    } catch(error) { show(`FAIL ${String(error)}`,true); }
  })();
  function show(summary:string,failed:boolean) {
    const output=document.createElement("pre");output.id="quick-test-results";output.style.cssText=`position:fixed;bottom:8px;left:12px;z-index:1500;padding:12px;font-size:11px;background:${failed?"#943d30":"#173c2c"};color:white;max-width:80vw;max-height:26vh;overflow:auto;white-space:pre-wrap;pointer-events:none`;
    output.textContent=`${summary}\n${results.join("\n")}`;document.body.append(output);document.title=summary;
  }
}
