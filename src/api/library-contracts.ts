export interface TagLabel { id: string; name: string }
export type LibrarySort = "recent" | "importedAsc" | "name" | "nameDesc" | "likes" | "likesAsc"
  | "views" | "viewsAsc" | "plays" | "playsAsc" | "created" | "createdAsc"
  | "recentlyViewed" | "recentlyPlayed" | "random";
export type SearchScope = "all" | "filename" | "systemTag" | "customTag";
export interface InclusiveRange { min: number | null; max: number | null }
export interface RandomOrder { seed: string; version: 1 }
export interface LibraryFilter {
  search: string;
  searchScope: SearchScope;
  mediaType: "image" | "video" | null;
  systemTagId: string | null;
  sourceTagId: string | null;
  customTagId: string | null;
  customTagIds: string[];
  customTagMode: "and" | "or";
  likes: InclusiveRange | null;
  views: InclusiveRange | null;
  plays: InclusiveRange | null;
  imported: InclusiveRange | null;
  viewed: "all" | "viewed" | "unviewed";
  sort: LibrarySort;
  random: RandomOrder;
}
export interface LibraryCursor { generation: number; signature: string; id: string; key: string }
export interface LibraryMedia { id: string; fileName: string; mediaType: "image" | "video"; importedAt: number; likeCount: number; width: number | null; height: number | null; duration: number | null; customTags: TagLabel[]; customTagCount: number }
export interface LibraryPage { items: LibraryMedia[]; nextCursor: LibraryCursor | null; generation: number }
export interface LibraryDetails { media: LibraryMedia; filePath: string; fileSize: number; videoCodec: string | null; audioCodec: string | null; container: string | null; frameRate: number | null }
export interface LibraryTag extends TagLabel { parentId: string | null; hasChildren: boolean; attached: boolean }
export interface TagPage { items: LibraryTag[]; nextAfter: string | null }
export interface TagQuery { kind: "system" | "custom"; parentId: string | null; afterId: string | null; mediaId: string | null; search: string; limit: number }
export type TagMutation = { action: "create"; name: string } | { action: "rename"; id: string; name: string } | { action: "delete"; id: string } | { action: "attach" | "detach"; mediaIds: string[]; tagIds: string[] };
export interface Thumbnail { mediaId: string; status: "pending" | "running" | "ready" | "failed" | "unavailable"; path: string | null; error: string | null }

export const initialFilter: LibraryFilter = {
  search: "", searchScope: "all", mediaType: null, systemTagId: null, sourceTagId: null, customTagId: null,
  customTagIds: [], customTagMode: "and", likes: null, views: null, plays: null, imported: null,
  viewed: "all", sort: "recent", random: { seed: "00000000000000000000000000000000", version: 1 },
};
const record = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;
const integer = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const stringOrNull = (v: unknown) => v === null || typeof v === "string";
const numberOrNull = (v: unknown) => v === null || (typeof v === "number" && Number.isFinite(v) && v >= 0);
function invalid(): never { throw { code: "invalid_response", message: "媒体库返回的数据格式不正确。", retryable: true }; }
function label(v: unknown): v is TagLabel { return record(v) && typeof v.id === "string" && typeof v.name === "string"; }
function media(v: unknown): v is LibraryMedia {
  return record(v) && typeof v.id === "string" && typeof v.fileName === "string" && (v.mediaType === "image" || v.mediaType === "video") && integer(v.importedAt) && integer(v.likeCount) && numberOrNull(v.width) && numberOrNull(v.height) && numberOrNull(v.duration) && Array.isArray(v.customTags) && v.customTags.length <= 8 && v.customTags.every(label) && integer(v.customTagCount) && v.customTagCount >= v.customTags.length;
}
function cursor(v: unknown): v is LibraryCursor { return record(v) && integer(v.generation) && typeof v.signature === "string" && typeof v.id === "string" && typeof v.key === "string"; }
export function parseLibraryPage(v: unknown): LibraryPage {
  if (!record(v) || !Array.isArray(v.items) || v.items.length > 200 || !v.items.every(media) || !integer(v.generation) || !(v.nextCursor === null || cursor(v.nextCursor))) invalid();
  return v as unknown as LibraryPage;
}
export function parseDetails(v: unknown): LibraryDetails {
  if (!record(v) || !media(v.media) || typeof v.filePath !== "string" || !integer(v.fileSize) || !stringOrNull(v.videoCodec) || !stringOrNull(v.audioCodec) || !stringOrNull(v.container) || !numberOrNull(v.frameRate)) invalid();
  return v as unknown as LibraryDetails;
}
export function parseTagPage(v: unknown): TagPage {
  if (!record(v) || !Array.isArray(v.items) || v.items.length > 200 || !v.items.every(t => label(t) && record(t) && stringOrNull(t.parentId) && typeof t.hasChildren === "boolean" && typeof t.attached === "boolean") || !stringOrNull(v.nextAfter)) invalid();
  return v as unknown as TagPage;
}
export function parseThumbnails(v: unknown): Thumbnail[] {
  if (!Array.isArray(v) || v.length > 100 || !v.every(t => record(t) && typeof t.mediaId === "string" && ["pending", "running", "ready", "failed", "unavailable"].includes(String(t.status)) && stringOrNull(t.path) && stringOrNull(t.error) && (t.status !== "ready" || (typeof t.path === "string" && /[\\/][a-f0-9]{64}\.jpg$/.test(t.path))))) invalid();
  return v as Thumbnail[];
}
