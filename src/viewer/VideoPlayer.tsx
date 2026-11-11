import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { viewerApi } from "../api/viewer.ts";
import type { PlaybackMode, ViewerOpen } from "../api/viewer.ts";
import { toApiError } from "../api/contracts.ts";
import { formatTime, ProgressSaver } from "./model.ts";
import {activityApi} from "../api/activity.ts";
import type {PlaybackPhase} from "../api/activity.ts";
import {PlaybackReporter} from "./activity-model.ts";
export interface PlayerControl { stopAndFlush: () => Promise<void> }
export function VideoPlayer({ session, activationId, fullscreen, register, onQualified }: { activationId:string|null; session: ViewerOpen; fullscreen: () => void; register: (control: PlayerControl | null) => void; onQualified: () => void }) {
  const video = useRef<HTMLVideoElement>(null);
  const saver = useMemo(() => new ProgressSaver(session.token, session.state!, request => viewerApi.save(request)), [session]);
  const announcedQualification = useRef(false);
  const reporter = useMemo(() => activationId ? new PlaybackReporter(activationId, async evidence => {
    const result = await activityApi.report(evidence);
    if (result.qualified && !announcedQualification.current) {
      announcedQualification.current = true;
      onQualified();
    }
    return result;
  }) : null, [activationId, onQualified]);
  const [activityError,setActivityError]=useState("");
  const emit=useCallback((phase:PlaybackPhase)=>{const v=video.current;if(!reporter||!v||(!loaded.current && phase!=="waiting"))return Promise.resolve();return reporter.report(phase,v.currentTime,Number.isFinite(v.duration)&&v.duration>0?v.duration:null,v.playbackRate);},[reporter]);
  const report=useCallback((phase:PlaybackPhase)=>{void emit(phase).catch(e=>{setActivityError(toApiError(e).message);video.current?.pause();});},[emit]);
  useEffect(()=>{const timer=setInterval(()=>{const v=video.current;if(v&&!v.paused&&!v.seeking&&v.readyState>=3)report("playing");},1000);return()=>clearInterval(timer)},[report]);
  const [mode, setMode] = useState<PlaybackMode>("direct");
  const [preparing, setPreparing] = useState(false);
  const [error, setError] = useState(""); const [saveError, setSaveError] = useState("");
  const [position, setPosition] = useState(session.state!.positionSeconds); const [duration, setDuration] = useState(session.state!.durationSeconds ?? 0);
  const [playing, setPlaying] = useState(false); const [volume, setVolume] = useState(.8); const [muted, setMuted] = useState(false); const [speed, setSpeed] = useState(1);
  const loaded = useRef(false), alive = useRef(true), fallbackBusy = useRef(false), wantsPlay = useRef(false);
  const resume = useRef(session.state!.positionSeconds), last = useRef(session.state!.positionSeconds);
  const snapshot = useCallback(() => {
    const element = video.current;
    if (!element || !loaded.current || !Number.isFinite(element.currentTime)) return saver.flush();
    const d = Number.isFinite(element.duration) && element.duration > 0 ? element.duration : null;
    last.current = d ? Math.min(d, element.currentTime) : element.currentTime;
    return saver.checkpoint({ positionSeconds: Math.max(0, last.current), durationSeconds: d });
  }, [saver]);
  const checkpoint = useCallback(() => { void snapshot().then(() => { if (alive.current) setSaveError(""); }, e => { if (alive.current) setSaveError(toApiError(e).message); }); }, [snapshot]);
  useEffect(() => {
    alive.current = true;
    register({ stopAndFlush: async () => { video.current?.pause(); await snapshot(); await emit("paused"); await reporter?.flush(); } });
    const timer = setInterval(checkpoint, 5000);
    const hidden = () => { if (document.visibilityState === "hidden") checkpoint(); };
    window.addEventListener("pagehide", checkpoint); window.addEventListener("beforeunload", checkpoint); document.addEventListener("visibilitychange", hidden);
    return () => { alive.current = false; clearInterval(timer); register(null); window.removeEventListener("pagehide", checkpoint); window.removeEventListener("beforeunload", checkpoint); document.removeEventListener("visibilitychange", hidden); void snapshot().catch(() => {}); };
  }, [register, snapshot, checkpoint, emit, reporter]);
  useEffect(() => { const element = video.current; if (element) { element.volume = volume; element.muted = muted; element.playbackRate = speed; } }, [volume, muted, speed, mode]);
  const fallback = async () => {
    if (fallbackBusy.current || !alive.current) return;
    if (mode === "proxy") { setError("此媒体无法播放。可关闭后重试；原文件未被修改。"); return; }
    report("waiting"); fallbackBusy.current = true; loaded.current = false; resume.current = last.current; setPreparing(true); setError("");
    const next = mode === "direct" ? "remux" : "proxy";
    try { await viewerApi.prepare(session.token, next); if (alive.current) setMode(next); }
    catch (e) {
      if (next === "remux" && alive.current) { try { await viewerApi.prepare(session.token, "proxy"); if (alive.current) setMode("proxy"); } catch (failure) { if (alive.current) setError(toApiError(failure).message); } }
      else if (alive.current) setError(toApiError(e).message);
    } finally { fallbackBusy.current = false; if (alive.current) setPreparing(false); }
  };
  return <div className="video-player"><div className="viewer-stage video-stage"><video key={`${session.token}-${mode}`} ref={video} src={viewerApi.resourceUrl(session.token, mode)} preload="metadata" crossOrigin="anonymous" playsInline aria-label="视频播放器"
    onLoadedMetadata={e => { const element = e.currentTarget; if (element !== video.current) return; if (!element.videoWidth) { void fallback(); return; } setError(""); const d = Number.isFinite(element.duration) ? element.duration : 0; setDuration(d); const target = d > 0 ? Math.min(resume.current, d) : resume.current; element.volume = volume; element.muted = muted; element.playbackRate = speed; if (target > 0) element.currentTime = target; else loaded.current = true; setPosition(target); if (wantsPlay.current) void element.play().catch(() => setPlaying(false)); }}
    onSeeked={e => { loaded.current = true; last.current = e.currentTarget.currentTime; setPosition(last.current); }}
    onTimeUpdate={e => { if (loaded.current) { last.current = e.currentTarget.currentTime; setPosition(last.current); } }}
    onPlaying={() => { report("playing"); }} onWaiting={() => { report("waiting"); }} onSeeking={() => { report("seeking"); }} onRateChange={() => { const v=video.current; if(v) report(v.paused?"paused":"playing"); }}
    onPlay={() => { setPlaying(true); wantsPlay.current = true; }} onPause={e => { if (e.currentTarget !== video.current) return; setPlaying(false); if (!fallbackBusy.current) wantsPlay.current = false; report("paused"); checkpoint(); }} onEnded={() => { wantsPlay.current = false; setPlaying(false); report("ended"); checkpoint(); }} onError={() => { void fallback(); }} />
    {preparing && <div className="viewer-preparing" role="status"><span className="loading-ring" />正在准备兼容的视频…<small>可关闭查看器以取消</small></div>}{error && <div className="viewer-error" role="alert">{error}<button onClick={() => { setError(""); if (mode === "proxy") video.current?.load(); else void fallback(); }}>重试</button></div>}
  </div><div className="viewer-controls video-controls"><div className="timeline-row"><span>{formatTime(position)}</span><input aria-label="播放进度" type="range" min={0} max={duration || 1} step={.1} value={Math.min(position, duration || 1)} disabled={!duration || preparing} onChange={e => { if (video.current) { video.current.currentTime = Number(e.target.value); setPosition(Number(e.target.value)); } }} /><span>{formatTime(duration)}</span></div>
    <div className="transport-row"><button aria-label={playing ? "暂停" : "播放"} disabled={!activationId || preparing || !!error} onClick={() => { const element = video.current; if (!element) return; if (!element.paused) element.pause(); else { wantsPlay.current = true; if (duration && element.currentTime >= duration) element.currentTime = 0; void element.play().catch(() => { if (element === video.current && !fallbackBusy.current && !element.error) setError("播放未能开始，请重试。"); }); } }}>{playing ? "Ⅱ 暂停" : "▶ 播放"}</button><button aria-label={muted ? "取消静音" : "静音"} onClick={() => setMuted(!muted)}>{muted ? "取消静音" : "静音"}</button><input aria-label="音量" className="volume-slider" type="range" min={0} max={1} step={.05} value={muted ? 0 : volume} onChange={e => { setVolume(Number(e.target.value)); setMuted(false); }} /><label className="speed-label">速度 <select aria-label="播放速度" value={speed} onChange={e => setSpeed(Number(e.target.value))}>{[.5,.75,1,1.25,1.5,2].map(value => <option key={value} value={value}>{value}×</option>)}</select></label><span className="transport-spacer" /><button onClick={fullscreen}>全屏</button></div>
    {activityError && <p className="progress-error" role="alert">活动记录失败：{activityError}<button onClick={()=>{void reporter?.flush().then(()=>setActivityError(""),e=>setActivityError(toApiError(e).message));}}>重试记录</button></p>}
    {saveError && <p className="progress-error" role="alert">进度保存失败：{saveError}<button onClick={checkpoint}>重试保存</button></p>}
  </div></div>;
}
