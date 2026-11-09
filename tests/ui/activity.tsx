// Real product entry, real IPC, synthetic isolated media only.
import "../../src/main.tsx";
import {libraryApi} from "../../src/api/library.ts";
import {activityApi} from "../../src/api/activity.ts";
import {initialFilter} from "../../src/api/library-contracts.ts";
if(new URLSearchParams(location.search).has("run")){
 const lines:string[]=[];
 const wait=async(work:()=>boolean|Promise<boolean>,label:string)=>{const end=Date.now()+30000;while(!await work()){if(Date.now()>end)throw Error(`Timeout ${label}`);await new Promise(r=>setTimeout(r,40))}};
 const check=(value:boolean,label:string)=>{if(!value)throw Error(label);lines.push(label)};
 const button=async(label:string)=>{await wait(()=>{const b=document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);return !!b&&!b.disabled},label);document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!.click()};
 const baseline=new Map<string,{likeCount:number;viewCount:number;playCount:number}>();
 const detail=async(id:string)=>{const value=await activityApi.statistics(id);const base=baseline.get(id);return base?{...value,likeCount:value.likeCount-base.likeCount,viewCount:value.viewCount-base.viewCount,playCount:value.playCount-base.playCount}:value;};
 void(async()=>{try{
  await wait(()=>document.querySelectorAll(".media-card").length===5,"grid");
  const sort=document.querySelector<HTMLSelectElement>('[aria-label="排序"]')!;sort.value="name";sort.dispatchEvent(new Event("change",{bubbles:true}));await wait(()=>document.querySelector(".file-name")?.textContent==="01_Landscape.jpg","sort");
  const page=await libraryApi.query({...initialFilter,sort:"name"},null,100);const image=page.items.find(m=>m.fileName==="01_Landscape.jpg")!,video=page.items.find(m=>m.fileName==="03_Direct.mp4")!;
  baseline.set(image.id,await activityApi.statistics(image.id));baseline.set(video.id,await activityApi.statistics(video.id));
  check((await detail(image.id)).viewCount===0,"Grid thumbnails do not record Views");
  const like=document.querySelector<HTMLButtonElement>(`[data-like="${image.id}"]`)!;for(let n=0;n<100;n++)like.click();await wait(async()=>(await detail(image.id)).likeCount===100,"100 button likes");check(true,"100 rapid button Likes create 100 durable events and counter increments");
  await button("打开 01_Landscape.jpg");await wait(async()=>!!document.querySelector(".viewer-original")&&(await detail(image.id)).viewCount===1,"first activation");
  window.dispatchEvent(new KeyboardEvent("keydown",{key:"l",bubbles:true}));await wait(async()=>(await detail(image.id)).likeCount===101,"L shortcut");await wait(()=>document.querySelector<HTMLButtonElement>(".viewer-header [data-like]")?.textContent?.includes((baseline.get(image.id)!.likeCount+101).toLocaleString())===true,"Like UI commit");check(true,"Like shortcut L uses the same service as the button");
  await button("下一个媒体");await wait(()=>document.querySelector(".viewer-title")?.textContent==="02_Still.jpg","B");await button("上一个媒体");await wait(async()=>document.querySelector(".viewer-title")?.textContent==="01_Landscape.jpg"&&(await detail(image.id)).viewCount===2,"return A");check(true,"A to B to A records a second A View without time debounce");
  window.dispatchEvent(new Event("resize"));await new Promise(r=>setTimeout(r,200));check((await detail(image.id)).viewCount===2,"Resize and rerender do not duplicate the activation");
  await button("下一个媒体");await wait(()=>document.querySelector(".viewer-title")?.textContent==="02_Still.jpg","second B");await button("下一个媒体");await wait(()=>document.querySelector(".viewer-title")?.textContent==="03_Direct.mp4"&&!!document.querySelector("video"),"video");
  check((await detail(video.id)).playCount===0,"Opening the VideoPlayer does not create a Play");
  const initialVideo=document.querySelector<HTMLVideoElement>("video")!;await wait(()=>initialVideo.readyState>=2,"decoded initial video");initialVideo.currentTime=0;await wait(()=>!initialVideo.seeking&&initialVideo.currentTime<0.1,"reset test position");
  await button("静音");await button("播放");await wait(()=>document.querySelector<HTMLVideoElement>("video")!.currentTime>.5,"real playback");await button("暂停");await new Promise(r=>setTimeout(r,250));check((await detail(video.id)).playCount===0,"A short ordinary-video play stays unqualified");
  await button("播放");await wait(async()=>(await detail(video.id)).playCount===1,"qualified playback");await button("暂停");check(true,"At least five real seconds qualify exactly one Play");
  const v=document.querySelector<HTMLVideoElement>("video")!;v.currentTime=29;await wait(()=>!v.seeking,"completed seek");
  const back=[...document.querySelectorAll<HTMLButtonElement>("button")].find(b=>b.textContent?.includes("返回媒体库"))!;back.click();await wait(()=>!document.querySelector(".viewer-overlay"),"close");check((await detail(video.id)).playCount===1,"Closing finalizes the same Play without recounting");
  const job=await activityApi.rebuild();await wait(async()=>(await activityApi.job(job.id)).state==="complete","statistics rebuild");check((await detail(image.id)).likeCount===101&&(await detail(image.id)).viewCount===2&&(await detail(video.id)).playCount===1,"Background statistics rebuild preserves event-derived counters");
  const output=document.createElement("pre");output.id="activity-test-results";output.style.cssText="position:fixed;bottom:0;left:0;z-index:100;background:#173c2c;color:white;padding:14px;font-size:12px;max-width:70vw";output.textContent=`PASS ${lines.length} Activity assertions\n${lines.join("\n")}`;document.body.append(output);document.title="PASS — Phase 7 Activity";
 }catch(e){const output=document.createElement("pre");output.id="activity-test-results";output.style.cssText="position:fixed;inset:20px;z-index:100;background:#fff0ed;color:#8a3027;padding:20px";output.textContent=`FAIL ${String(e)}\n${lines.join("\n")}`;document.body.append(output);document.title="FAIL — Phase 7 Activity";}})();
}
