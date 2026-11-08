import { useEffect, useState } from "react";
import { thumbnailUrl } from "../api/library.ts";
import type { LibraryMedia, Thumbnail } from "../api/library-contracts.ts";
export function ThumbnailImage({ thumbnail, name }: { thumbnail?: Thumbnail; name: string }) {
  const [broken, setBroken] = useState(false);
  useEffect(() => setBroken(false), [thumbnail?.path]);
  if (thumbnail?.status === "ready" && thumbnail.path && !broken) return <img src={thumbnailUrl(thumbnail.path)} loading="lazy" decoding="async" alt={name} onError={() => setBroken(true)} />;
  return <div className="thumbnail-placeholder"><span aria-hidden="true">▧</span><small>{broken ? "缩略图读取失败" : thumbnail?.status === "failed" ? "缩略图不可用" : thumbnail?.status === "unavailable" ? "媒体暂不可用" : "等待缩略图"}</small></div>;
}
export function MediaCard({ media, thumbnail, selected, focused, toggle, open, view, selectionFull }: { media: LibraryMedia; thumbnail?: Thumbnail; selected: boolean; focused: boolean; toggle: () => void; open: () => void; view: () => void; selectionFull: boolean }) {
  return <article className={`media-card ${focused ? "focused" : ""} ${selected ? "selected" : ""}`}>
    <div className="card-preview"><button className="preview-button" onClick={view} aria-label={`打开 ${media.fileName}`}><ThumbnailImage thumbnail={thumbnail} name={media.fileName} /></button><span className="media-type">{media.mediaType === "video" ? "VIDEO" : "IMAGE"}</span><input className="card-select" type="checkbox" checked={selected} disabled={!selected && selectionFull} onChange={toggle} aria-label={`选择 ${media.fileName}`} /></div>
    <div className="card-info"><button className="file-name truncate" title={media.fileName} onClick={open}>{media.fileName}</button><div className="card-meta"><span>{media.width && media.height ? `${media.width} × ${media.height}` : media.mediaType === "video" ? "视频" : "图片"}</span><span aria-label={`Like Count ${media.likeCount}`}>♡ {media.likeCount.toLocaleString()}</span></div>
      <div className="card-tags" aria-label="Custom Tags">{media.customTags.slice(0, 2).map(tag => <span className="tag-chip truncate" key={tag.id} title={tag.name}>{tag.name}</span>)}{media.customTagCount > 2 && <span className="tag-overflow">+{media.customTagCount - 2}</span>}{!media.customTagCount && <span className="no-tags">未添加标签</span>}</div>
    </div>
  </article>;
}
