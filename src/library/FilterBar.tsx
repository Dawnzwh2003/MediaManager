import { useEffect, useState } from "react";
import type { LibraryFilter } from "../api/library-contracts.ts";
export function FilterBar({ filter, change, refresh, loading }: { filter: LibraryFilter; change: (filter: LibraryFilter) => void; refresh: () => void; loading: boolean }) {
  const [search, setSearch] = useState(filter.search);
  useEffect(() => { setSearch(filter.search); }, [filter.search]);
  useEffect(() => { if (search === filter.search) return; const timer = setTimeout(() => change({ ...filter, search }), 300); return () => clearTimeout(timer); }, [search, filter, change]);
  return <div className="filter-bar"><label className="search-field"><span>⌕</span><input aria-label="搜索文件名" placeholder="搜索文件名…" maxLength={200} value={search} onChange={e => setSearch(e.target.value)} /></label>
    <label className="select-field"><span className="sr-only">媒体类型</span><select aria-label="媒体类型" value={filter.mediaType ?? "all"} onChange={e => change({ ...filter, mediaType: e.target.value === "all" ? null : e.target.value as "image" | "video" })}><option value="all">全部类型</option><option value="image">图片</option><option value="video">视频</option></select></label>
    <label className="select-field"><span className="sr-only">排序</span><select aria-label="排序" value={filter.sort} onChange={e => change({ ...filter, sort: e.target.value as LibraryFilter["sort"] })}><option value="recent">最近导入</option><option value="name">文件名 A–Z</option><option value="likes">Like Count 高–低</option></select></label>
    <button className="secondary" onClick={refresh} disabled={loading} aria-label="刷新媒体库">↻ 刷新</button>
  </div>;
}
