import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { libraryApi } from "../api/library.ts";
import type { LibraryFilter, TagMutation } from "../api/library-contracts.ts";
import { Sidebar } from "../library/Sidebar.tsx";
import { FilterBar } from "../library/FilterBar.tsx";
import { MediaGrid } from "../library/MediaGrid.tsx";
import { DetailsPanel } from "../library/DetailsPanel.tsx";
import { TagSelector } from "../library/TagSelector.tsx";
import { customFilterIds, hasLibraryFilters, LibraryController, MAX_SELECTION, toggleSelection } from "../library/model.ts";
import { ViewerShell } from "../viewer/ViewerShell.tsx";
import { StatisticsControl } from "../library/StatisticsControl.tsx";
import { QuickBrowseShell } from "../quick-browse/QuickBrowseShell.tsx";

export function App() {
  const controller = useMemo(() => new LibraryController(libraryApi.query), []);
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [focused, setFocused] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [modal, setModal] = useState<"manage" | "batch" | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [quick, setQuick] = useState(false);
  useEffect(() => { void controller.refresh(); return controller.cancel; }, [controller]);
  const change = useCallback((filter: LibraryFilter) => { setSelected(new Set()); setFocused(null); void controller.change(filter); }, [controller]);
  const refresh = useCallback(() => { setRevision(r => r + 1); void controller.refresh(); }, [controller]);
  const changed = useCallback((mutation?: TagMutation) => {
    setRevision(r => r + 1);
    const filter = controller.getSnapshot().filter;
    if (mutation?.action === "delete" && customFilterIds(filter).includes(mutation.id)) {
      void controller.change({ ...filter, customTagId: null, customTagIds: customFilterIds(filter).filter(id => id !== mutation.id) });
    } else void controller.refresh();
  }, [controller]);
  useEffect(() => { const close = (event: KeyboardEvent) => { if (event.key === "Escape") setModal(null); }; window.addEventListener("keydown", close); return () => window.removeEventListener("keydown", close); }, []);
  return <main className="library-shell"><Sidebar filter={state.filter} onFilter={change} revision={revision} onManage={() => setModal("manage")} />
    <section className="library-main" aria-label="Media Library"><header className="library-header"><div><span className="eyebrow">YOUR COLLECTION, IN FOCUS</span><h1>媒体库<span className="heading-dot">.</span></h1><p>把目光留给内容，把整理交给标签。</p></div><span className="local-badge"><span className="status-dot" /> LOCAL LIBRARY</span></header>
      <button className="secondary" disabled={state.loading || !!state.error} onClick={() => setQuick(true)}>Quick Browse · 键盘浏览</button>
      <FilterBar filter={state.filter} change={change} refresh={refresh} shuffle={() => {
        setSelected(new Set());
        setFocused(null);
        void controller.shuffle();
      }} loading={state.loading} />
      <div className="collection-toolbar"><div><span className="collection-title">{state.filter.systemTagId ? "目录与所有子目录" : customFilterIds(state.filter).length ? "标签中的媒体" : "All Media"}</span><span className="muted small">{state.loading ? "正在查询…" : `本页 ${state.items.length} 项`}</span></div><div className="selection-actions">{selected.size > 0 ? <><span className="small">已选 {selected.size}/{MAX_SELECTION}</span><button className="text-button" onClick={() => setModal("batch")}>批量编辑标签</button><button className="text-button" onClick={() => setSelected(new Set())}>清空</button></> : <button className="text-button" disabled={state.loading || !state.items.length} onClick={() => setSelected(new Set(state.items.map(m => m.id)))}>选择本页</button>}</div></div>
      {state.error ? <div className="library-empty" role="alert"><span className="empty-symbol">▧</span><h2>{state.error.code === "desktop_required" ? "在桌面端打开你的媒体库" : state.error.code === "stale_cursor" ? "媒体库已有更新" : "暂时无法读取媒体库"}</h2><p>{state.error.message}</p><button className="primary" onClick={refresh}>重新加载</button></div> : state.loading ? <div className="library-empty" role="status"><span className="loading-ring" /><p>正在读取本地媒体…</p></div> : state.items.length ? <MediaGrid items={state.items} selected={selected} focused={focused} toggle={id => setSelected(s => toggleSelection(s, id))} open={setFocused} view={setViewing} revision={revision} /> : <div className="library-empty"><span className="empty-symbol">▦</span><h2>{hasLibraryFilters(state.filter) ? "没有匹配的媒体" : "从一份收藏开始"}</h2><p>{hasLibraryFilters(state.filter) ? "试试其他关键词、类型或标签。" : "扫描入库的图片与视频会显示在这里。"}</p></div>}
      <footer className="library-footer"><span>仅显示缩略图 · 原件只读</span><StatisticsControl finished={refresh} /><div><button className="secondary" disabled={state.loading || !state.previous || !!state.error} onClick={() => { void controller.previous(); }}>← 上一页</button><span className="small">第 {state.page} 页</span><button className="secondary" disabled={state.loading || !state.next || !!state.error} onClick={() => { void controller.next(); }}>下一页 →</button></div></footer>
    </section><DetailsPanel id={focused} close={() => setFocused(null)} revision={revision} changed={changed} />
    {modal && <div className="modal-backdrop" onClick={e => { if (e.target === e.currentTarget) setModal(null); }}><section className="tag-modal" role="dialog" aria-modal="true" aria-labelledby="tag-modal-title"><div className="panel-heading"><div><span className="eyebrow">CUSTOM TAGS</span><h2 id="tag-modal-title">{modal === "manage" ? "管理你的标签" : `为 ${selected.size} 项批量编辑标签`}</h2></div><button className="icon-button" onClick={() => setModal(null)} aria-label="关闭标签窗口" autoFocus>×</button></div><TagSelector key={modal} manage={modal === "manage"} mediaIds={modal === "batch" ? [...selected] : []} revision={revision} changed={changed} /></section></div>}
    {viewing && <ViewerShell initialId={viewing} filter={state.filter} close={() => {setViewing(null);refresh();}} revision={revision} changed={changed} />}
    {quick && <QuickBrowseShell filter={state.filter} revision={revision} changed={changed} close={() => { setQuick(false); refresh(); }} />}
  </main>;
}
