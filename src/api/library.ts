import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { toApiError } from "./contracts.ts";
import { parseDetails, parseLibraryPage, parseTagPage, parseThumbnails } from "./library-contracts.ts";
import type { LibraryCursor, LibraryFilter, TagMutation, TagQuery } from "./library-contracts.ts";

async function call(command: string, args: Record<string, unknown>): Promise<unknown> {
  if (!isTauri()) throw { code: "desktop_required", message: "请在桌面应用中打开媒体库，以读取本地数据库。", retryable: false };
  try { return await invoke<unknown>(command, args); } catch (error) { throw toApiError(error); }
}
export const libraryApi = {
  thumbnailUrl: (path: string) => convertFileSrc(path),
  query: async (filter: LibraryFilter, cursor: LibraryCursor | null, limit: number) => parseLibraryPage(await call("query_library", { filter, cursor, limit })),
  details: async (id: string) => parseDetails(await call("get_media_details", { id })),
  tags: async (query: TagQuery) => parseTagPage(await call("list_library_tags", { query })),
  mutate: async (mutation: TagMutation) => { await call("mutate_custom_tag", { mutation }); },
  thumbnails: async (ids: string[], retry = false) => parseThumbnails(await call("get_thumbnails", { ids, retry })),
};
export function thumbnailUrl(path: string): string { return libraryApi.thumbnailUrl(path); }
export type LibraryApi = typeof libraryApi;
