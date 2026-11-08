import { useCallback, useEffect, useRef, useState } from "react";
import { viewerApi } from "../api/viewer.ts";
import type { ViewerOpen } from "../api/viewer.ts";
import { clampPan, fitScale, zoomAt } from "./model.ts";
import type { ImageTransform } from "./model.ts";
export function ImageViewer({ session, fullscreen }: { session: ViewerOpen; fullscreen: () => void }) {
  const stage = useRef<HTMLDivElement>(null);
  const [image, setImage] = useState({ width: 0, height: 0 });
  const [size, setSize] = useState({ width: 1, height: 1 });
  const [transform, setTransform] = useState<ImageTransform>({ scale: 1, x: 0, y: 0 });
  const [mode, setMode] = useState<"fit" | "custom">("fit");
  const [error, setError] = useState(false);
  const drag = useRef<{ x: number; y: number; start: ImageTransform } | null>(null);
  const clamp = useCallback((t: ImageTransform) => clampPan(t, image.width, image.height, size.width, size.height), [image, size]);
  const fit = useCallback(() => { setMode("fit"); setTransform({ scale: fitScale(image.width, image.height, size.width, size.height), x: 0, y: 0 }); }, [image, size]);
  useEffect(() => { const element = stage.current; if (!element) return; const observer = new ResizeObserver(([entry]) => { if (entry) setSize({ width: entry.contentRect.width, height: entry.contentRect.height }); }); observer.observe(element); return () => observer.disconnect(); }, []);
  useEffect(() => { if (mode === "fit") fit(); else setTransform(t => clamp(t)); }, [mode, fit, clamp]);
  useEffect(() => {
    const element = stage.current; if (!element) return;
    const wheel = (event: WheelEvent) => { event.preventDefault(); const rect = element.getBoundingClientRect(); setMode("custom"); setTransform(t => clamp(zoomAt(t, t.scale * (event.deltaY < 0 ? 1.15 : 1 / 1.15), event.clientX - rect.left - rect.width / 2, event.clientY - rect.top - rect.height / 2))); };
    element.addEventListener("wheel", wheel, { passive: false }); return () => element.removeEventListener("wheel", wheel);
  }, [clamp]);
  return <div className="image-viewer"><div className="viewer-stage image-stage" ref={stage} aria-label="图片查看区域" tabIndex={0}
    onPointerDown={e => { if (e.button !== 0 || !image.width) return; e.currentTarget.setPointerCapture(e.pointerId); drag.current = { x: e.clientX, y: e.clientY, start: transform }; }}
    onPointerMove={e => { if (!drag.current) return; const d = drag.current; setTransform(clamp({ ...d.start, x: d.start.x + e.clientX - d.x, y: d.start.y + e.clientY - d.y })); }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
    {error ? <div className="viewer-error" role="alert">图片无法解码或内容已变化。可关闭查看器并重新扫描后重试。</div> : <img className="viewer-original" src={viewerApi.resourceUrl(session.token, "direct")} alt="当前图片" draggable={false} decoding="async" onLoad={e => setImage({ width: e.currentTarget.naturalWidth, height: e.currentTarget.naturalHeight })} onError={() => setError(true)} style={{ width: image.width || undefined, height: image.height || undefined, transform: `translate(-50%,-50%) translate(${transform.x}px,${transform.y}px) scale(${transform.scale})` }} />}
  </div><div className="viewer-controls image-controls"><button aria-label="缩小" disabled={!image.width || error} onClick={() => { setMode("custom"); setTransform(t => clamp(zoomAt(t, t.scale / 1.25))); }}>−</button><output aria-label="缩放比例">{Math.round(transform.scale * 100)}%</output><button aria-label="放大" disabled={!image.width || error} onClick={() => { setMode("custom"); setTransform(t => clamp(zoomAt(t, t.scale * 1.25))); }}>＋</button><span className="control-divider" /><button onClick={fit} disabled={!image.width}>Fit</button><button onClick={() => { setMode("custom"); setTransform({ scale: 1, x: 0, y: 0 }); }} disabled={!image.width}>100%</button><button onClick={fullscreen}>全屏</button><span className="viewer-hint">滚轮缩放 · 拖动平移</span></div></div>;
}
