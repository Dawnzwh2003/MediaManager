import { useEffect, useRef, useState } from "react";
import type { ReportItem, ReportKind } from "../api/report-contracts.ts";
import { useThumbnails } from "../library/hooks.ts";
import { ThumbnailImage } from "../library/MediaCard.tsx";
import { durationText, progress, reportRows } from "./model.ts";

export function ReportList({ items, kind, open, revision, compact = false }: { items: ReportItem[]; kind: ReportKind; open: (item: ReportItem, items: ReportItem[]) => void; revision: number; compact?: boolean }) {
  const element = useRef<HTMLDivElement>(null);
  const [top, setTop] = useState(0), [height, setHeight] = useState(600);
  useEffect(() => { const node = element.current; if (!node || compact) return; const observer = new ResizeObserver(entries => setHeight(entries[0]?.contentRect.height ?? 600)); observer.observe(node); return () => observer.disconnect(); }, [compact]);
  useEffect(() => { setTop(0); if (element.current) element.current.scrollTop = 0; }, [items]);
  const range = compact ? { start: 0, end: items.length } : reportRows(items.length, top, height);
  const visible = items.slice(range.start, range.end);
  const { thumbnails, error } = useThumbnails(visible.filter(i => i.state === "active").map(i => i.id), revision);
  return <div className={`report-list ${compact ? "compact" : ""}`} ref={element} onScroll={e => setTop(e.currentTarget.scrollTop)}>
    {error && <p role="alert">{error.message}</p>}
    {!items.length ? <p className="report-empty">{kind === "continueWatching" ? "没有未完成的视频。" : "还没有符合条件的记录。"}</p> : <div role="list" aria-label={kind} style={compact ? undefined : { height: items.length * 96, position: "relative" }}><div style={compact ? undefined : { position: "absolute", inset: "0 0 auto", transform: `translateY(${range.start * 96}px)` }}>
      {visible.map((item, index) => {
        const percent = progress(item);
        return <div className="report-row" role="listitem" key={item.id} data-media-id={item.id}>
          <button className="report-open" disabled={item.state !== "active"} onClick={() => open(item, items)} aria-label={`打开 ${item.fileName}`}>
            <span className="report-thumb"><ThumbnailImage thumbnail={thumbnails[item.id]} name={item.fileName} /></span>
            <span className="report-description"><strong className="truncate">{item.fileName}</strong><span>{item.mediaType === "video" ? "VIDEO" : "IMAGE"} · {new Date(item.lastAt).toLocaleString()}</span>
              {item.position !== null && <span className="watch-progress"><span>{durationText(item.position)} / {item.duration ? durationText(item.duration) : "时长未知"}{percent !== null ? ` · ${Math.floor(percent)}%` : ""}</span>{percent !== null && <progress aria-label={`${item.fileName} 播放进度`} max={100} value={percent} />}</span>}
            </span>
          </button>
          <span className="report-value">{item.state !== "active" ? <small>{item.state === "missing" ? "文件暂不可用" : "原文件已被替换"}</small> : kind === "continueWatching" ? "续播 →" : kind === "recentlyAdded" ? "NEW" : <><strong>{item.count.toLocaleString()}</strong><small>{kind === "mostLiked" ? "Likes" : kind.includes("Played") ? "Plays" : "Views"}</small></>}<span className="sr-only">第 {range.start + index + 1} 项</span></span>
        </div>;
      })}
    </div></div>}
  </div>;
}
