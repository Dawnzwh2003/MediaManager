import { invoke, isTauri } from "@tauri-apps/api/core";
import { toApiError } from "./contracts.ts";
import { parseImportJob, parseStorage, parseSources } from "../importing/model.ts";
async function call(command: string, args: Record<string, unknown> = {}) {
  if (!isTauri()) throw { code: "desktop_required", message: "请在桌面应用中导入文件夹。", retryable: false };
  try { return await invoke<unknown>(command, args); } catch (e) { throw toApiError(e); }
}
export const importApi = {
  pick: async () => parseImportJob(await call("pick_import_folder")),
  job: async (id: string | null = null) => parseImportJob(await call("get_import_job", { id })),
  start: async (id: string, resume = false) => parseImportJob(await call("start_folder_import", { id, resume })),
  cancel: async (id: string) => { await call("cancel_folder_import", { id }); },
  storage: async () => parseStorage(await call("get_import_storage")),
  pickStorage: async () => { const result = await call("pick_import_storage"); return result === null ? null : parseStorage(result); },
  sources: async (sourceId: string | null, parentId: string | null, after = "") => parseSources(await call("list_import_sources", { sourceId, parentId, after })),
};
