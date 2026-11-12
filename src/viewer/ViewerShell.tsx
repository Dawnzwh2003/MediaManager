import { useCallback, useEffect, useRef, useState } from "react";
import type { LibraryFilter, TagMutation } from "../api/library-contracts.ts";
import type { ViewerNavigation, ViewerOpen } from "../api/viewer.ts";
import { openViewer, viewerApi } from "../api/viewer.ts";
import { libraryApi } from "../api/library.ts";
import { toApiError } from "../api/contracts.ts";
import { DetailsPanel } from "../library/DetailsPanel.tsx";
import { TagSelector } from "../library/TagSelector.tsx";
import { ImageViewer } from "./ImageViewer.tsx";
import { VideoPlayer } from "./VideoPlayer.tsx";
import type { PlayerControl } from "./VideoPlayer.tsx";
import "./viewer.css";
import {activityApi} from "../api/activity.ts";
import {ActivationController} from "./activity-model.ts";
import {LikeButton} from "../library/LikeButton.tsx";
import {isLikeShortcut} from "../library/like-shortcut.ts";
import { quickBrowseApi } from "../api/quick-browse.ts";
import { quickShortcut, TagShortcutQueue } from "../quick-browse/model.ts";
import type { QuickViewerContext } from "../quick-browse/model.ts";

