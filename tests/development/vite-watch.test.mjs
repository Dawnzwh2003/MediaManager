import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { createServer } from "vite";

test("Vite watches source changes but ignores workspace Cargo build artifacts", { timeout: 15_000 }, async () => {
  const root = await mkdtemp(path.join(tmpdir(), "media-manager-vite-watch-"));
  let server;
  try {
    await mkdir(path.join(root, "src"));
    await mkdir(path.join(root, "target", "debug"), { recursive: true });
    const source = path.join(root, "src", "probe.js");
    const artifact = path.join(root, "target", "debug", "generated.html");
    await writeFile(source, "export const value = 0;\n");
    await writeFile(artifact, "initial synthetic build artifact\n");
    server = await createServer({
      configFile: fileURLToPath(new URL("../../vite.config.ts", import.meta.url)),
      root,
      cacheDir: path.join(root, "node_modules", ".vite"),
      clearScreen: false,
      logLevel: "silent",
      server: { port: 0, strictPort: false, hmr: false },
      optimizeDeps: { noDiscovery: true, include: [] },
    });
    // Wait for the actual watcher to finish its initial crawl before mutating files.
    const watcherReady = new Promise((resolve) => server.watcher.once("ready", resolve));
    await server.listen();
    await watcherReady;
    const targetDirectory = path.join(root, "target");
    assert.equal(Object.keys(server.watcher.getWatched()).some((directory) =>
      directory === targetDirectory || directory.startsWith(`${targetDirectory}${path.sep}`)),
    false, "Cargo output directory must not be watched");
    const events = [];
    server.watcher.on("all", (event, file) => events.push({ event, file: path.resolve(file) }));
    await writeFile(artifact, "updated synthetic build artifact\n");
    await writeFile(source, "export const value = 1;\n");
    for (let attempt = 0; attempt < 50 && !events.some(({ file }) => file === source); attempt += 1) {
      await delay(100);
    }
    assert.ok(events.some(({ file }) => file === source), "source change must reach the watcher");
    await delay(250);
    assert.equal(events.some(({ file }) => file === artifact), false, "Cargo artifact must be ignored");
  } finally {
    if (server) await server.close();
    assert.equal(path.dirname(root), path.resolve(tmpdir()));
    assert.ok(path.basename(root).startsWith("media-manager-vite-watch-"));
    await rm(root, { recursive: true, force: true });
  }
});
