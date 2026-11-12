import { invoke, isTauri } from "@tauri-apps/api/core";
import { toApiError } from "./contracts.ts";
import { parseDashboard, parseReportPage } from "./report-contracts.ts";
import type { ReportCursor, ReportQuery, TimeWindow } from "./report-contracts.ts";
async function call(command: string, args: Record<string, unknown>): Promise<unknown> {
  if (!isTauri()) throw { code: "desktop_required", message: "请在桌面应用中查看统计。", retryable: false };
  try { return await invoke<unknown>(command, args); } catch (error) { throw toApiError(error); }
}
export const reportsApi = {
  query: async (query: ReportQuery, cursor: ReportCursor | null, limit: number) => parseReportPage(await call("query_reports", { query, cursor, limit })),
  dashboard: async (today: TimeWindow) => parseDashboard(await call("get_dashboard", { today })),
};
