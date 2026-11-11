import { useEffect, useRef, useState } from "react";
import type { BrowsePreferences, BrowseSettings } from "../api/quick-browse.ts";
import { useTagPage } from "../library/hooks.ts";
import { toApiError } from "../api/contracts.ts";

export function QuickBrowseSettings({ settings, revision, save, close }: { settings: BrowseSettings; revision: number; save: (value: BrowsePreferences) => Promise<void>; close: () => void }) {
  const [value, setValue] = useState(settings);
  const [digit, setDigit] = useState(1), [search, setSearch] = useState("");
  const [pages, setPages] = useState<(string | null)[]>([null]);
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const panel = useRef<HTMLElement>(null);
  const tags = useTagPage({ kind: "custom", parentId: null, mediaId: null, afterId: pages.at(-1) ?? null, search, limit: 100 }, revision);
  useEffect(() => {
    const previous = document.activeElement; panel.current?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); if (!busy) close(); }
      if (event.key === "Tab") {
        const nodes = [...panel.current!.querySelectorAll<HTMLElement>("button:not(:disabled),input,select")];
        if (event.shiftKey && (document.activeElement === nodes[0] || document.activeElement === panel.current)) { event.preventDefault(); nodes.at(-1)?.focus(); }
        else if (!event.shiftKey && document.activeElement === nodes.at(-1)) { event.preventDefault(); nodes[0]?.focus(); }
      }
    };
    window.addEventListener("keydown", key, true);
    return () => { window.removeEventListener("keydown", key, true); if (previous instanceof HTMLElement) previous.focus(); };
  }, [close, busy]);
  return <div className="quick-modal"><section className="quick-settings" ref={panel} tabIndex={-1} role="dialog" aria-modal="true" aria-label="快速浏览设置">
    <div className="panel-heading"><h2>快速浏览设置</h2><button disabled={busy} onClick={close}>关闭</button></div>
    <label><input type="checkbox" checked={value.autoplay} onChange={e => setValue({ ...value, autoplay: e.target.checked })} /> 自动播放视频</label>
    <label><input type="checkbox" checked={value.muted} onChange={e => setValue({ ...value, muted: e.target.checked })} /> 视频默认静音</label>
    <p>数字键切换标签：没有则添加，已有则移除。</p>
    <div className="quick-slots">{Array.from({ length: 9 }, (_, index) => index + 1).map(slot => <button key={slot} aria-pressed={digit === slot} onClick={() => setDigit(slot)}><kbd>{slot}</kbd> {value.bindings.find(binding => binding.digit === slot)?.name ?? "未绑定"}</button>)}</div>
    <label>为数字 {digit} 选择标签<input aria-label="搜索快捷标签" placeholder="搜索 Custom Tag" value={search} onChange={e => { setSearch(e.target.value); setPages([null]); }} /></label>
    <button onClick={() => setValue({ ...value, bindings: value.bindings.filter(binding => binding.digit !== digit) })}>清除数字 {digit} 的绑定</button>
    {tags.error && <p role="alert">{tags.error.message}</p>}
    <div className="quick-tag-options">{tags.loading ? <p>正在查询标签…</p> : tags.page.items.map(tag => <button key={tag.id} onClick={() => setValue({ ...value, bindings: [...value.bindings.filter(binding => binding.digit !== digit), { digit, tagId: tag.id, name: tag.name }] })}>{tag.name}</button>)}</div>
    <div className="quick-settings-actions"><button disabled={pages.length === 1 || tags.loading} onClick={() => setPages(p => p.slice(0, -1))}>上一页</button><button disabled={!tags.page.nextAfter || tags.loading} onClick={() => setPages(p => [...p, tags.page.nextAfter])}>下一页</button><button className="primary" disabled={busy} onClick={() => { setBusy(true); void save(value).then(close, e => { setError(toApiError(e).message); setBusy(false); }); }}>保存设置</button></div>
    {error && <p role="alert">{error}</p>}
  </section></div>;
}
