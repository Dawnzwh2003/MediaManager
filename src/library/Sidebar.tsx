import { useState } from "react";
import type { LibraryFilter, LibraryTag } from "../api/library-contracts.ts";
import { useTagPage } from "./hooks.ts";

function SystemBranch({ parentId, filter, choose, revision, depth = 0 }: { parentId: string | null; filter: LibraryFilter; choose: (tag: LibraryTag) => void; revision: number; depth?: number }) {
  const [after, setAfter] = useState<string | null>(null);
  const [history, setHistory] = useState<(string | null)[]>([]);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const { page, loading, error } = useTagPage({ kind: "system", parentId, afterId: after, mediaId: null, search: "", limit: 100 }, revision);
  return <ul className="tag-tree" aria-label={depth === 0 ? "System Tags" : "子目录"}>
    {loading && <li className="muted small">正在读取目录…</li>}
    {error && <li role="alert" className="small">{error.message}</li>}
    {!loading && !error && !page.items.length && depth === 0 && <li className="muted small">尚无已扫描目录</li>}
    {page.items.map(tag => <li key={tag.id}>
      <div className="tree-row">
        <button className="tree-toggle" aria-label={`${expanded.has(tag.id) ? "收起" : "展开"} ${tag.name}`} aria-expanded={expanded.has(tag.id)} disabled={!tag.hasChildren} onClick={() => setExpanded(previous => { const next = new Set(previous); if (next.has(tag.id)) next.delete(tag.id); else next.add(tag.id); return next; })}>{tag.hasChildren ? expanded.has(tag.id) ? "⌄" : "›" : "·"}</button>
        <button className={`nav-item ${filter.systemTagId === tag.id ? "active" : ""}`} onClick={() => choose(tag)} title={tag.name}><span className="folder-icon">▱</span><span className="truncate">{tag.name}</span></button>
      </div>
      {expanded.has(tag.id) && <SystemBranch parentId={tag.id} filter={filter} choose={choose} revision={revision} depth={depth + 1} />}
    </li>)}
    {(history.length > 0 || page.nextAfter) && <li className="mini-pager"><button disabled={loading || !history.length} onClick={() => { setAfter(history.at(-1) ?? null); setHistory(history.slice(0, -1)); }}>上一组</button><button disabled={loading || !page.nextAfter} onClick={() => { setHistory([...history, after].slice(-256)); setAfter(page.nextAfter); }}>下一组</button></li>}
  </ul>;
}
export function Sidebar({ filter, onFilter, revision, onManage }: { filter: LibraryFilter; onFilter: (filter: LibraryFilter) => void; revision: number; onManage: () => void }) {
  const [after, setAfter] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const { page, loading, error } = useTagPage({ kind: "custom", parentId: null, afterId: after, mediaId: null, search, limit: 100 }, revision);
  return <aside className="sidebar" aria-label="媒体库导航">
    <div className="brand"><span className="brand-mark">▦</span><div>Media Library<small>本地媒体，井然有序</small></div></div>
    <div className="nav-section"><span className="section-label">LIBRARY</span><button className={`nav-item all-media ${!filter.systemTagId && !filter.customTagId ? "active" : ""}`} onClick={() => onFilter({ ...filter, systemTagId: null, customTagId: null })}><span>▦</span> All Media</button></div>
    <div className="nav-section"><h2 className="section-label">SYSTEM TAGS <span className="badge">只读</span></h2><SystemBranch parentId={null} filter={filter} choose={tag => onFilter({ ...filter, systemTagId: tag.id, customTagId: null })} revision={revision} /><p className="nav-note">父目录默认包含所有子目录。</p></div>
    <div className="nav-section custom-navigation"><div className="section-heading"><h2 className="section-label">CUSTOM TAGS</h2><button className="icon-button" onClick={onManage} aria-label="管理 Custom Tags">＋</button></div>
      <input className="tag-search" aria-label="搜索导航标签" placeholder="查找标签…" maxLength={128} value={search} onChange={e => { setSearch(e.target.value); setAfter(null); }} />
      {loading && <p className="muted small">正在读取标签…</p>}{error && <p role="alert" className="small">{error.message}</p>}
      {!loading && !error && !page.items.length && <p className="muted small">还没有标签，点击 ＋ 创建。</p>}
      {page.items.map(tag => <button key={tag.id} className={`nav-item ${filter.customTagId === tag.id ? "active" : ""}`} onClick={() => onFilter({ ...filter, customTagId: tag.id, systemTagId: null })}><span className="tag-dot" /><span className="truncate">{tag.name}</span></button>)}
      {after && <button className="text-button" onClick={() => setAfter(null)}>返回标签首页</button>}{page.nextAfter && <button className="text-button" onClick={() => setAfter(page.nextAfter)}>下一组标签 →</button>}
    </div>
    <div className="sidebar-footer"><span className="status-dot" /> 本地数据库 · 原件只读</div>
  </aside>;
}
