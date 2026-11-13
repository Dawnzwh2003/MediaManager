export interface ImportJob {
  id: string; sourceRoot: string; storageRoot: string; state: string; stage: string;
  scannedFiles: number; newMedia: number; existingMedia: number; duplicateFiles: number;
  newTags: number; conflicts: number; estimatedStorageRequired: number; processed: number;
  newlyImported: number; existingMediaReused: number; tagsAdded: number; failedFiles: number;
  currentFile: string | null; error: string | null; failures: { relativePath: string; error: string }[];
}
export interface ImportStorageSettings { mediaRoot: string; availableBytes: number }
export interface SourceNode { id: string; name: string; sourceId: string; parentId: string | null; relativePath: string; hasChildren: boolean; root: boolean }
export interface SourcePage { items: SourceNode[]; nextAfter: string | null }
const record = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;
const count = (v: unknown) => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const nullable = (v: unknown) => v === null || typeof v === "string";
function invalid(): never { throw { code: "invalid_response", message: "导入服务返回的数据格式不正确。", retryable: true }; }
export function parseImportJob(value: unknown): ImportJob | null {
  if (value === null) return null;
  if (!record(value) || !["id", "sourceRoot", "storageRoot", "stage"].every(k => typeof value[k] === "string")
    || !["scanning", "preview", "importing", "completed", "cancelled", "interrupted", "failed"].includes(String(value.state))
    || !["scannedFiles", "newMedia", "existingMedia", "duplicateFiles", "newTags", "conflicts", "estimatedStorageRequired", "processed", "newlyImported", "existingMediaReused", "tagsAdded", "failedFiles"].every(k => count(value[k]))
    || !nullable(value.currentFile) || !nullable(value.error) || !Array.isArray(value.failures) || value.failures.length > 20
    || !value.failures.every(f => record(f) && typeof f.relativePath === "string" && typeof f.error === "string")) invalid();
  return value as unknown as ImportJob;
}
export function parseStorage(value: unknown): ImportStorageSettings {
  if (!record(value) || typeof value.mediaRoot !== "string" || !count(value.availableBytes)) invalid();
  return value as unknown as ImportStorageSettings;
}
export function parseSources(value: unknown): SourcePage {
  if (!record(value) || !Array.isArray(value.items) || value.items.length > 100 || !nullable(value.nextAfter)
    || !value.items.every(n => record(n) && ["id", "name", "sourceId", "relativePath"].every(k => typeof n[k] === "string") && nullable(n.parentId) && typeof n.hasChildren === "boolean" && typeof n.root === "boolean")) invalid();
  return value as unknown as SourcePage;
}
export const isImportRunning = (job: ImportJob | null) => job?.state === "scanning" || job?.state === "importing";
export const importProgress = (job: ImportJob) => job.scannedFiles ? Math.min(100, Math.round(job.processed / job.scannedFiles * 100)) : 0;
export function bytesLabel(bytes: number) {
  const unit = bytes >= 1024 * 1024 ? "MiB" : bytes >= 1024 ? "KiB" : "B";
  const divisor = unit === "MiB" ? 1024 * 1024 : unit === "KiB" ? 1024 : 1;
  return `${(bytes / divisor).toLocaleString(undefined, { maximumFractionDigits: 1 })} ${unit}`;
}
