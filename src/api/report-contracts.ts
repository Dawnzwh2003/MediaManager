export type ReportKind = "recentlyViewed" | "recentlyPlayed" | "mostViewed" | "mostPlayed" | "mostLiked" | "continueWatching" | "recentlyAdded";
export type Period = "today" | "week" | "month" | "all";
export type ReportSection = "library" | "history" | "continue" | "dashboard";
export interface TimeWindow { start: number | null; end: number }
export interface ReportQuery { kind: ReportKind; window: TimeWindow }
export interface ReportCursor { generation: number; signature: string; key: number; id: string }
export interface ReportItem { id: string; fileName: string; mediaType: "image" | "video"; state: "active" | "missing" | "superseded"; count: number; lastAt: number; position: number | null; duration: number | null }
export interface ReportPage { items: ReportItem[]; nextCursor: ReportCursor | null }
export interface Dashboard {
  totalMedia: number; images: number; videos: number; totalLikes: number; viewsToday: number;
  recentlyAdded: ReportItem[]; recentlyViewed: ReportItem[]; continueWatching: ReportItem[]; mostLiked: ReportItem[]; mostViewed: ReportItem[];
}
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object";
const count = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const seconds = (v: unknown) => v === null || (typeof v === "number" && Number.isFinite(v) && v >= 0);
function invalid(): never { throw { code: "invalid_response", message: "统计返回的数据格式不正确。", retryable: true }; }
function item(v: unknown): v is ReportItem {
  return record(v) && typeof v.id === "string" && typeof v.fileName === "string"
    && ["image", "video"].includes(String(v.mediaType)) && ["active", "missing", "superseded"].includes(String(v.state))
    && count(v.count) && count(v.lastAt) && seconds(v.position) && seconds(v.duration);
}
function items(v: unknown, max: number): v is ReportItem[] { return Array.isArray(v) && v.length <= max && v.every(item); }
export function parseReportPage(v: unknown): ReportPage {
  if (!record(v) || !items(v.items, 200)) invalid();
  const c = v.nextCursor;
  if (!(c === null || (record(c) && count(c.generation) && count(c.key) && typeof c.id === "string" && typeof c.signature === "string"))) invalid();
  return v as unknown as ReportPage;
}
export function parseDashboard(v: unknown): Dashboard {
  if (!record(v) || ![v.totalMedia, v.images, v.videos, v.totalLikes, v.viewsToday].every(count)
    || v.totalMedia !== (v.images as number) + (v.videos as number)
    || ![v.recentlyAdded, v.recentlyViewed, v.continueWatching, v.mostLiked, v.mostViewed].every(value => items(value, 6))) invalid();
  return v as unknown as Dashboard;
}
