import { useEffect, useRef, useState } from "react";
import type { LibraryMedia } from "../api/library-contracts.ts";
import { useThumbnails } from "./hooks.ts";
import { MediaCard } from "./MediaCard.tsx";
import { MAX_SELECTION, ROW_HEIGHT, virtualRows } from "./model.ts";
export function MediaGrid({ items, selected, focused, toggle, open, revision }: { items: LibraryMedia[]; selected: ReadonlySet<string>; focused: string | null; toggle: (id: string) => void; open: (id: string) => void; revision: number }) {
  const container = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 800, height: 600 });
  const [scrollTop, setScrollTop] = useState(0);
  useEffect(() => { const element = container.current; if (!element) return; const observer = new ResizeObserver(entries => { const rect = entries[0]?.contentRect; if (rect) setSize({ width: rect.width, height: rect.height }); }); observer.observe(element); return () => observer.disconnect(); }, []);
  useEffect(() => { if (container.current) container.current.scrollTop = 0; setScrollTop(0); }, [items]);
  const columns = Math.max(1, Math.floor((size.width + 16) / 206));
  const range = virtualRows(items.length, columns, scrollTop, size.height);
  const visible = items.slice(range.start, range.end);
  const { thumbnails, error } = useThumbnails(visible.map(m => m.id), revision);
  return <div className="grid-scroll" ref={container} onScroll={e => setScrollTop(e.currentTarget.scrollTop)} aria-label="媒体网格">
    {error && <p role="alert" className="inline-error">{error.message}</p>}
    <div className="grid-spacer" style={{ height: range.totalHeight }}><div className="virtual-grid" style={{ transform: `translateY(${range.offset}px)`, gridTemplateColumns: `repeat(${columns}, minmax(0,1fr))`, gridAutoRows: ROW_HEIGHT - 16 }}>{visible.map(media => <MediaCard key={media.id} media={media} thumbnail={thumbnails[media.id]} selected={selected.has(media.id)} focused={focused === media.id} toggle={() => toggle(media.id)} open={() => open(media.id)} selectionFull={selected.size >= MAX_SELECTION} />)}</div></div>
  </div>;
}
