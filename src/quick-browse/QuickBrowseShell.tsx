import { useCallback, useEffect, useRef, useState } from "react";
import type { LibraryFilter, TagMutation } from "../api/library-contracts.ts";
import { quickBrowseApi } from "../api/quick-browse.ts";
import type { BrowseItem, BrowsePreferences, BrowseSession, BrowseSettings } from "../api/quick-browse.ts";
import type { Activation } from "../api/activity.ts";
import { viewerApi } from "../api/viewer.ts";
import { toApiError } from "../api/contracts.ts";
import { ActivationController } from "../viewer/activity-model.ts";
import { ViewerShell } from "../viewer/ViewerShell.tsx";
import { QuickBrowseSettings } from "./QuickBrowseSettings.tsx";
import "./quick-browse.css";

type Current = { session: BrowseSession; item: BrowseItem; activation: Activation };
export function QuickBrowseShell({ filter, revision, changed, close }: { filter: LibraryFilter; revision: number; changed: (mutation?: TagMutation) => void; close: () => void }) {
  const [current, setCurrent] = useState<Current | null>(null);
  const currentRef = useRef(current); currentRef.current = current;
  const [settings, setSettings] = useState<BrowseSettings>({ autoplay: false, muted: true, bindings: [] });
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [building, setBuilding] = useState<BrowseSession | null>(null);
  const [starting, setStarting] = useState(true), [error, setError] = useState("");
  const [empty, setEmpty] = useState(false);
  const frozenFilter = useRef(structuredClone(filter));
  const alive = useRef(true), busy = useRef(false);
  const pending = useRef<string | null>(null), activeSession = useRef<string | null>(null);
  const controller = useRef<ActivationController | null>(null);
  const playerFlush = useRef<(() => Promise<void>) | null>(null);
  const originalFullscreen = useRef(false);
  const registerFlush = useCallback((flush: (() => Promise<void>) | null) => { playerFlush.current = flush; }, []);
  const shutdown = useCallback(async (force = false) => {
    if (!force) await playerFlush.current?.();
    if (pending.current) await quickBrowseApi.close(pending.current);
    if (activeSession.current) await quickBrowseApi.close(activeSession.current);
  }, []);
  const leave = useCallback(async (force = false, alreadyFlushed = false) => {
    if (busy.current && !pending.current) return;
    try { await shutdown(force || alreadyFlushed); await viewerApi.setWindowFullscreen(originalFullscreen.current); alive.current = false; close(); }
    catch (failure) { setError(toApiError(failure).message); }
  }, [shutdown, close]);
  const waitReady = useCallback(async (session: BrowseSession) => {
    if (!alive.current) { await quickBrowseApi.close(session.id); return null; }
    while (alive.current && session.state === "building") {
      setBuilding(session);
      await new Promise(resolve => setTimeout(resolve, 150));
      session = await quickBrowseApi.get(session.id);
    }
    if (!alive.current) { await quickBrowseApi.close(session.id); return null; }
    if (session.state !== "ready") throw new Error(session.error ?? "浏览列表构建已取消，请重试。");
    return session;
  }, []);
  const enter = useCallback(async (session: BrowseSession, flush?: () => Promise<void>) => {
    const ready = await waitReady(session);
    if (!ready) return;
    await flush?.();
    controller.current = new ActivationController(ready.id);
    activeSession.current = ready.id;
    pending.current = null;
    if (!ready.itemCount) { setCurrent(null); setEmpty(true); return; }
    const item = (await quickBrowseApi.items(ready.id, -1))[0];
    if (!item) throw new Error("浏览列表缺少当前项目。");
    await quickBrowseApi.select(ready.id, item.ordinal);
    if (alive.current) setCurrent({ session: ready, item, activation: controller.current.activate(item.mediaId, item.contentVersionId, "quick_browser") });
  }, [waitReady]);
  const start = useCallback(async () => {
    if (busy.current) return;
    busy.current = true; setStarting(true); setError(""); setEmpty(false);
    const id = crypto.randomUUID(); pending.current = id;
    try {
      const preferences = await quickBrowseApi.settings();
      if (!alive.current) return;
      setSettings(preferences);
      await enter(await quickBrowseApi.create(id, frozenFilter.current, preferences));
    } catch (failure) { if (alive.current) setError(toApiError(failure).message); }
    finally { busy.current = false; if (alive.current) { setStarting(false); setBuilding(null); } }
  }, [enter]);
  useEffect(() => {
    alive.current = true;
    void viewerApi.windowFullscreen().then(value => { originalFullscreen.current = value; });
    const timer = setTimeout(() => { void start(); }, 0);
    let unlisten: (() => void) | undefined;
    void viewerApi.onApplicationClose(shutdown).then(cleanup => { if (!alive.current) cleanup(); else unlisten = cleanup; });
    return () => { alive.current = false; clearTimeout(timer); unlisten?.(); if (pending.current) void quickBrowseApi.close(pending.current).catch(() => {}); if (activeSession.current) void quickBrowseApi.close(activeSession.current).catch(() => {}); };
  }, [start, shutdown]);
  const navigate = useCallback(async (step: -1 | 1, flush: () => Promise<void>) => {
    const old = currentRef.current;
    if (!old || busy.current) return;
    const ordinal = old.item.ordinal + step;
    if (ordinal < 0 || ordinal >= old.session.itemCount) return;
    busy.current = true;
    let flushed = false;
    try {
      const item = (await quickBrowseApi.items(old.session.id, ordinal - 1))[0];
      if (!item || item.ordinal !== ordinal) throw new Error("无法读取下一项。");
      await flush();
      flushed = true;
      await quickBrowseApi.select(old.session.id, ordinal);
      setCurrent({ session: { ...old.session, currentOrdinal: ordinal }, item, activation: controller.current!.activate(item.mediaId, item.contentVersionId, "quick_browser") });
    } catch (failure) {
      if (flushed) { controller.current!.leave(); setCurrent({ ...old, activation: controller.current!.activate(old.item.mediaId, old.item.contentVersionId, "quick_browser") }); }
      throw failure;
    } finally { busy.current = false; }
  }, []);
  const shuffle = useCallback(async (flush: () => Promise<void>) => {
    const old = currentRef.current;
    if (!old || busy.current) return;
    busy.current = true; setStarting(true); setError("");
    const id = crypto.randomUUID(); pending.current = id;
    try { await enter(await quickBrowseApi.shuffle(old.session.id, id), flush); }
    catch (failure) { setError(toApiError(failure).message); }
    finally { busy.current = false; if (alive.current) { setStarting(false); setBuilding(null); } }
  }, [enter]);
  const fullscreen = useCallback(() => { void viewerApi.windowFullscreen().then(value => viewerApi.setWindowFullscreen(!value)).catch(e => setError(toApiError(e).message)); }, []);
  const save = async (preferences: BrowsePreferences) => {
    const result = await quickBrowseApi.saveSettings(preferences, activeSession.current);
    setSettings(result);
    setCurrent(old => old ? { ...old, session: { ...old.session, autoplay: result.autoplay, muted: result.muted } } : null);
  };
  useEffect(() => {
    if (current && !starting && !empty) return;
    const keyboard = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); void leave(); } };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [current, starting, empty, leave]);
  return <div className="quick-root">
    {current && <ViewerShell key={current.activation.id} initialId={current.item.mediaId} filter={current.session.filter} revision={revision} changed={changed} close={force => leave(force, true)} quick={{ ...current, settings, settingsOpen: settingsOpen || starting, navigate, shuffle, fullscreen, openSettings: () => setSettingsOpen(true), registerFlush }} />}
    {(starting || empty || error) && <div className="quick-status" role="dialog" aria-modal="true" aria-label="快速浏览状态"><section><span className="eyebrow">QUICK BROWSE</span><h2>{starting ? "正在固定浏览列表…" : empty ? "当前条件下没有媒体" : "快速浏览暂时无法继续"}</h2>{starting && <p role="status">{building ? `${building.phase === "publishing" ? "正在发布" : "已读取"} ${building.builtCount.toLocaleString()} 项` : "正在准备…"}</p>}{error && <p role="alert">{error}</p>}<div>{!starting && !current && <button onClick={() => { void start(); }}>重试</button>}<button onClick={() => { void leave(); }}>{starting ? "取消并退出 · Esc" : "返回媒体库 · Esc"}</button></div></section></div>}
    {settingsOpen && <QuickBrowseSettings settings={settings} revision={revision} save={save} close={() => setSettingsOpen(false)} />}
  </div>;
}
