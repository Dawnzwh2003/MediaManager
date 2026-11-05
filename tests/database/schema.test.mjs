import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

test("the initial SQL creates only strict application metadata and a valid timestamp", async () => {
  const sql = await readFile(new URL("../../src-tauri/migrations/0001_app_metadata.sql", import.meta.url), "utf8");
  const database = new DatabaseSync(":memory:");
  try {
    database.exec(sql);
    const tables = database.prepare("SELECT name FROM sqlite_schema WHERE type='table'").all();
    assert.deepEqual(tables.map((row) => row.name), ["app_metadata"]);
    const row = database.prepare("SELECT value FROM app_metadata WHERE key='database_created_at'").get();
    assert.match(row.value, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/);
    assert.throws(() => database.exec("INSERT INTO app_metadata VALUES ('database_created_at','overwrite')"));
  } finally {
    database.close();
  }
});
