import assert from "node:assert/strict";
import { test } from "node:test";
import { quickShortcut, TagShortcutQueue } from "../../src/quick-browse/model.ts";
import { parseBrowseSession, parseBrowseItems, parseBrowseSettings, parseTagToggle } from "../../src/api/quick-browse.ts";
import type { ToggleTagRequest } from "../../src/api/quick-browse.ts";
import { initialFilter } from "../../src/api/library-contracts.ts";
import { ActivationController } from "../../src/viewer/activity-model.ts";

test("all Quick Browse keys map once per active press, never browser command chords", () => {
  const key = { key: "", code: "", repeat: false, ctrlKey: false, altKey: false, metaKey: false };
  for (const [input, action] of [["ArrowLeft","previous"],["ArrowRight","next"],[" ","next"],["L","like"],["t","tags"],["F","fullscreen"],["i","info"],["r","shuffle"],["Escape","exit"],["1",1],["9",9]] as const) assert.equal(quickShortcut({ ...key, key: input }), action);
  for (const modifier of ["repeat","ctrlKey","altKey","metaKey"] as const) assert.equal(quickShortcut({ ...key, key: "1", [modifier]: true }), null);
  assert.equal(quickShortcut({ ...key, key: "0" }), null);
});
test("rapid number presses preserve every toggle and retry exactly the failed request", async () => {
  const requests: ToggleTagRequest[] = []; const feedback: boolean[] = []; let fail = true; let attached = false;
  const queue = new TagShortcutQueue("session", 22, async request => { requests.push({ ...request }); if (fail) throw Error("lost response"); attached = !attached; return { mediaId: "A", tagId: "tag", tagName: "Keep", attached }; }, result => feedback.push(result.attached));
  await assert.rejects(queue.toggle(1));
  await assert.rejects(queue.toggle(1));
  assert.deepEqual(requests[0], requests[1]); fail = false;
  await queue.flush();
  assert.deepEqual(feedback, [true,false]);
  assert.equal(new Set(requests.map(request => request.requestId)).size, 2);
  assert.ok(requests.every(request => request.sessionId === "session" && request.ordinal === 22));
});
test("quick activations survive rerenders and settings while A B A gets new views", () => {
  let next = 0; const controller = new ActivationController("quick-session", () => `v${++next}`);
  const a = controller.activate("A", 1, "quick_browser");
  assert.equal(controller.activate("A", 1, "quick_browser"), a);
  const b = controller.activate("B", 1, "quick_browser");
  const again = controller.activate("A", 1, "quick_browser");
  assert.deepEqual([a,b,again].map(item => item.ordinal), [1,2,3]);
  assert.ok([a,b,again].every(item => item.source === "quick_browser"));
});
test("quick IPC guards bound pages, unique digit bindings and stable session state", () => {
  const session = {id:"s",startedAt:1,endedAt:null,sortMode:"random",filter:initialFilter,randomSeed:"0".repeat(32),algorithmVersion:1,state:"ready",phase:"complete",itemCount:100000,builtCount:100000,currentOrdinal:0,autoplay:false,muted:true,error:null,parentSessionId:null};
  assert.equal(parseBrowseSession(session).itemCount,100000);
  assert.throws(() => parseBrowseSession({...session,randomSeed:"x"}));
  assert.throws(() => parseBrowseSession({...session,currentOrdinal:100000}));
  const item = {ordinal:0,mediaId:"A",contentVersionId:1,fileName:"a.png",available:true};
  assert.equal(parseBrowseItems([item]).length,1);assert.throws(() => parseBrowseItems(Array(201).fill(item)));
  assert.throws(() => parseBrowseSettings({autoplay:false,muted:true,bindings:[{digit:1,tagId:"a",name:"A"},{digit:1,tagId:"b",name:"B"}]}));
  assert.equal(parseTagToggle({mediaId:"A",tagId:"a",tagName:"A",attached:false}).attached,false);
});
