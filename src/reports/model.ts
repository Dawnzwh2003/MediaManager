import type { Period, ReportItem, TimeWindow } from "../api/report-contracts.ts";

export function timeWindow(period: Period, now = new Date()): TimeWindow {
  // Calendar arithmetic preserves local midnight across 23/25-hour DST days.
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  if (period === "week") start.setDate(start.getDate() - 6);
  if (period === "month") start.setDate(start.getDate() - 29);
  return { start: period === "all" ? null : start.getTime(), end: now.getTime() + 1 };
}
export function progress(item: Pick<ReportItem, "position" | "duration">): number | null {
  return item.position !== null && item.duration !== null && item.duration > 0
    ? Math.min(100, Math.max(0, item.position / item.duration * 100)) : null;
}
export function durationText(value: number): string {
  const seconds = Math.floor(value);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
export function reportRows(count: number, top: number, height: number) {
  const start = Math.max(0, Math.floor(top / 96) - 3);
  return { start: Math.min(count, start), end: Math.min(count, Math.ceil((top + height) / 96) + 3) };
}
