import { useCallback, useEffect, useRef, useState } from "react";
import { reportsApi } from "../api/reports.ts";
import type { Dashboard, Period, ReportCursor, ReportItem, ReportKind, ReportPage, ReportQuery, ReportSection } from "../api/report-contracts.ts";
import type { ApiError } from "../api/contracts.ts";
import { toApiError } from "../api/contracts.ts";
import { initialFilter } from "../api/library-contracts.ts";
import { ViewerShell } from "../viewer/ViewerShell.tsx";
import { ReportList } from "./ReportList.tsx";
import { timeWindow } from "./model.ts";
import "./reports.css";

const historyKinds = ["recentlyViewed", "recentlyPlayed", "mostViewed", "mostPlayed", "mostLiked"] as const;
export const reportTitles: Record<ReportKind, string> = { recentlyViewed: "Recently Viewed", recentlyPlayed: "Recently Played", mostViewed: "Most Viewed", mostPlayed: "Most Played", mostLiked: "Most Liked", continueWatching: "Continue Watching", recentlyAdded: "Recently Added" };
const sections = ["recentlyAdded", "recentlyViewed", "continueWatching", "mostLiked", "mostViewed"] as const;

export function ReportsWorkspace({ section, revision, changed }: { section: Exclude<ReportSection, "library">; revision: number; changed: () => void }) {
  const [kind, setKind] = useState<ReportKind>(section === "continue" ? "continueWatching" : "recentlyViewed");
  const [period, setPeriod] = useState<Period>("all");
  const [query, setQuery] = useState<ReportQuery>(() => ({ kind, window: timeWindow("all") }));
  const [page, setPage] = useState<ReportPage>({ items: [], nextCursor: null });
  const [cursor, setCursor] = useState<ReportCursor | null>(null);
  const [trail, setTrail] = useState<(ReportCursor | null)[]>([]);
  const [pageNumber, setPageNumber] = useState(1);
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [error, setError] = useState<ApiError | null>(null), [loading, setLoading] = useState(true);
  const [viewing, setViewing] = useState<{ id: string; ids: string[] } | null>(null);
  const serial = useRef(0);
  const refresh = useCallback(() => { setQuery(q => ({ ...q, window: timeWindow(period) })); setCursor(null); setTrail([]); setPageNumber(1); }, [period]);
  useEffect(() => {
    const request = ++serial.current;
    setLoading(true); setError(null); setPage({ items: [], nextCursor: null }); setDashboard(null);
    const work = section === "dashboard" ? reportsApi.dashboard(timeWindow("today")) : reportsApi.query(query, cursor, 100);
    void work.then(result => { if (request !== serial.current) return; if ("totalMedia" in result) setDashboard(result); else setPage(result); setLoading(false); }, failure => { if (request === serial.current) { setError(toApiError(failure)); setLoading(false); } });
    return () => { ++serial.current; };
  }, [section, query, cursor, revision]);
  useEffect(() => { let day = new Date().toDateString(); const timer = setInterval(() => { const next = new Date().toDateString(); if (day !== next) { day = next; refresh(); } }, 30000); return () => clearInterval(timer); }, [refresh]);
  const select = (nextKind: ReportKind, nextPeriod = period) => { setKind(nextKind); setPeriod(nextPeriod); setQuery({ kind: nextKind, window: timeWindow(nextPeriod) }); setCursor(null); setTrail([]); setPageNumber(1); };
  const open = (item: ReportItem, items: ReportItem[]) => setViewing({ id: item.id, ids: items.filter(i => i.state === "active").map(i => i.id) });
  return <section className="reports-workspace" aria-label={section === "dashboard" ? "Dashboard" : section === "continue" ? "Continue Watching" : "History"}>
    <header className="library-header"><div><span className="eyebrow">YOUR COLLECTION, OVER TIME</span><h1>{section === "dashboard" ? "Dashboard" : section === "continue" ? "Continue Watching" : "History"}<span className="heading-dot">.</span></h1><p>{section === "dashboard" ? "收藏与每一次停留，都有迹可循。" : section === "continue" ? "从上次停下的地方，继续。" : "重访看过的内容，发现你最常回来的收藏。"}</p></div><button className="secondary" onClick={refresh} aria-label="刷新统计" disabled={loading}>刷新</button></header>
    {section !== "dashboard" && <div className="report-controls">{section === "history" && <select aria-label="History 分类" value={kind} onChange={e => select(e.target.value as ReportKind)}>{historyKinds.map(k => <option key={k} value={k}>{reportTitles[k]}</option>)}</select>}{section === "history" && <div role="group" aria-label="时间范围">{([ ["today", "Today"], ["week", "Last 7 Days"], ["month", "Last 30 Days"], ["all", "All Time"] ] as const).map(([value, label]) => <button key={value} aria-pressed={period === value} onClick={() => select(kind, value)}>{label}</button>)}</div>}<span className="small muted">{section === "continue" ? "当前版本 · 非零进度 · 未完成" : "按所选范围内的事件汇总 · 本地日期"}</span></div>}
    {error ? <div className="report-empty" role="alert"><h2>{error.code === "stale_cursor" ? "记录已有更新" : "暂时无法读取统计"}</h2><p>{error.message}</p><button className="primary" onClick={refresh}>重新加载</button></div> : loading ? <div className="report-empty" role="status"><span className="loading-ring" /><p>正在读取统计…</p></div> : dashboard ? <div className="dashboard-scroll"><div className="dashboard-metrics">{([ ["Total Media", dashboard.totalMedia], ["Images", dashboard.images], ["Videos", dashboard.videos], ["Total Likes", dashboard.totalLikes], ["Views Today", dashboard.viewsToday] ] as const).map(([label, value]) => <article className="metric" key={label}><span>{label}</span><strong data-metric={label}>{value.toLocaleString()}</strong></article>)}</div><p className="dashboard-note">媒体数包含暂时缺失的文件，不含已替换的旧实例；Likes 保留全部历史。Today 从本地零点起计算。</p><div className="dashboard-sections">{sections.map(k => <section className="dashboard-panel" key={k}><h2>{reportTitles[k]}</h2><ReportList items={dashboard[k]} kind={k} open={open} revision={revision} compact /></section>)}</div></div> : <><div className="report-caption"><h2>{reportTitles[kind]}</h2><span className="small muted">本页 {page.items.length} 项</span></div><ReportList items={page.items} kind={kind} open={open} revision={revision} /><footer className="report-footer"><span className="small muted">{section === "history" ? "缺失或被替换的原文件保留历史记录。" : "完成状态依据当前进度计算；保存进度不增加播放次数。"}</span><div><button className="secondary" disabled={!trail.length} onClick={() => { setCursor(trail.at(-1) ?? null); setTrail(t => t.slice(0, -1)); setPageNumber(n => n - 1); }}>← 上一页</button><span className="small">第 {pageNumber} 页</span><button className="secondary" disabled={!page.nextCursor} onClick={() => { setTrail(t => [...t, cursor].slice(-256)); setCursor(page.nextCursor); setPageNumber(n => n + 1); }}>下一页 →</button></div></footer></>}
    {viewing && <ViewerShell initialId={viewing.id} filter={initialFilter} collection={viewing.ids} revision={revision} changed={changed} close={() => { setViewing(null); changed(); refresh(); }} />}
  </section>;
}