export function ViewerShell({ initialId, filter, close, revision, changed, quick, collection }: { initialId: string; filter: LibraryFilter; close: (force?: boolean) => void | Promise<void>; revision: number; changed: (mutation?: TagMutation) => void; quick?: QuickViewerContext; collection?: string[] }) {
  const collectionAtOpen = useRef(collection);
  const quickRef = useRef(quick); quickRef.current = quick;
  const [feedback, setFeedback] = useState("");
  const [tagError, setTagError] = useState("");
  const [tagQueue] = useState(() => quick ? new TagShortcutQueue(quick.session.id, quick.item.ordinal, quickBrowseApi.toggle, result => { setFeedback(`${result.attached ? "已添加" : "已移除"} · ${result.tagName}`); changed(); }) : null);
  useEffect(() => { if (!feedback) return; const timer = setTimeout(() => setFeedback(""), 1800); return () => clearTimeout(timer); }, [feedback]);
  const element = useRef<HTMLDivElement>(null);
  const activations = useRef(new ActivationController(crypto.randomUUID()));
  const activeId = useRef<string|null>(null);
  const ownOrderingChange = useRef(false);
  const navigationAnchor = useRef<string | null>(null);
  const [activationId,setActivationId] = useState<string|null>(null);
  const [likes,setLikes] = useState(0);
  const filterAtOpen = useRef({ ...filter });
  const [id, setId] = useState(initialId), [name, setName] = useState("");
  const [session, setSession] = useState<ViewerOpen | null>(null);
  const sessionRef = useRef<ViewerOpen | null>(null), control = useRef<PlayerControl | null>(null);
  const [navigation, setNavigation] = useState<ViewerNavigation | null>(null);
  const [pane, setPane] = useState<"details" | "tags" | null>(null);
  const [loading, setLoading] = useState(true), [busy, setBusy] = useState(false), [retry, setRetry] = useState(0);
  const [error, setError] = useState("");
  const register = useCallback((value: PlayerControl | null) => { control.current = value; }, []);
  useEffect(() => { const previous = document.activeElement; element.current?.focus(); return () => { if (previous instanceof HTMLElement) previous.focus(); }; }, []);
  const flush = useCallback(async () => { await tagQueue?.flush(); await control.current?.stopAndFlush(); if(activeId.current) await activityApi.close(activeId.current); }, [tagQueue]);
  useEffect(() => { quickRef.current?.registerFlush(flush); return () => quickRef.current?.registerFlush(null); }, [flush]);
  const refreshNavigation = useCallback(async () => {
    if (quickRef.current) return;
    if (collectionAtOpen.current) {
      const index = collectionAtOpen.current.indexOf(id);
      setNavigation({ previousId: collectionAtOpen.current[index - 1] ?? null, nextId: collectionAtOpen.current[index + 1] ?? null, generation: 0, key: "" });
      return;
    }
    const token = sessionRef.current?.token;
    if (!token) return;
    try {
      const fresh = await viewerApi.navigation(filterAtOpen.current, id, null, navigationAnchor.current);
      if (sessionRef.current?.token === token) setNavigation(fresh);
    } catch (failure) {
      if (sessionRef.current?.token === token) setError(toApiError(failure).message);
    }
  }, [id]);
  useEffect(() => {
    let active = true; let opened: ViewerOpen | null = null;
    navigationAnchor.current = null;
    ownOrderingChange.current = false;
    setLoading(true); setActivationId(null); setSession(null); sessionRef.current = null; setNavigation(null); setError("");
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const details = await libraryApi.details(id);
          if (!active) return;
          setName(details.media.fileName); setLikes(details.media.likeCount); const context = quickRef.current; opened = await openViewer(id, context ? () => quickBrowseApi.open(context.session.id, context.item.ordinal) : undefined);
          if (!active) { await viewerApi.close(opened.token); return; }
          sessionRef.current = opened; setSession(opened); setLoading(false);
          // The current item's View can change its rank or remove it from Unviewed.
          const entry = context || collectionAtOpen.current ? null : await viewerApi.navigation(filterAtOpen.current, id, null);
          if (!active) return;
          navigationAnchor.current = entry?.key ?? null;
          await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
          if (!active) return;
          const activation = context?.activation ?? activations.current.activate(id,opened.contentVersionId,collectionAtOpen.current ? "history" : filterAtOpen.current.search ? "search" : id === initialId ? "grid" : "normal_viewer");
          activeId.current = activation.id;
          await activityApi.activate(opened.token,activation);
          if(opened.mediaType === "video") await activityApi.openPlayer(activation.id);
          if(active) setActivationId(activation.id); else { await activityApi.close(activation.id); return; }
          await refreshNavigation();
        } catch (e) { if (active) { setError(toApiError(e).message); setLoading(false); } }
      })();
    }, 0);
    return () => { active = false; clearTimeout(timer); if (opened) { const token = opened.token; void viewerApi.close(token).catch(() => {}); } };
  }, [id, retry, refreshNavigation]);
  const refreshAfterActivity = useCallback(() => {
    ownOrderingChange.current = true;
    setNavigation(null);
    void refreshNavigation();
  }, [refreshNavigation]);
  useEffect(() => {
    window.addEventListener("like-recorded", refreshAfterActivity);
    return () => window.removeEventListener("like-recorded", refreshAfterActivity);
  }, [refreshAfterActivity]);
  const lastRevision = useRef(revision);
  useEffect(() => { if (lastRevision.current !== revision) { lastRevision.current = revision; void refreshNavigation(); } }, [revision, refreshNavigation]);
  useEffect(() => {
    if (quickRef.current) return;
    let stopped = false; let unlisten: (() => void) | undefined;
    void viewerApi.onApplicationClose(flush).then(cleanup => { if (stopped) cleanup(); else unlisten = cleanup; }).catch(() => { setError("无法注册退出前保存；周期性进度保存仍会继续。"); });
    return () => { stopped = true; unlisten?.(); };
  }, [flush]);
  const leave = useCallback(async (force = false) => {
    setBusy(true);
    try { if (!force) await flush(); if (sessionRef.current) await viewerApi.close(sessionRef.current.token); if (!quickRef.current && document.fullscreenElement) await document.exitFullscreen(); await close(force); }
    catch (e) { setError(`关闭前保存未完成：${toApiError(e).message}`); setBusy(false); }
  }, [close, flush]);
  const navigate = useCallback(async (direction: "previousId" | "nextId") => {
    if (quickRef.current) { if (busy || loading) return; setBusy(true); try { await quickRef.current.navigate(direction === "previousId" ? -1 : 1, flush); } catch (e) { setError(toApiError(e).message); } finally { setBusy(false); } return; }
    if (!navigation || busy || loading) return;
    if (collectionAtOpen.current) {
      const target = navigation[direction];
      if (target) { setBusy(true); try { await flush(); setId(target); } catch (e) { setError(toApiError(e).message); } finally { setBusy(false); } }
      return;
    }
    setBusy(true); setError("");
    try { const fresh = await viewerApi.navigation(filterAtOpen.current, id, ownOrderingChange.current ? null : navigation.generation, navigationAnchor.current); ownOrderingChange.current=false; const target = fresh[direction]; if (target) { await flush(); setId(target); } }
    catch (e) { setError(toApiError(e).message); } finally { setBusy(false); }
  }, [navigation, busy, loading, id, flush]);
  const fullscreen = useCallback(() => { if (quickRef.current) { quickRef.current.fullscreen(); return; } const work = document.fullscreenElement ? document.exitFullscreen() : element.current?.requestFullscreen(); void work?.catch(() => setError("无法进入全屏，请重试。")); }, []);
  const shuffle = useCallback(async () => { if (!quickRef.current || busy) return; setBusy(true); try { await quickRef.current.shuffle(flush); } catch (e) { setError(toApiError(e).message); } finally { setBusy(false); } }, [busy, flush]);
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (quickRef.current) {
        if (quickRef.current.settingsOpen) return;
        if (event.key === "Escape") { event.preventDefault(); void leave(); return; }
        if ((event.target instanceof Element ? event.target : null)?.closest("input,select,textarea,[contenteditable=true]")) return;
        const action = quickShortcut(event);
        if (action !== null) { event.preventDefault();
          if (action === "previous" || action === "next") void navigate(action === "previous" ? "previousId" : "nextId");
          else if (action === "like") element.current?.querySelector<HTMLButtonElement>("[data-like]")?.click();
          else if (action === "tags") setPane(p => p === "tags" ? null : "tags");
          else if (action === "info") setPane(p => p === "details" ? null : "details");
          else if (action === "fullscreen") fullscreen();
          else if (action === "shuffle") void shuffle();
          else if (typeof action === "number" && !busy && !loading) {
            if (!quickRef.current.settings.bindings.some(binding => binding.digit === action)) setFeedback(`${action} 尚未绑定标签 · 点击快捷键设置`);
            else void tagQueue?.toggle(action).then(() => setTagError(""), e => setTagError(toApiError(e).message));
          }
          return;
        }
      }
      if (event.key === "Tab") { const nodes = [...(element.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]') ?? [])]; const first = nodes[0], last = nodes.at(-1); if (event.shiftKey && (document.activeElement === first || document.activeElement === element.current)) { event.preventDefault(); last?.focus(); } else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); } return; }
      if ((event.target instanceof Element ? event.target : null)?.closest("input,select,textarea,[contenteditable=true]")) return;
      if(isLikeShortcut(event)){event.preventDefault();element.current?.querySelector<HTMLButtonElement>("[data-like]")?.click();return;}
      if ((event.target instanceof Element ? event.target : null)?.closest("button")) return;
      if (event.key === "ArrowLeft") { event.preventDefault(); void navigate("previousId"); }
      if (event.key === "ArrowRight") { event.preventDefault(); void navigate("nextId"); }
      if (event.key === "Escape" && !document.fullscreenElement) { event.preventDefault(); void leave(); }
    };
    window.addEventListener("keydown", keyboard); return () => window.removeEventListener("keydown", keyboard);
  }, [navigate, leave, fullscreen, shuffle, busy, loading, tagQueue]);
  return <div className="viewer-overlay" role="dialog" aria-modal="true" aria-label="媒体查看器" data-activation-id={activationId ?? undefined} data-quick-session={quick?.session.id} ref={element} tabIndex={-1}>
    <header className="viewer-header"><button className="viewer-back" onClick={() => { void leave(); }} disabled={busy}>← 返回媒体库</button><div className="viewer-title truncate" title={name}>{name || "正在打开媒体…"}</div><div className="viewer-header-actions"><LikeButton key={id} mediaId={id} initialCount={likes} /><button aria-pressed={pane === "tags"} onClick={() => setPane(pane === "tags" ? null : "tags")}>Custom Tag</button><button aria-pressed={pane === "details"} onClick={() => setPane(pane === "details" ? null : "details")}>Details</button></div></header>
    {quick && <div className="quick-toolbar"><strong>QUICK BROWSE · {quick.item.ordinal + 1} / {quick.session.itemCount}</strong><span>← → / Space 浏览 · L 点赞 · T 标签 · I 信息 · F 全屏 · Esc 退出</span><button onClick={quick.openSettings}>快捷键设置</button><button disabled={busy} onClick={() => { void shuffle(); }}>Shuffle Again · R</button><div className="quick-bindings">{quick.settings.bindings.map(binding => <span key={binding.digit}><kbd>{binding.digit}</kbd> {binding.name}</span>)}</div><output aria-live="polite">{feedback}</output></div>}
    {tagError && <div className="viewer-notice" role="alert">{tagError}<button onClick={() => { void tagQueue?.flush().then(() => setTagError(""), e => setTagError(toApiError(e).message)); }}>重试标签操作</button></div>}
    {error && <div className="viewer-notice" role="alert">{error}<button onClick={() => { setError(""); if (!session || !activationId) setRetry(retry + 1); else void refreshNavigation(); }}>重试 / 更新导航</button><button onClick={() => { void leave(true); }}>仍然关闭</button></div>}
    <div className="viewer-body"><div className="viewer-content">{loading ? <div className="viewer-loading" role="status"><span className="loading-ring" />正在打开只读媒体…</div> : session ? session.mediaType === "image" ? <ImageViewer key={session.token} session={session} fullscreen={fullscreen} /> : <VideoPlayer key={session.token} activationId={activationId} session={session} fullscreen={fullscreen} register={register} onQualified={refreshAfterActivity} autoplay={quick?.session.autoplay} initiallyMuted={quick?.session.muted} /> : <div className="viewer-loading">媒体无法打开，可继续浏览或重试。</div>}
      <nav className="viewer-navigation" aria-label="媒体切换"><button aria-label="上一个媒体" disabled={busy || loading || (quick ? quick.item.ordinal === 0 : !navigation?.previousId)} onClick={() => { void navigate("previousId"); }}>← Previous</button><span>{session?.mediaType === "video" ? "VIDEO" : "IMAGE"} · 原件只读</span><button aria-label="下一个媒体" disabled={busy || loading || (quick ? quick.item.ordinal + 1 >= quick.session.itemCount : !navigation?.nextId)} onClick={() => { void navigate("nextId"); }}>Next →</button></nav>
    </div>{pane && <div className="viewer-panel">{pane === "details" ? <DetailsPanel id={id} close={() => setPane(null)} revision={revision} changed={changed} /> : <section className="viewer-tags"><div className="panel-heading"><h2>Custom Tags</h2><button className="icon-button" aria-label="关闭标签面板" onClick={() => setPane(null)}>×</button></div><TagSelector mediaIds={[id]} revision={revision} changed={changed} /></section>}</div>}</div>
  </div>;
}
