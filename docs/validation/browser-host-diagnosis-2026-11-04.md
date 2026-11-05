# Phase 1：Chrome host 连接诊断（2026-11-04，多伦多时间）

## 结论

本次按用户 `@Chrome` 指定重试，仍在浏览器连接阶段失败，未取得页面、未进入 URL 自动审批。Phase 1 的浏览器 `desktop_required`、重试及重载专项仍待验收，**Ready for next phase: NO**。

后续调查已定位浏览器报错的直接条件：**运行库未获得完整的浏览器访问授权上下文**，在连接浏览器之前即失败。用户已确认 Chrome 在 Settings > Computer Use 显示 Manage。桌面宿主已启动并提供 pipe，官方 `@oai/sky` 客户端可以枚举应用；无需再次以“未安装扩展”解释当前错误。授权上下文为何未传入该任务仍需宿主侧核实，尚未验证恢复。

## 实际证据

检查始于多伦多时间约 19:22，对应 UTC 2026-11-05 00:22。桌面日志按 UTC 日期保存，本次重点读取 `Logs/2026/11/05/`，而非仅检查 `11/04/` 的旧记录。

| 检查 | 实际结果 |
| --- | --- |
| `cua.getState()` | apps/browsers 均为空；原生错误为 `Computer Use native pipe is unavailable` / `os error 2`；浏览器错误为 `The browser access host is unavailable`。 |
| `cua.createBrowserTab("chrome", "http://127.0.0.1:1420/", {sessionName:"🧪 Phase 1"})` | 同一 host 错误；未创建可访问标签页。 |
| `cua_repl.js_reset` 后重新 `cua.getState()` | 两项错误仍然存在。该重置未恢复通道。 |
| 只读系统 pipe 清单 | 存在 `codex-computer-use-b3d4c5db-ea16-4923-8036-252b7bdea401` 及多个 `codex-browser-use-*` pipe。 |
| 桌面应用启动日志 | UTC `00:21:41.490` browser pipe 开始监听；`00:21:41.504` computer pipe startup ready。`00:22:38`、`00:22:41` 又为会话创建 browser pipe。 |
| MCP 启动日志 | UTC `00:22:39`、`00:22:42` 的相关会话均记录 `cua_repl status=ready`、`error=null`，但这不代表实际 host 调用成功。 |
| 官方 `@oai/sky` 初始化及 `sky.list_apps()` | 成功返回应用清单；仅用于确认原生宿主响应，没有选中或操作浏览器窗口，没有以该通道替代 Chrome 页面验收。 |
| 配置只读核对 | `node_repl` 与 unified-computer-use 插件均启用；当前配置的 computer pipe 与系统清单一致；后端配置包含 `chrome,iab,mcpapps`。配置文件本身不能证明已运行的客户端继承了正确环境。 |
| 进程关系 | 桌面应用于本地 `19:21:35` 重启；另有 `11/02` 启动的 app-server-daemon 存活，并拥有一组 `cua-repl`/`node-repl` 子进程。另有桌面应用自己的工具进程组。当前失败调用属于哪一组、是否使用过期环境，未能确认。 |

宿主日志来源为当前桌面实例的 `codex-desktop-62db53a6-d0b7-4cee-b29a-b003eca29cd2-54020-t0-i1-002135-0.log`。只读提取了 host、pipe、MCP 状态与相关启动行；未复制完整私有应用日志、凭据或浏览历史进入仓库。

## 判断与恢复方向

初轮已排除“所有本机宿主都没有启动”这一解释；后续定位见下一节。日志未提供足以独立判断 Chrome 扩展握手是否成功的直接记录，用户已确认设置界面显示 Manage。

[OpenAI 官方浏览器扩展排障文档](https://learn.chatgpt.com/docs/chrome-extension)建议核对桌面应用 Settings > Computer Use 中 Chrome 是否显示 Manage、使用安装扩展的浏览器 profile，并尝试新会话以清除会话特有的连接状态；持续失败时使用 `/feedback` 附上 chat ID。

本次已验证仅重置 REPL 无效，桌面应用也已实际重启。Manage 状态已由用户确认，后续不重复要求检查该项。没有擅自终止后台 daemon、修改插件配置、替换 pipe、变更权限或重装扩展。

## 用户确认 Manage 后的进一步定位（本地约 19:31 起）

1. 再次 `cua.getState()`，原生 pipe 与 browser host 错误不变。
2. 只读检查本机官方运行库 `@oai/browser-desktop/scripts/browser-service.mjs`。该错误文字对应的守卫在 `BROWSER_USE_ACCESS_PIPE_PATH` 或 `BROWSER_USE_ACCESS_VERIFYING_IDENTITY` 缺失时立即抛出；此时尚未尝试连接浏览器宿主。它不表示 localhost 站点拒绝、Chrome 页面失败或扩展缺失。
3. 插件启动配置通过 `env_vars` 继承 `BROWSER_USE_ACCESS_EXECUTOR_ID`、`BROWSER_USE_ACCESS_PIPE_PATH`、`BROWSER_USE_ACCESS_VERIFYING_IDENTITY`。当前任务 shell 对这三项做了只返回存在性布尔值的检查，三项均不存在；未打印授权数据。shell 环境不等同于 MCP 服务内部环境，但结合运行库守卫，支持本任务启动上下文未完整注入的判断。
4. 当前 shell 父进程链实际为 `powershell.exe → PID 53524 codex.exe`；后者路径属于 `app-server-daemon/releases/0.166.0-x86_64-pc-windows-msvc`，启动时间为本地 11/02 19:55。桌面应用 11/04 19:21 的重启没有替换这个承载本任务的后台服务。不能再把“桌面应用重启过”等同于“当前后台服务也重启过”。
5. 只读查看该已安装版本的 `codex app-server daemon --help`，确认支持 `restart`；没有执行重启。

因此已定位直接失败条件为浏览器授权上下文缺失，后台服务与桌面应用生命周期分离是已证实的环境事实；是否由该 daemon 的环境继承、桌面任务重新附着或其他宿主缺陷造成，仍需恢复试验确认。原生 Computer Use 的 pipe 错误是另一项观察，不能仅由浏览器守卫解释其具体原因。

下一步可对实际后台服务执行受控重启，然后从桌面应用重新建立浏览器任务。官方支持通过 `/daemon` 选择 Restart background server；它可能中断其他会话的活动或排队任务，因此本轮调查未主动重启承载当前及其他会话的服务。参见 [OpenAI 官方后台服务管理说明](https://learn.chatgpt.com/docs/developer-commands)。重启后仍须重新检查授权注入及真实 Chrome 页面；不将重启建议记为已修复。

## 本轮项目检查与边界

- `npm.cmd test`：PASS，30/30，零失败或跳过。
- `npm.cmd run typecheck`：PASS。
- `git diff --check`：PASS。
- 源码 `src/api/app-status.ts` 仍在普通浏览器中以 `isTauri()` 拦截，返回 `desktop_required` 与“请通过桌面应用启动，以连接本地基础服务。”；静态检查不代替页面验收。
- 本轮未重新运行 Rust、production build、桌面 smoke；此前通过结果见 [Phase 1 再次检查](phase1-recheck-2026-11-04.md)及[完整复验](phase12-2026-11-04.md)。
- 只更新验证文档及 PLANS.md；未修改业务代码、Schema、migration、依赖、锁文件，未访问正式数据库或用户媒体，未推进后续业务。
