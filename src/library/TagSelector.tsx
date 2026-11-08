import { useState } from "react";
import { libraryApi } from "../api/library.ts";
import type { TagMutation } from "../api/library-contracts.ts";
import { toApiError } from "../api/contracts.ts";
import { useTagPage } from "./hooks.ts";

export function TagSelector({ mediaIds = [], revision, changed, manage = false }: { mediaIds?: string[]; revision: number; changed: (mutation?: TagMutation) => void; manage?: boolean }) {
  const [search, setSearch] = useState("");
  const [after, setAfter] = useState<string | null>(null);
  const [history, setHistory] = useState<(string | null)[]>([]);
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [editedName, setEditedName] = useState("");
  const [deleting, setDeleting] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const { page, loading, error: readError } = useTagPage({ kind: "custom", parentId: null, afterId: after, mediaId: mediaIds.length === 1 ? mediaIds[0]! : null, search, limit: 50 }, revision);
  async function mutate(mutation: TagMutation) {
    setBusy(true); setError(""); setMessage("");
    try {
      await libraryApi.mutate(mutation);
      setEditing(null); setDeleting(null); setName("");
      setMessage(mutation.action === "attach" ? `已为 ${mediaIds.length} 项添加标签。` : mutation.action === "detach" ? `已从 ${mediaIds.length} 项移除标签。` : "标签已更新。");
      changed(mutation);
    } catch (e) { setError(toApiError(e).message); } finally { setBusy(false); }
  }
  return <section className="tag-selector" aria-label={manage ? "Custom Tag 管理" : "编辑媒体标签"}>
    <label className="field-label">查找 Custom Tag<input value={search} maxLength={128} placeholder="搜索标签…" onChange={e => { setSearch(e.target.value); setAfter(null); setHistory([]); }} /></label>
    {manage && <form className="create-tag" onSubmit={e => { e.preventDefault(); void mutate({ action: "create", name: name.trim() }); }}><input aria-label="新标签名称" maxLength={128} placeholder="新标签名称" value={name} onChange={e => setName(e.target.value)} /><button className="primary" disabled={busy || !name.trim()}>创建</button></form>}
    {(error || readError) && <p role="alert" className="inline-error">{error || readError?.message}<button className="text-button" onClick={() => { setError(""); changed(); }}>重新读取</button></p>}
    {message && <p role="status" className="success small">{message}</p>}{loading && <p className="muted small">正在读取标签…</p>}
    {!loading && !page.items.length && <p className="empty-tags">没有匹配的标签。{!manage && "可在侧栏 ＋ 中创建。"}</p>}
    <div className="tag-options">{page.items.map(tag => <div className="tag-option" key={tag.id}>
      {editing === tag.id ? <form className="rename-form" onSubmit={e => { e.preventDefault(); void mutate({ action: "rename", id: tag.id, name: editedName.trim() }); }}><input aria-label={`重命名 ${tag.name}`} maxLength={128} value={editedName} onChange={e => setEditedName(e.target.value)} autoFocus /><button disabled={busy || !editedName.trim()}>保存</button><button type="button" className="text-button" onClick={() => setEditing(null)}>取消</button></form> : <><span className="tag-chip truncate" title={tag.name}>{tag.name}</span>
        {manage ? <div className="tag-actions">{deleting === tag.id ? <><span className="small">移除所有关联？</span><button className="danger" disabled={busy} onClick={() => { void mutate({ action: "delete", id: tag.id }); }}>确认删除</button><button className="text-button" onClick={() => setDeleting(null)}>取消</button></> : <><button className="text-button" disabled={busy} onClick={() => { setEditing(tag.id); setEditedName(tag.name); }}>重命名</button><button className="text-button danger-text" disabled={busy} onClick={() => setDeleting(tag.id)}>删除</button></>}</div> : mediaIds.length === 1 ? <button className={tag.attached ? "text-button danger-text" : "text-button"} disabled={busy} onClick={() => { void mutate({ action: tag.attached ? "detach" : "attach", mediaIds, tagIds: [tag.id] }); }}>{tag.attached ? "✓ 已添加 · 移除" : "＋ 添加"}</button> : <div className="tag-actions"><button className="text-button" disabled={busy || !mediaIds.length} onClick={() => { void mutate({ action: "attach", mediaIds, tagIds: [tag.id] }); }}>全部添加</button><button className="text-button danger-text" disabled={busy || !mediaIds.length} onClick={() => { void mutate({ action: "detach", mediaIds, tagIds: [tag.id] }); }}>全部移除</button></div>}
      </>}
    </div>)}</div>
    {(history.length > 0 || page.nextAfter) && <div className="mini-pager"><button disabled={loading || busy || !history.length} onClick={() => { setAfter(history.at(-1) ?? null); setHistory(history.slice(0, -1)); }}>上一组</button><button disabled={loading || busy || !page.nextAfter} onClick={() => { setHistory([...history, after].slice(-256)); setAfter(page.nextAfter); }}>下一组</button></div>}
    <p className="muted small">标签只改变数据库关联，保留原文件及物理目录。</p>
  </section>;
}
