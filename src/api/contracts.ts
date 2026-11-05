export interface ApiError {
  code: string;
  message: string;
  retryable: boolean;
}

export interface AppStatus {
  appName: string;
  appVersion: string;
  phase: 1;
  database: {
    schemaVersion: number;
    sqliteVersion: string;
    foreignKeys: boolean;
    journalMode: string;
    busyTimeoutMs: number;
  };
  storage: {
    dataReady: boolean;
    cacheReady: boolean;
    thumbnailsReady: boolean;
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function toApiError(value: unknown): ApiError {
  if (
    isRecord(value) &&
    typeof value.code === "string" &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  ) {
    return { code: value.code, message: value.message, retryable: value.retryable };
  }
  // Unknown exceptions can contain paths or SQL; do not display their raw text.
  return { code: "internal", message: "无法连接基础服务，请重试。", retryable: true };
}

export function parseAppStatus(value: unknown): AppStatus {
  if (isRecord(value)) {
    const database = value.database;
    const storage = value.storage;
    if (
      typeof value.appName === "string" &&
      typeof value.appVersion === "string" &&
      value.phase === 1 &&
      isRecord(database) &&
      Number.isSafeInteger(database.schemaVersion) &&
      Number(database.schemaVersion) >= 1 &&
      typeof database.sqliteVersion === "string" &&
      typeof database.foreignKeys === "boolean" &&
      typeof database.journalMode === "string" &&
      Number.isSafeInteger(database.busyTimeoutMs) &&
      Number(database.busyTimeoutMs) >= 0 &&
      isRecord(storage) &&
      typeof storage.dataReady === "boolean" &&
      typeof storage.cacheReady === "boolean" &&
      typeof storage.thumbnailsReady === "boolean"
    ) {
      return value as unknown as AppStatus;
    }
  }
  throw { code: "invalid_response", message: "基础服务返回的数据格式不正确。", retryable: false } satisfies ApiError;
}
