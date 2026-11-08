import { useEffect, useState } from "react";
import { libraryApi } from "../api/library.ts";
import type { LibraryDetails, TagMutation } from "../api/library-contracts.ts";
import { toApiError } from "../api/contracts.ts";
import { ThumbnailImage } from "./MediaCard.tsx";
import { TagSelector } from "./TagSelector.tsx";
import { useThumbnails } from "./hooks.ts";
export function DetailsPanel({ id, close, revision, changed }: { id: string | null; close: () => void; revision: number; changed: (mutation?: TagMutation) => void }) {
  const [details, setDetails] = useState<LibraryDetails | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const { thumbnails } = useThumbnails(id ? [id] : [], revision + retry);
  useEffect(() => {
    let active = true; setDetails(null); setError("");
    if (id) libraryApi.details(id).then(result => { if (active) setDetails(result); }, e => { if (active) setError(toApiError(e).message); });
    return () => { active = false; };
  }, [id, revision, retry]);
  return <aside className={`details-panel ${id ? "has-details" : ""}`} aria-label="媒体详细信息"><div className="panel-heading"><h2>详细信息</h2>{id && <button className="icon-button" onClick={close} aria-label="关闭详情">×</button>}</div>
    {!id ? <div className="details-empty"><span>▧</span><h3>让每份素材都有归处</h3><p>选择媒体查看 metadata，<br />或为它添加 Custom Tags。</p></div> : <>
      {error ? <div role="alert"><p className="inline-error">{error}</p><button className="secondary" onClick={() => setRetry(retry + 1)}>重试</button></div> : !details ? <p role="status" className="muted">正在读取详细信息…</p> : <>
        <div className="detail-preview"><ThumbnailImage thumbnail={thumbnails[id]} name={details.media.fileName} /></div><h3 className="detail-name">{details.media.fileName}</h3><span className="type-label">{details.media.mediaType === "video" ? "VIDEO" : "IMAGE"}</span>
        <dl className="metadata"><div><dt>尺寸</dt><dd>{details.media.width && details.media.height ? `${details.media.width} × ${details.media.height}` : "未知"}</dd></div><div><dt>文件大小</dt><dd>{(details.fileSize / 1024 / 1024).toFixed(2)} MB</dd></div>{details.media.mediaType === "video" && <><div><dt>时长</dt><dd>{details.media.duration === null ? "未知" : `${details.media.duration.toFixed(1)} 秒`}</dd></div><div><dt>Codec</dt><dd>{details.videoCodec ?? "未知"}</dd></div><div><dt>Frame rate</dt><dd>{details.frameRate?.toFixed(2) ?? "未知"}</dd></div></>}<div><dt>Like Count</dt><dd>{details.media.likeCount.toLocaleString()}</dd></div></dl>
        <label className="field-label">文件位置<span className="file-path">{details.filePath}</span></label>
        {thumbnails[id]?.status === "failed" && <button className="text-button" onClick={() => { void libraryApi.thumbnails([id], true).then(() => setRetry(retry + 1), e => setError(toApiError(e).message)); }}>重新生成缩略图</button>}
        <div className="detail-tags-heading"><h3>Custom Tags</h3><span className="muted small">{details.media.customTagCount} 个</span></div><TagSelector mediaIds={[id]} revision={revision} changed={changed} />
      </>}
    </>}
  </aside>;
}
