import { invoke, isTauri } from "@tauri-apps/api/core";
import { parseAppStatus, toApiError } from "./contracts.ts";
import type { AppStatus } from "./contracts.ts";

export async function getAppStatus(): Promise<AppStatus> {
  if (!isTauri()) {
    throw {
      code: "desktop_required",
      message: "请通过桌面应用启动，以连接本地基础服务。",
      retryable: false,
    };
  }
  try {
    return parseAppStatus(await invoke<unknown>("get_app_status"));
  } catch (error) {
    throw toApiError(error);
  }
}
