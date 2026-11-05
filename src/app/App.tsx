import { useEffect, useState } from "react";
import { getAppStatus } from "../api/app-status";
import { toApiError } from "../api/contracts.ts";
import type { ApiError, AppStatus } from "../api/contracts.ts";

type State =
  | { kind: "loading" }
  | { kind: "ready"; status: AppStatus }
  | { kind: "error"; error: ApiError };

export function App() {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    setState({ kind: "loading" });
    getAppStatus().then(
      (status) => { if (active) setState({ kind: "ready", status }); },
      (error: unknown) => { if (active) setState({ kind: "error", error: toApiError(error) }); },
    );
    return () => { active = false; };
  }, [attempt]);

  return (
    <main className="shell">
      <header>
        <span className="eyebrow">PHASE 1 · FOUNDATION</span>
        <h1>Media Manager</h1>
        <p className="intro">本地图片与视频管理，从可靠的基础开始。</p>
      </header>
      <section className="status-card" aria-labelledby="status-heading" aria-live="polite" aria-busy={state.kind === "loading"}>
        <h2 id="status-heading">应用状态</h2>
        {state.kind === "loading" && <p role="status">正在连接本地基础服务…</p>}
        {state.kind === "error" && (
          <div role="alert">
            <p>{state.error.message}</p>
            <small>错误代码：{state.error.code}</small>
          </div>
        )}
        {state.kind === "ready" && (
          <>
            <p className="ready">本地基础服务已就绪</p>
            <dl>
              <div><dt>应用版本</dt><dd>{state.status.appVersion}</dd></div>
              <div><dt>数据库版本</dt><dd>{state.status.database.schemaVersion}</dd></div>
              <div><dt>SQLite</dt><dd>{state.status.database.sqliteVersion}</dd></div>
              <div><dt>运行目录</dt><dd>{Object.values(state.status.storage).every(Boolean) ? "已就绪" : "异常"}</dd></div>
            </dl>
          </>
        )}
        <button
          type="button"
          disabled={state.kind === "loading"}
          onClick={() => setAttempt((value) => value + 1)}
        >
          {state.kind === "error" ? "重试连接" : "刷新状态"}
        </button>
      </section>
      <footer>应用骨架 · 媒体管理功能将在后续阶段加入</footer>
    </main>
  );
}
