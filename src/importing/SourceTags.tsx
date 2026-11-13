import { useEffect, useState } from "react";
import { importApi } from "../api/importing.ts";
import { toApiError } from "../api/contracts.ts";
import type { SourceNode, SourcePage } from "./model.ts";
function Branch({ source, parent, selected, choose, revision }: { source: string | null; parent: string | null; selected: string | null; choose: (id: string) => void; revision: number }) {
  const [page, setPage] = useState<SourcePage>({ items: [], nextAfter: null });
  const [after, setAfter] = useState(""); const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [error, setError] = useState("");
  useEffect(() => { let active = true; importApi.sources(source, parent, after).then(p => { if (active) { setPage(p); setError(""); } }, e => { if (active) setError(toApiError(e).message); }); return () => { active = false; }; }, [source, parent, after, revision]);
  const expand = (node: SourceNode) => setExpanded(previous => { const next = new Set(previous); if (next.has(node.id)) next.delete(node.id); else next.add(node.id); return next; });
  return <ul className="tag-tree">{error && <li role="alert" className="small">{error}</li>}{page.items.map(node => <li key={node.id}><div className="tree-row"><button className="tree-toggle" aria-label={`展开来源 ${node.name}`} disabled={!node.hasChildren} aria-expanded={expanded.has(node.id)} onClick={() => expand(node)}>{node.hasChildren ? expanded.has(node.id) ? "⌄" : "›" : "·"}</button><button className={`nav-item ${selected === node.id ? "active" : ""}`} title={node.relativePath} onClick={() => node.root ? expand(node) : choose(node.id)}>{node.name}</button></div>{expanded.has(node.id) && <Branch source={node.sourceId} parent={node.root ? null : node.id} selected={selected} choose={choose} revision={revision} />}</li>)}{(after || page.nextAfter) && <li className="mini-pager">{after && <button onClick={() => setAfter("")}>首页</button>}{page.nextAfter && <button onClick={() => setAfter(page.nextAfter!)}>下一组来源</button>}</li>}</ul>;
}
export function SourceTags({ selected, choose, revision }: { selected: string | null; choose: (id: string | null) => void; revision: number }) {
  return <div className="nav-section"><h2 className="section-label">SOURCE TAGS <span className="badge">只读</span></h2>{selected && <button className="text-button" onClick={() => choose(null)}>清除来源筛选</button>}<Branch source={null} parent={null} selected={selected} choose={choose} revision={revision} /><p className="nav-note">导入来源目录；不改变物理目录或 Custom Tags。</p></div>;
}
