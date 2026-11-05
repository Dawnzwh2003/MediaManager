# 浏览器预览专项重试（2026-11-04，23:51 UTC 起）

后续检查更新：多伦多时间 2026-11-04 19:04 起（UTC 11-05），原生窗口枚举已恢复；browser host 仍不可用，测试 Chrome 窗口读取被 URL 自动审批终止。详见 [最新 Phase 1 检查日志](phase1-recheck-2026-11-04.md)。以下结果和原因分析保留为本次较早重试的历史记录，不代表最新原生连接状态。

## 结果

**BLOCKED（工具连接不可用），未验收。** 本轮没有读取到真实浏览器页面，不能确认 `desktop_required` 的显示、重试按钮或重载行为。

Phase 1 仍仅剩此专项待验收；此前已通过的真实桌面检查和 Phase 2 Windows 数据层验收保持有效。未进入下一阶段，未改业务代码、Schema、migration、依赖、权限或锁文件。

## 实际重试

| 步骤 | 本次观察 |
| --- | --- |
| 经批准执行 `npm.cmd run dev` | Vite 8.3.5 启动成功，实际绑定 `http://127.0.0.1:1420/`。wildcard 端口提示未阻止绑定。 |
| 浏览器专用入口 `cua.getBrowser({ url: "http://127.0.0.1:1420/" })` | 返回 `The browser access host is unavailable`，未获得浏览器连接。 |
| 初始化 computer-use 并调用 `sky.list_windows()` | 返回 `Computer Use native pipe is unavailable: failed to connect native pipe: 系统找不到指定的文件。 (os error 2)`，未获得窗口列表。 |
| 启动独立 Chrome 测试配置，目标为 localhost | Chrome 启动命令 exit 0；未使用用户日常浏览器配置。由于窗口连接不可用，未确认页面内容。 |
| 重置 node_repl、重新导入 `@oai/sky` 并再次调用 `sky.list_windows()` | 同一 native pipe 错误，恢复尝试无效。未继续输入或操作浏览器。 |
| 清理 | 已停止本轮 Vite 服务器；无法通过工具确认或关闭 Chrome 测试窗口，如有出现可由用户关闭。 |

Chrome 使用此前的独立测试目录 `.tools/validation/phase12-2026-11-04/chrome-profile`，启动目标仍为 `http://127.0.0.1:1420/`。本轮没有打开正式桌面应用或访问用户媒体。

## 与上轮的区别及剩余条件

[上轮完整测试](phase12-2026-11-04.md)在读取 Chrome URL 时被自动审批检查终止。本轮更早停在浏览器连接和原生管道连接阶段，**没有进入 URL 自动审批检查**，不能把本次记录写成同一审批再次拒绝。

本地服务器启动成功只证明服务可启动，不代替浏览器渲染验收。待浏览器入口或 computer-use 原生连接恢复后，仍需实际观察页面显示“请通过桌面应用启动，以连接本地基础服务。”及错误码 `desktop_required`，点击“重试连接”和重载后继续显示该错误，且不显示伪造的服务就绪状态。

本轮仅更新验证文档及阶段状态，未重跑前端/Rust 全套检查，也没有将以前的通过结果记为本轮重新执行。

## 原因分析（后续只读检查）

已确认的断点位于自动化连接层，尚未取得浏览器渲染结果，不能据此认定 Media Manager 页面有缺陷。

1. `The browser access host is unavailable` 出现在获取浏览器连接时，早于页面读取或业务交互，证明该次调用没有可用的浏览器 host 连接。它本身不说明 localhost 被拒绝，也不能单独证明扩展未安装。
2. `native pipe ... (os error 2)` 出现在最基本的 `sky.list_windows()`，此时尚未选择 Chrome。它表示客户端找不到其尝试连接的本地管道端点，并不等于宿主没有任何有效端点。只读进程查询确认宿主仍在运行；后续日志与管道名称核对进一步确认新宿主端点存在，见下文。不能因存在多个 Chromium 子进程就推断版本冲突。
3. 对独立测试 profile 的 Extensions 目录、Preferences 和 Secure Preferences 做了定向读取：存在 Acrobat 及 Chrome 内置组件等注册项，**未发现 ChatGPT 扩展**。此前为了隔离私人浏览器数据使用了独立 profile，却没有先验证该 profile 已建立 ChatGPT 浏览器连接，这是测试准备缺口。它与 Chrome 扩展通道不可用一致，也可能影响此前的可靠 URL 识别，但尚不能证明它就是自动审批拒绝的唯一原因，更不能用它解释所有原生 pipe 故障。

重置 node_repl 只重新创建 JavaScript 客户端，未证明当前任务与新宿主的桥接配置已重新建立，所以相同错误持续出现并不意外。前端服务器可启动、Chrome 启动命令 exit 0，都不等于自动化桥接连接正常。

恢复顺序应先确认桌面应用的 Computer Use/Any App 能力已就绪，再恢复 browser host 或已连接的 Chrome 扩展通道；浏览器重试应使用安装扩展的实际 profile 中的新标签页，或恢复后的内置浏览器。不要继续把未验证连接的独立 profile 当作已连通的浏览器入口。

上述产品设置和连接排障依据 [OpenAI Computer Use 文档](https://learn.chatgpt.com/docs/computer-use)及[浏览器扩展文档](https://learn.chatgpt.com/docs/chrome-extension)。官方文档明确要求使用安装扩展的浏览器 profile，并建议按连接状态检查、重启浏览器/桌面应用、新建会话的顺序排障；未提供本次特定管道错误的确定根因。此追加分析没有修改系统设置、安装扩展或重启用户进程。

### 宿主日志与端点核对

已定位实际宿主为 Windows 包 `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0`。仅对相应应用日志做了定向只读检查，并只枚举 `codex-browser-use-*` / `codex-computer-use-*` 管道名称，没有连接管道或发送自定义协议。

| 时间/检查 | 证据 |
| --- | --- |
| 23:32 UTC 左右 | 先前同日真实桌面测试成功，证明原生自动化曾可用。 |
| 23:47:28.618 UTC | 新宿主实例日志记录 browser-use native pipe listening。 |
| 23:47:28.638 UTC | 同一新实例记录 computer-use native pipe startup ready。 |
| 23:47:58.025 UTC | 新实例再次注册 browser-use 管道。 |
| 23:51 UTC 后 | 本任务获取 browser host 失败，原生客户端报告找不到管道；重置 node_repl 后仍失败。 |
| 随后的只读名称核对 | 日志中 23:47:28 注册的 Computer Use 管道仍存在，对应 Browser Use 管道也存在。 |

因此当前最可能的故障是：**宿主实例切换后，本任务的自动化桥接仍使用旧端点，或任务与新宿主的连接上下文没有同步。** 这比“宿主完全没启动”更符合日志和实际管道状态。但尚未取得客户端实际尝试连接的管道名，不能把旧端点绑定断言为唯一根因，也不能排除其他会话路由问题。

下一次应先在当前 Codex 桌面宿主中新建任务/会话，确认原生窗口枚举及浏览器连接已恢复，再通过已安装并连接 ChatGPT 扩展的实际 Chrome profile 或内置浏览器执行预览。仅反复重启 Vite、切换 localhost 端口或重置 JavaScript 客户端，不会验证或修复宿主会话绑定。独立 profile 缺少扩展的问题也应单独解决，不能把它与原生管道失配混为一个原因。
