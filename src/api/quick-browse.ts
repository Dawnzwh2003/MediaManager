import { invoke, isTauri } from "@tauri-apps/api/core";
import { toApiError } from "./contracts.ts";
import type { LibraryFilter } from "./library-contracts.ts";
import { parseViewerOpen } from "./viewer.ts";

export interface BrowseSession {
  id: string;
  startedAt: number;
  endedAt: number | null;
  sortMode: string;
  filter: LibraryFilter;
  randomSeed: string | null;
  algorithmVersion: number;
  state: "building" | "ready" | "closed";
  phase: string;
  itemCount: number;
  builtCount: number;
  currentOrdinal: number | null;
  autoplay: boolean;
  muted: boolean;
  error: string | null;
  parentSessionId: string | null;
}
export interface BrowseItem { ordinal: number; mediaId: string; contentVersionId: number; fileName: string; available: boolean }
export interface TagBinding { digit: number; tagId: string }
export interface ResolvedBinding extends TagBinding { name: string }
export interface BrowsePreferences { autoplay: boolean; muted: boolean; bindings: TagBinding[] }
export interface BrowseSettings extends BrowsePreferences { bindings: ResolvedBinding[] }
export interface ToggleTagRequest { requestId: string; sessionId: string; ordinal: number; digit: number }
export interface ToggleTagResult { mediaId: string; tagId: string; tagName: string; attached: boolean }
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object";
const integer = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const nullableText = (value: unknown) => value === null || typeof value === "string";
function invalid(): never { throw { code: "invalid_response", message: "快速浏览返回的数据格式不正确。", retryable: true }; }
export function parseBrowseSession(value: unknown): BrowseSession {
  if (!record(value) || typeof value.id !== "string" || !integer(value.startedAt)
    || !(value.endedAt === null || integer(value.endedAt)) || typeof value.sortMode !== "string"
    || !record(value.filter) || typeof value.filter.search !== "string"
    || !(value.randomSeed === null || typeof value.randomSeed === "string" && /^[a-f0-9]{32}$/.test(value.randomSeed))
    || value.algorithmVersion !== 1 || !["building", "ready", "closed"].includes(String(value.state))
    || !["pending", "spooling", "publishing", "cleaning", "complete"].includes(String(value.phase))
    || !integer(value.itemCount) || !integer(value.builtCount)
    || !(value.currentOrdinal === null || integer(value.currentOrdinal) && value.currentOrdinal < value.itemCount)
    || typeof value.autoplay !== "boolean" || typeof value.muted !== "boolean"
    || !nullableText(value.error) || !nullableText(value.parentSessionId)) invalid();
  return value as unknown as BrowseSession;
}
export function parseBrowseItems(value: unknown): BrowseItem[] {
  if (!Array.isArray(value) || value.length > 200 || !value.every(item => record(item)
    && integer(item.ordinal) && typeof item.mediaId === "string" && integer(item.contentVersionId)
    && item.contentVersionId > 0 && typeof item.fileName === "string" && typeof item.available === "boolean")) invalid();
  return value as BrowseItem[];
}
export function parseBrowseSettings(value: unknown): BrowseSettings {
  if (!record(value) || typeof value.autoplay !== "boolean" || typeof value.muted !== "boolean"
    || !Array.isArray(value.bindings) || value.bindings.length > 9
    || !value.bindings.every(binding => record(binding) && integer(binding.digit)
      && binding.digit >= 1 && binding.digit <= 9 && typeof binding.tagId === "string" && typeof binding.name === "string")
    || new Set(value.bindings.map(binding => binding.digit)).size !== value.bindings.length) invalid();
  return value as unknown as BrowseSettings;
}
export function parseTagToggle(value: unknown): ToggleTagResult {
  if (!record(value) || typeof value.mediaId !== "string" || typeof value.tagId !== "string"
    || typeof value.tagName !== "string" || typeof value.attached !== "boolean") invalid();
  return value as unknown as ToggleTagResult;
}
async function call(command: string, args: Record<string, unknown>): Promise<unknown> {
  if (!isTauri()) throw { code: "desktop_required", message: "请在桌面端使用快速浏览。", retryable: false };
  try { return await invoke(command, args); } catch (error) { throw toApiError(error); }
}
export const quickBrowseApi = {
  create: async (requestId: string, filter: LibraryFilter, preferences: BrowsePreferences) => parseBrowseSession(await call("create_quick_browse", { request: { requestId, filter, autoplay: preferences.autoplay, muted: preferences.muted } })),
  get: async (sessionId: string) => parseBrowseSession(await call("get_quick_browse", { sessionId })),
  items: async (sessionId: string, after: number, limit = 1) => parseBrowseItems(await call("quick_browse_items", { sessionId, after, limit })),
  select: async (sessionId: string, ordinal: number) => { await call("select_quick_browse", { sessionId, ordinal }); },
  shuffle: async (sessionId: string, requestId: string) => parseBrowseSession(await call("shuffle_quick_browse", { sessionId, requestId })),
  close: async (sessionId: string) => { await call("close_quick_browse", { sessionId }); },
  settings: async () => parseBrowseSettings(await call("get_quick_browse_settings", {})),
  saveSettings: async (preferences: BrowsePreferences, sessionId: string | null) => parseBrowseSettings(await call("save_quick_browse_settings", { preferences, sessionId })),
  toggle: async (request: ToggleTagRequest) => parseTagToggle(await call("toggle_quick_browse_tag", { request })),
  open: async (sessionId: string, ordinal: number) => parseViewerOpen(await call("open_quick_browse_item", { sessionId, ordinal })),
};
