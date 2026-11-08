# Phase 5：清理、UI 修复与最终验收

本次按用户要求沿用 Phase 1 已通过的隔离 Tauri 桌面验证方法，完成此前未完成的 Phase 5 UI 验收。范围仍为基础 Media Library，不新增 Like Event、播放器或私人媒体操作。

## 清理与失败原因

- 已核实并停止旧项目 Vite PID 39028 与独立测试 Chrome PID 27332。停止前规范化 `.bin/../vite/bin/vite.js` 路径，确认属于本 checkout；未停止日常浏览器或共享 daemon。随后确认 1420 释放，新 Vite 正常监听。
- Phase 1 记录没有证明 browser host/URL 审批问题已经修复。本轮没有通过其他接口读取此前被阻止的 Chrome 页面，而是关闭旧浏览器后使用 Phase 1 的独立 Tauri identifier/appDirectoriesOverride 方式验收。
- 原 `FAIL` 的具体断言已在隔离桌面复现：创建标签后，夹具只等待内存 Map 更新，随后立即查找尚未渲染的 `.tag-option`，导致 `Cannot read properties of undefined (reading 'querySelectorAll')`。改为等待实际 DOM 标签行，并对 rename/delete 同样等待真实 UI 更新，没有删除断言或跳过失败用例。
- 将开发夹具的 mock 从 Tauri internals 改为 API adapter，以兼容原生环境中的只读 internals。生产默认 adapter 仍调用真实 Tauri；fixture 仅由 tests/ui/library.html 导入，未进入 production entry/build。
- 最终关闭两个隔离 Tauri 测试窗口，两个 desktop:dev 命令均 exit 0；窗口清单无测试窗口，1420 查询无监听记录。浏览器 host 未单独重启或宣称修复。

## 最终验证

| 实际命令/检查 | 结果 |
| --- | --- |
| npm.cmd test | PASS，37/37。 |
| npm.cmd run build | PASS，严格 TypeScript（含 tests/**/*.tsx）及 Vite build。 |
| npm.cmd run test:dev | PASS，1/1。 |
| cargo test --locked --offline -- --include-ignored | PASS，45/45，包含全部 4 项真实 FFmpeg 用例。 |
| cargo test --locked --offline --no-default-features -- --include-ignored | PASS，45/45，包含全部 4 项真实 FFmpeg 用例。 |
| cargo clippy --locked --offline --all-targets -- -D warnings | PASS，包含新 fixture example。 |
| cargo fmt --all -- --check | PASS。 |
| git diff --check | PASS，文档更新后再次检查。 |
| tests/ui/library.html?run=1，隔离 Tauri WebView | PASS，11 项 DOM 断言，8 次分页/刷新查询，最大 thumbnail batch=21、page=100。 |
| 111 个合成媒体的真实 SQLite/Tauri IPC smoke | PASS，详见下文。 |

一次在真实桌面应用运行时重跑 cargo test，被 Windows 正在使用的 media-manager.exe 文件锁拒绝。关闭测试应用后原命令再次运行并通过；没有改变权限或跳过测试。

## UI 回归（mock adapter）

实际检查 lazy thumbnail、可见卡片少于一页数量、分页替换数据、滚动后 DOM 有界、详情不读取原件、创建/重命名/删除标签、双媒体批量添加/移除及请求预算。断言执行结果在原生 WebView 可见，截图见 [11 项通过结果](phase5-ui-tests.png)。

该夹具按需生成 100,000 媒体的查询响应，不能将其视为真实 SQLite 性能测试。真实 100,000 行数据库用例遍历 1000 个 keyset 页，无重复或漏项，查询计划使用 cursor index；首次实际热查询记录 p50=0.475ms、p95=0.658ms、末页=0.442ms。本机测试结果不构成跨平台或所有过滤组合的性能承诺。

## 真实链路 smoke

通过 `cargo run --locked --offline --no-default-features --example seed_library_validation`，在 `.tools/validation/phase5-native/` 内生成 110 张合成 JPEG 和 1 个合成视频。使用正式 migration runner、scanner、metadata/thumbnail 服务和 Repository 建库/入库，未手工改正式数据库或伪造 ready。

使用独立 identifier `com.mediamanager.validation.phase5native`，`appDirectoriesOverride` 指向上述 fixture/runtime，运行正式 App、正式 IPC 及正式 asset scope：

- All Media 首/末页显示 100＋11 项，缓存 JPEG 正常显示。
- 根目录自身没有媒体，点击父 System Tag 仍返回后代媒体；展开树后显示 Photos、Videos，选择 Videos 只返回 1 项。
- 视频 DetailsPanel 实际返回 320×180、2.0 秒、MPEG4、10 fps；thumbnail 的画面时间码为 0.200 秒。
- 通过详情的真实 mutation command 为视频添加 Custom Tag，UI 显示已添加、数量为 1；只读查库确认关联已提交。
- fixture 只读复查：media=111、ready=111、三个 Event Log 总数=0、video Custom Tags=1，110 张合成原图 SHA-256 全部与原始 fixture 相同。
- 无 System Tag 目录修改入口、无 Like/Event/播放器交互。

真实窗口截图见 [SQLite/IPC 与视频详情](phase5-native.png)。工具通道成功读取并观察了实际结果；没有将 mock 通过代替该项。

## 重跑与清理

隔离桌面命令使用 `npm.cmd run desktop:dev -- --no-watch --config <validation-config.json> -- --locked --offline`，配置只更改独立 identifier、应用数据目录及测试窗口。mock UI 配置另设 devUrl 到测试 HTML；真实链路配置保留产品 `/` 入口。

环境沿用 Rust/Cargo 1.99.1、Node 24.22.2，FFmpeg/ffprobe 9.0.2 由当前进程 MEDIA_MANAGER_FFMPEG_DIR 提供。原生数据/截图均为合成数据；Unix 条件用例、正式 release 打包、browser host 通道及长期内存性能不在本次通过范围。

结束时先关闭隔离窗口、等 desktop:dev 正常退出，再检查 1420。若仍有监听，只在核实启动命令为本项目 Vite 后停止；不得终止所有 Node/Chrome 进程。
