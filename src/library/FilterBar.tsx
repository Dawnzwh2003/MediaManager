import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import type { InclusiveRange, LibraryFilter, LibrarySort, SearchScope } from "../api/library-contracts.ts";
import { customFilterIds } from "./model.ts";
import "./query-filters.css";

const sorts: [LibrarySort, string][] = [
  ["recent", "Imported ↓"], ["importedAsc", "Imported ↑"],
  ["name", "Filename A–Z"], ["nameDesc", "Filename Z–A"],
  ["likes", "Like ↓"], ["likesAsc", "Like ↑"], ["views", "View ↓"], ["viewsAsc", "View ↑"],
  ["plays", "Play ↓"], ["playsAsc", "Play ↑"], ["created", "Created ↓"], ["createdAsc", "Created ↑"],
  ["recentlyViewed", "Recently Viewed"], ["recentlyPlayed", "Recently Played"], ["random", "Random"],
];

function localDay(value: number | null): string {
  if (value === null) return "";
  const date = new Date(value);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${String(date.getFullYear()).padStart(4, "0")}-${month}-${day}`;
}

function readRange(form: FormData, field: string, dates = false): InclusiveRange | null {
  const bounds = ["min", "max"].map(bound => {
    const value = String(form.get(`${field}-${bound}`) ?? "");
    if (!value) return null;
    if (dates) {
      const date = new Date(`${value}T00:00:00`);
      if (bound === "max") date.setDate(date.getDate() + 1);
      const timestamp = date.getTime() - (bound === "max" ? 1 : 0);
      if (Number.isSafeInteger(timestamp)) return timestamp;
    } else if (Number.isSafeInteger(Number(value)) && Number(value) >= 0) {
      return Number(value);
    }
    throw new Error("请输入有效的日期或非负整数。");
  });
  const min = bounds[0] ?? null;
  const max = bounds[1] ?? null;
  if (min !== null && max !== null && min > max) throw new Error("范围下限必须小于或等于上限。");
  return min === null && max === null ? null : { min, max };
}

function RangeFields({ field, label, range, dates = false }: { field: string; label: string; range: InclusiveRange | null; dates?: boolean }) {
  return <fieldset className={`query-range${dates ? " query-range-dates" : ""}`}>
    <legend>{label}</legend>
    {(["min", "max"] as const).map(bound => {
      const value = range?.[bound] ?? null;
      const name = `${label} ${bound === "min" ? "起点" : "终点"}`;
      const input = <input key={bound} name={`${field}-${bound}`} type={dates ? "date" : "number"}
        min={dates ? undefined : 0} step={dates ? undefined : 1} aria-label={name}
        placeholder={bound === "min" ? "最小值" : "最大值"} defaultValue={dates ? localDay(value) : value ?? ""} />;
      return dates ? <label key={bound} className="query-date-bound">{bound === "min" ? "起始日" : "结束日"}{input}</label> : input;
    })}
  </fieldset>;
}

export function FilterBar({ filter, change, refresh, shuffle, loading }: {
  filter: LibraryFilter;
  change: (filter: LibraryFilter) => void;
  refresh: () => void;
  shuffle: () => void;
  loading: boolean;
}) {
  const [search, setSearch] = useState(filter.search);
  const [expanded, setExpanded] = useState(false);
  const [error, setError] = useState("");
  const tags = customFilterIds(filter);
  const ranges = [filter.likes, filter.views, filter.plays, filter.imported].filter(Boolean).length;
  const active = ranges + Number(filter.viewed !== "all");

  useEffect(() => { setSearch(filter.search); }, [filter.search]);
  useEffect(() => {
    if (search === filter.search) return;
    const timer = setTimeout(() => change({ ...filter, search }), 300);
    return () => clearTimeout(timer);
  }, [search, filter, change]);

  function apply(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      const viewed = form.get("viewed");
      change({ ...filter, search, likes: readRange(form, "likes"), views: readRange(form, "views"),
        plays: readRange(form, "plays"), imported: readRange(form, "imported", true),
        viewed: viewed === "viewed" || viewed === "unviewed" ? viewed : "all" });
      setExpanded(false);
      setError("");
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "无法应用筛选。");
    }
  }

  return <div>
    <div className="filter-bar query-toolbar">
      <label className="search-field"><span aria-hidden="true">⌕</span><input aria-label="搜索媒体" placeholder="文件名或标签…"
        maxLength={200} value={search} onChange={event => setSearch(event.target.value)} /></label>
      <label className="select-field"><select aria-label="搜索范围" value={filter.searchScope}
        onChange={event => change({ ...filter, search, searchScope: event.target.value as SearchScope })}>
        <option value="all">全部字段</option><option value="filename">文件名</option>
        <option value="systemTag">System Tag</option><option value="customTag">Custom Tag</option>
      </select></label>
      <label className="select-field"><select aria-label="媒体类型" value={filter.mediaType ?? "all"}
        onChange={event => change({ ...filter, mediaType: event.target.value === "all" ? null : event.target.value as "image" | "video" })}>
        <option value="all">全部类型</option><option value="image">图片</option><option value="video">视频</option>
      </select></label>
      <label className="select-field"><select aria-label="排序" value={filter.sort}
        onChange={event => change({ ...filter, sort: event.target.value as LibrarySort })}>
        {sorts.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </select></label>
      {tags.length > 1 && <label className="select-field"><select aria-label="Custom Tag 匹配" value={filter.customTagMode}
        onChange={event => change({ ...filter, customTagMode: event.target.value === "or" ? "or" : "and" })}>
        <option value="and">AND 全部标签</option><option value="or">OR 任一标签</option>
      </select></label>}
      <button className="secondary" aria-expanded={expanded} aria-controls="library-query-filters"
        onClick={() => { setExpanded(!expanded); setError(""); }}>筛选{active ? ` (${active})` : ""}</button>
      {filter.sort === "random" && <button className="secondary" onClick={shuffle} disabled={loading}>Shuffle Again</button>}
      <button className="secondary" onClick={refresh} disabled={loading} aria-label="刷新媒体库">↻ 刷新</button>
    </div>
    {expanded && <form id="library-query-filters" className="query-filters" onSubmit={apply}>
      <div className="query-filter-fields">
        <RangeFields field="likes" label="Like Count" range={filter.likes} />
        <RangeFields field="views" label="View Count" range={filter.views} />
        <RangeFields field="plays" label="Play Count" range={filter.plays} />
        <RangeFields field="imported" label="导入日期" range={filter.imported} dates />
        <label className="query-viewed">查看状态<select name="viewed" defaultValue={filter.viewed}>
          <option value="all">全部</option><option value="viewed">已查看</option><option value="unviewed">未查看</option>
        </select></label>
      </div>
      {error && <p className="inline-error" role="alert">{error}</p>}
      <div className="query-filter-actions">
        <button className="text-button" type="button" onClick={() => {
          change({ ...filter, search, likes: null, views: null, plays: null, imported: null, viewed: "all" });
          setExpanded(false);
        }}>清除范围和状态</button>
        <button className="primary" type="submit">应用筛选</button>
      </div>
    </form>}
  </div>;
}
