import assert from "node:assert/strict";
import { test } from "node:test";
import { ProgressSaver, clampPan, fitScale, zoomAt, formatTime } from "../../src/viewer/model.ts";
import type { PlaybackState, ProgressWrite } from "../../src/api/viewer.ts";
import { parsePlaybackState, parseViewerOpen, parseNavigation } from "../../src/api/viewer.ts";
const initial: PlaybackState = { positionSeconds: 0, durationSeconds: 100, stateVersion: 1 };
test("checkpoint queue serializes saves and coalesces in-flight progress", async () => {
  const requests: ProgressWrite[] = []; const resolvers: ((state: PlaybackState) => void)[] = [];
  const saver = new ProgressSaver("owner", initial, request => { requests.push(request); return new Promise(resolve => resolvers.push(resolve)); });
  const first = saver.checkpoint({ positionSeconds: 5, durationSeconds: 100 });
  const second = saver.checkpoint({ positionSeconds: 10, durationSeconds: 100 });
  const last = saver.checkpoint({ positionSeconds: 15, durationSeconds: 100 });
  assert.equal(requests.length, 1); resolvers[0]!({ ...initial, positionSeconds: 5, stateVersion: 2 });
  await new Promise(resolve => setImmediate(resolve)); assert.equal(requests.length, 2);
  assert.deepEqual(requests[1], { token: "owner", sequence: 2, expectedVersion: 2, positionSeconds: 15, durationSeconds: 100 });
  resolvers[1]!({ ...initial, positionSeconds: 15, stateVersion: 3 }); await Promise.all([first, second, last]);
  await saver.checkpoint({ positionSeconds: 15, durationSeconds: 100 }); assert.equal(requests.length, 2);
});
test("failed checkpoint retries identical identity before the latest snapshot", async () => {
  const requests: ProgressWrite[] = []; let fail = true;
  const saver = new ProgressSaver("old-session", initial, async request => { requests.push({ ...request }); if (fail) throw new Error("injected IPC failure"); return { positionSeconds: request.positionSeconds, durationSeconds: 100, stateVersion: request.expectedVersion + 1 }; });
  await assert.rejects(saver.checkpoint({ positionSeconds: 7, durationSeconds: 100 }));
  fail = false; await saver.checkpoint({ positionSeconds: 3, durationSeconds: 100 });
  assert.deepEqual(requests[0], requests[1]); assert.equal(requests[2]?.sequence, 2); assert.equal(requests[2]?.expectedVersion, 2); assert.equal(requests[2]?.positionSeconds, 3);
});
test("close flush waits for latest state and separate media never share owners", async () => {
  const writes: ProgressWrite[] = [];
  const save = async (request: ProgressWrite) => { writes.push(request); return { positionSeconds: request.positionSeconds, durationSeconds: 100, stateVersion: request.expectedVersion + 1 }; };
  const a = new ProgressSaver("A", initial, save), b = new ProgressSaver("B", initial, save);
  await a.checkpoint({ positionSeconds: 6, durationSeconds: 100 }); await a.flush(); await b.checkpoint({ positionSeconds: 2, durationSeconds: 100 });
  assert.deepEqual(writes.map(w => [w.token, w.sequence]), [["A", 1], ["B", 1]]);
  await assert.rejects(a.checkpoint({ positionSeconds: NaN, durationSeconds: 100 }));
  await assert.rejects(a.checkpoint({ positionSeconds: -1, durationSeconds: 100 }));
});
test("fit, 100%, pointer anchored zoom and pan have finite bounds", () => {
  assert.equal(fitScale(4000, 2000, 1040, 640), .25);
  assert.equal(fitScale(100, 100, 1040, 640), 1);
  assert.deepEqual(zoomAt({ scale: 1, x: 0, y: 0 }, 2, 100, 0), { scale: 2, x: -100, y: 0 });
  assert.deepEqual(clampPan({ scale: 1, x: 9999, y: -9999 }, 2000, 1000, 1000, 500), { scale: 1, x: 500, y: -250 });
  assert.deepEqual(clampPan({ scale: .1, x: 20, y: 20 }, 2000, 1000, 1000, 500), { scale: .1, x: 0, y: 0 });
  assert.equal(zoomAt({ scale: 1, x: 0, y: 0 }, 1000).scale, 16); assert.equal(formatTime(125.9), "2:05"); assert.equal(formatTime(Infinity), "0:00");
});
test("viewer IPC guards reject invalid versions, ownership and state payloads", () => {
  const valid = { token: "a".repeat(32), mediaId: "m", contentVersionId: 1, mediaType: "video", mime: "video/mp4", state: initial };
  assert.equal(parseViewerOpen(valid).mediaType, "video"); assert.equal(parseNavigation({ previousId: null, nextId: "n", generation: 0, key: "0" }).nextId, "n");
  for (const value of [{ ...valid, token: "../../private" }, { ...valid, contentVersionId: 0 }, { ...valid, state: null }, { ...valid, mediaType: "image" }]) assert.throws(() => parseViewerOpen(value));
  for (const state of [{ ...initial, positionSeconds: NaN }, { ...initial, stateVersion: 0 }, { ...initial, durationSeconds: -1 }]) assert.throws(() => parsePlaybackState(state));
});
