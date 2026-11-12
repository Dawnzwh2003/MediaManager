import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));

async function sourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? sourceFiles(target) : [target];
  }));
  return nested.flat();
}

test("frontend IPC imports stay in api and no SQL/filesystem/shell plugin is installed", async () => {
  const manifest = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
  const dependencies = Object.keys({ ...manifest.dependencies, ...manifest.devDependencies });
  assert.ok(!dependencies.some((name) => /plugin-(sql|fs|shell)/.test(name)));
  for (const file of await sourceFiles(path.join(root, "src"))) {
    const source = await readFile(file, "utf8");
    assert.ok(!/@tauri-apps\/plugin-(sql|fs|shell)/.test(source), file);
    if (!file.startsWith(path.join(root, "src", "api") + path.sep)) {
      assert.ok(!/@tauri-apps\//.test(source), `IPC outside api: ${file}`);
    }
  }
});

test("only the local main window can invoke library queries and custom tag edits", async () => {
  const capability = JSON.parse(await readFile(path.join(root, "src-tauri/capabilities/main.json"), "utf8"));
  assert.equal(capability.local, true);
  assert.deepEqual(capability.windows, ["main"]);
  assert.deepEqual(capability.permissions, ["allow-get-app-status", "allow-query-reports", "allow-get-dashboard", "allow-query-library", "allow-get-media-details", "allow-list-library-tags", "allow-mutate-custom-tag", "allow-get-thumbnails", "allow-open-viewer", "allow-close-viewer", "allow-save-playback-state", "allow-prepare-playback", "allow-viewer-navigation", "allow-get-playback-state", "core:event:allow-listen", "core:event:allow-unlisten", "core:window:allow-destroy", "allow-add-like", "allow-get-statistics", "allow-activate-media", "allow-open-player-session", "allow-report-playback", "allow-close-activity", "allow-rebuild-statistics", "allow-statistics-job", "allow-cancel-statistics-rebuild", "allow-create-quick-browse", "allow-get-quick-browse", "allow-quick-browse-items", "allow-select-quick-browse", "allow-shuffle-quick-browse", "allow-close-quick-browse", "allow-get-quick-browse-settings", "allow-save-quick-browse-settings", "allow-toggle-quick-browse-tag", "allow-open-quick-browse-item", "core:window:allow-is-fullscreen", "core:window:allow-set-fullscreen"]);
  assert.equal(capability.remote, undefined);
});

test("asset access is restricted to generated JPEG thumbnails", async () => {
  const config = JSON.parse(await readFile(path.join(root, "src-tauri/tauri.conf.json"), "utf8"));
  assert.deepEqual(config.app.security.assetProtocol, { enable: true, scope: ["$APPLOCALDATA/cache/thumbnails/*.jpg"] });
  for (const csp of [config.app.security.csp, config.app.security.devCsp]) {
    assert.ok(csp.includes("img-src 'self' asset: http://asset.localhost"));
    assert.ok(!csp.includes("file:"));
  }
});
