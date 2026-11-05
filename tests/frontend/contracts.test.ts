import assert from "node:assert/strict";
import { test } from "node:test";
import { parseAppStatus, toApiError } from "../../src/api/contracts.ts";

const status = {
  appName: "Media Manager",
  appVersion: "0.1.0",
  phase: 1,
  database: { schemaVersion: 1, sqliteVersion: "3.51.3", foreignKeys: true, journalMode: "wal", busyTimeoutMs: 2000 },
  storage: { dataReady: true, cacheReady: true, thumbnailsReady: true },
};

test("accepts the camelCase Rust status contract", () => {
  assert.equal(parseAppStatus(status).database.schemaVersion, 1);
});

test("rejects incomplete, wrong-version and malformed IPC results", () => {
  for (const value of [null, {}, { ...status, phase: 2 }, { ...status, database: { ...status.database, schemaVersion: "1" } }]) {
    assert.throws(() => parseAppStatus(value), (error) => toApiError(error).code === "invalid_response");
  }
});

test("preserves typed errors while hiding unknown native exception details", () => {
  const busy = { code: "busy", message: "请重试", retryable: true };
  assert.deepEqual(toApiError(busy), busy);
  const error = toApiError(new Error("SQL with C:\\private\\secret.db"));
  assert.equal(error.code, "internal");
  assert.ok(!error.message.includes("secret"));
});
