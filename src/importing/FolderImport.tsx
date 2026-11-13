import { useEffect, useRef, useState } from "react";
import { importApi } from "../api/importing.ts";
import { toApiError } from "../api/contracts.ts";
import { bytesLabel, importProgress, isImportRunning } from "./model.ts";
import type { ImportJob, ImportStorageSettings } from "./model.ts";
import "./importing.css";

const failureLabel = (code: string) => ({ source_changed: "文件已变化，请重新预览", insufficient_storage: "存储空间不足", source_unavailable: "来源暂时不可访问", unsafe_path: "路径不安全或目标冲突", database_error: "关联保存失败，可恢复后重试", copy_or_read_failed: "读取或复制失败" }[code] ?? code);
export function FolderImport({ changed }: { changed: () => void }) {
  const [open, setOpen] = useState<"import" | "settings" | null>(null);
  const [job, setJob] = useState<ImportJob | null>(null);
  const [settings, setSettings] = useState<ImportStorageSettings | null>(null);
  const [error, setError] = useState(""); const [busy, setBusy] = useState(false);
  const reported = useRef("");
  const jobId = job?.id ?? null, running = isImportRunning(job);
  useEffect(() => {
    let active = true; let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const latest = await importApi.job(jobId);
        if (!active) return;
        setJob(latest);
        if (latest && !isImportRunning(latest) && latest.state !== "preview" && reported.current !== `${latest.id}:${latest.state}`) {
          reported.current = `${latest.id}:${latest.state}`; changed();
        }
      } catch (e) { if (active && open) setError(toApiError(e).message); }
      if (active && (jobId === null || running)) timer = setTimeout(() => { void poll(); }, 1000);
    };
    void poll(); return () => { active = false; clearTimeout(timer); };
  }, [jobId, running, changed, open]);
  async function action(work: () => Promise<void>) { setBusy(true); setError(""); try { await work(); } catch (e) { setError(toApiError(e).message); } finally { setBusy(false); } }
  return <><div className="import-actions"><button className="primary" onClick={() => { setOpen("import"); setError(""); }}>Import Folder {running ? "· 导入进行中" : ""}</button><button className="secondary" onClick={() => { setOpen("settings"); void action(async () => setSettings(await importApi.storage())); }}>Settings</button></div>
    {open && <div className="modal-backdrop"><section className="tag-modal import-modal" role="dialog" aria-modal="true" aria-labelledby="import-title">
      <div className="panel-heading"><h2 id="import-title">{open === "settings" ? "Settings · 媒体存储" : "Import Folder"}</h2><button className="icon-button" aria-label="关闭导入窗口" onClick={() => setOpen(null)}>×</button></div>
      {error && <p role="alert" className="inline-error">{error}</p>}
      {open === "settings" ? <><p>媒体 data 存储位置</p><code className="import-path">{settings?.mediaRoot ?? "尚未读取"}</code><p className="muted">可用空间：{settings ? bytesLabel(settings.availableBytes) : "—"}</p><button className="secondary" disabled={busy || running || job?.state === "preview"} onClick={() => { void action(async () => { const next = await importApi.pickStorage(); if (next) setSettings(next); }); }}>选择存储文件夹</button><p>仅改变后续新文件的目标。已有媒体、数据库和历史保留原位置；新导入媒体按稳定 ID 独立存储。</p><p className="muted">来源与所有托管存储目录不能互相包含。原始文件保留。</p></> : <>
        <p>复制支持的图片与视频，保留原始文件。精确同名与完整内容相同的媒体复用历史；来源目录另存为只读 Source Tags。</p>
        <button className="secondary" disabled={busy || running || job?.state === "preview" || job?.state === "interrupted"} onClick={() => { void action(async () => { const next = await importApi.pick(); if (next) setJob(next); }); }}>选择外部文件夹…</button>
        {job && <><p className="import-path">Source: {job.sourceRoot}</p><p className="import-path muted">Data: {job.storageRoot}</p><p role="status">{({ scanning: "正在扫描和核验完整内容…", preview: "导入预览", importing: "正在导入…", completed: "导入完成", cancelled: "已取消，已完成的媒体保留", interrupted: "导入中断，可恢复", failed: "任务未完成" }[job.state])}</p>
          <dl className="import-counts">{([["Scanned Files", job.scannedFiles], ["New Media", job.newMedia], ["Existing Media", job.existingMedia], ["Duplicate Files", job.duplicateFiles], ["New Tags", job.newTags], ["Conflicts", job.conflicts]] as const).map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value.toLocaleString()}</dd></div>)}</dl>
          <p>Estimated Storage Required: {bytesLabel(job.estimatedStorageRequired)}</p>
          {job.stage === "import" && <><p>Imported / Total: {job.processed} / {job.scannedFiles} · {importProgress(job)}%</p><progress aria-label="导入进度" max={Math.max(1, job.scannedFiles)} value={job.processed} /><p>Newly Imported: {job.newlyImported} · Existing Media Reused: {job.existingMediaReused} · Tags Added: {job.tagsAdded}</p></>}
          {job.currentFile && <p className="import-path">Current File: {job.currentFile}</p>}<p>Failed Files: {job.failedFiles}</p>
          {job.error && <p role="alert">{failureLabel(job.error)}</p>}
          {job.failures.length > 0 && <ul className="import-errors">{job.failures.map((f, i) => <li key={i}>{f.relativePath}: {failureLabel(f.error)}</li>)}</ul>}
          {job.failures.length === 20 && <p className="muted small">仅显示前 20 个错误。</p>}
          <div className="import-actions">
            {job.state === "preview" && <button className="primary" disabled={busy} onClick={() => { void action(async () => setJob(await importApi.start(job.id))); }}>开始导入</button>}
            {(running || job.state === "preview" || job.state === "interrupted") && <button className="secondary" disabled={busy} onClick={() => { void action(async () => { await importApi.cancel(job.id); setJob(await importApi.job(job.id)); }); }}>Cancel</button>}
            {(job.state === "cancelled" || job.state === "interrupted" || job.state === "failed" || (job.state === "completed" && job.failedFiles > 0)) && <button className="secondary" disabled={busy} onClick={() => { void action(async () => setJob(await importApi.start(job.id, true))); }}>Resume / Retry</button>}
          </div>
        </>}
      </>}
    </section></div>}
  </>;
}
