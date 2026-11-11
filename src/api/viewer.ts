import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toApiError } from "./contracts.ts";
import type { LibraryFilter } from "./library-contracts.ts";
export interface PlaybackState { positionSeconds: number; durationSeconds: number | null; stateVersion: number }
export interface ViewerOpen { token: string; mediaId: string; contentVersionId: number; mediaType: "image" | "video"; mime: string; state: PlaybackState | null }
export type PlaybackMode = "direct" | "remux" | "proxy";
export interface ProgressWrite { token: string; sequence: number; expectedVersion: number; positionSeconds: number; durationSeconds: number | null }
export interface ViewerNavigation { previousId: string | null; nextId: string | null; generation: number; key: string }
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object";
const number = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v) && v >= 0;
const id = (v: unknown) => v === null || typeof v === "string";
function invalid(): never { throw { code: "invalid_response", message: "查看器返回的数据格式不正确。", retryable: true }; }
export function parsePlaybackState(v: unknown): PlaybackState {
  if (!record(v) || !number(v.positionSeconds) || !(v.durationSeconds === null || number(v.durationSeconds)) || !Number.isSafeInteger(v.stateVersion) || Number(v.stateVersion) < 1) invalid();
  return v as unknown as PlaybackState;
}
export function parseViewerOpen(v: unknown): ViewerOpen {
  if (!record(v) || typeof v.token !== "string" || !/^[a-f0-9]{32}$/.test(v.token) || typeof v.mediaId !== "string" || !Number.isSafeInteger(v.contentVersionId) || Number(v.contentVersionId) < 1 || !["image", "video"].includes(String(v.mediaType)) || typeof v.mime !== "string" || !(v.state === null || record(v.state))) invalid();
  if (v.state !== null) parsePlaybackState(v.state);
  if ((v.mediaType === "video") !== (v.state !== null)) invalid();
  return v as unknown as ViewerOpen;
}
export function parseNavigation(v: unknown): ViewerNavigation {
  if (!record(v) || !id(v.previousId) || !id(v.nextId) || !Number.isSafeInteger(v.generation) || Number(v.generation) < 0 || typeof v.key !== "string") invalid();
  return v as unknown as ViewerNavigation;
}
async function call(command: string, args: Record<string, unknown>): Promise<unknown> {
  if (!isTauri()) throw { code: "desktop_required", message: "请在桌面应用中打开媒体。", retryable: false };
  try { return await invoke(command, args); } catch (error) { throw toApiError(error); }
}
export const viewerApi = {
  windowFullscreen: async () => isTauri() ? getCurrentWindow().isFullscreen() : Boolean(document.fullscreenElement),
  setWindowFullscreen: async (enabled: boolean) => {
    if (isTauri()) { await getCurrentWindow().setFullscreen(enabled); return; }
    if (enabled) await document.documentElement.requestFullscreen();
    else if (document.fullscreenElement) await document.exitFullscreen();
  },
  open: async (id: string) => parseViewerOpen(await call("open_viewer", { id })),
  close: async (token: string) => { await call("close_viewer", { token }); },
  save: async (request: ProgressWrite) => parsePlaybackState(await call("save_playback_state", { request })),
  prepare: async (token: string, mode: PlaybackMode) => { await call("prepare_playback", { token, mode }); },
  navigation: async (filter: LibraryFilter, id: string, generation: number | null, anchor: string | null = null) =>
    parseNavigation(await call("viewer_navigation", { filter, id, generation, anchor })),
  resourceUrl: (token: string, mode: PlaybackMode) => `${convertFileSrc(token, "media")}/${mode}`,
  getState: async (id: string, contentVersionId: number): Promise<PlaybackState | null> => { const state = await call("get_playback_state", { id, contentVersionId }); return state === null ? null : parsePlaybackState(state); },
  onApplicationClose: async (flush: () => Promise<void>): Promise<() => void> => {
    if (!isTauri()) return () => {};
    const window = getCurrentWindow();
    return window.onCloseRequested(async event => {
      event.preventDefault();
      try { await Promise.race([flush().catch(() => {}), new Promise<void>(resolve => setTimeout(resolve, 3000))]); }
      finally { await window.destroy(); }
    });
  },
};
// Serialise ownership claims, including rapid close/reopen and StrictMode effect replay.
let opening: Promise<unknown> = Promise.resolve();
export function openViewer(id: string, open = () => viewerApi.open(id)): Promise<ViewerOpen> {
  const result = opening.then(open);
  opening = result.catch(() => {});
  return result;
}
