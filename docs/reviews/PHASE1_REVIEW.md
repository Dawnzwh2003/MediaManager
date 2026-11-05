# Phase 1 Code Review 与验证记录

最新复查见 [Phase 1 再次检查](../validation/phase1-recheck-2026-11-04.md)（多伦多时间 2026-11-04 19:04 起）：Node 30/30、类型检查/前端构建、Rust 两种配置各 24/24、fmt/check/Clippy 重新通过；原生窗口枚举恢复，但 browser host 不可用，测试 Chrome 窗口读取被 URL 自动审批终止。Phase 1 仍仅浏览器专项待验收，本轮未重跑桌面 smoke 或 release 构建。

最新验证见 [2026-11-04 Phase 1/2 测试](../validation/phase12-2026-11-04.md)：自动检查、正式构建与真实桌面 IPC/持久化/实例锁均通过，仅浏览器预览专项仍受工具阻塞。下文保留原审查时的状态与发现，不代表当前工具链缺失。

其后 [浏览器单项重试](../validation/browser-preview-2026-11-04.md)仍未读取到页面：浏览器 host 和原生 pipe 连接不可用，重置运行时无效。Phase 1 状态未改变。

日期：2026-11-01。审查范围为当前工作树的 Phase 1 新增文件及四份更新文档。本文记录静态审查与实际可执行测试，不代替原生编译或真实桌面 IPC 测试。

## 结论

最小骨架已编写；**Phase 1 尚未验收**。本机没有 Rust/Cargo、MSVC C++ Build Tools、Windows SDK；npm registry 网络请求被系统以 EACCES 拒绝，无法安装 React/Vite/TypeScript/Tauri CLI。原生构建、前端类型检查和真实通信链路仍是阻塞的验证项。不能宣称应用已成功启动。

阶段边界遵守最新授权：只有 app status 用例和数据库基础设施，没有 scanner、缩略图生成、图片/视频查看、标签、Like/View/Play、Quick Browse，也没有安装或调用 FFmpeg。Phase 2 未开始。

## 已修复的审查发现

| 级别 | 问题 | 修复与回归覆盖 |
| --- | --- | --- |
| Important | 在检查较新/不兼容库前设置 WAL 会改动其持久状态。 | 增加只读历史验证，在 WAL 前拒绝；新增字节级保留回归测试，尚待 cargo test。 |
| Important | rusqlite 0.41 的备份 API 已改为 Name/泛型 progress，旧 DatabaseName/None 调用不适用。 | 对照所选版本官方源码改为 MAIN_DB 和 NO_PROGRESS；升级备份测试尚待执行。 |
| Important | Windows 文件锁冲突可能不映射为 WouldBlock。 | 同时比较 fs2 的原生 lock_contended_error；第二实例/释放锁测试覆盖，尚待执行。 |
| Important | SQLite sidecar 若已为链接，可能绕过主数据库文件检查。 | 同时验证 -wal/-shm/-journal；Unix symlink 回归已编写，Windows 检查包含 reparse point。 |
| Important | LIKE 'sqlite_%' 中下划线是通配符，会漏掉不属于 SQLite 保留前缀的既有对象。 | 使用 NOT GLOB 'sqlite_*'，检测所有用户 Schema 对象；测试使用 sqliteXuser，尚待执行 Rust runner。 |
| Minor | 不同平台换行可能造成迁移 checksum 漂移。 | 添加 .gitattributes 固定 LF；已应用 SQL 不可修改。 |

## 边界与恢复检查

- React 只从 src/api 调用 invoke；唯一 command 为 get_app_status，权限只授予本地 main 窗口。没有 SQL/fs/shell 插件、任意路径参数或业务写入口。
- AppState 只管基础状态，commands/domain/services/platform/db 分层；SQLite 连接和首次路径初始化在 spawn_blocking 内执行，Mutex 保护惰性初始化及连接。失败不会缓存成功或给浏览器伪造响应。
- 运行目录只在应用 local-data 根下创建；幂等、不覆盖文件、拒绝受管目录链接/reparse point。后台状态响应只给 ready 标志，不返回个人路径。
- 文件锁持有到连接关闭；先验证历史再配置持久 PRAGMA。正常重开只校验已应用迁移，不重建、不重复写元数据、不为无升级重开做备份。
- 编号 SQL 与 ledger 记录同事务，失败 RAII 回滚；升级前一致性 backup，拒绝新版本、checksum/name/gap 异常、未知 Schema 和损坏库，无删除/重建 fallback。
- IPC 错误仅序列化稳定 code/message/retryable，原生错误/路径/SQL 不直接传前端。未知前端异常也用通用消息。
- 直接依赖精确版本已有；完整 lockfiles 未生成，不能宣称完整可重现依赖解析。未来增加业务表必须新 migration，不临时改结构。

## 实际验证

环境：Windows x64、Node 24.22.2、npm 11.19.1、WebView2 已存在；未找到 Rust/MSVC/Windows SDK。

| 命令 | 实际结果 |
| --- | --- |
| node scripts/generate-icons.mjs | 成功，生成 icon.png/icon.ico。 |
| npm test | 6 项通过、0 失败：3 项 DTO/错误契约、2 项权限/依赖边界、1 项实际初始 SQL。Node 自带 SQLite 不是 Rust bundled SQLite，不能替代其测试。 |
| npm install --no-audit --no-fund | EACCES，无法连接 registry.npmjs.org；未完成依赖解析。 |
| cargo check / cargo test | cargo 不存在，未进入编译。已有 13 项 Windows 适用 Rust 测试和 2 项 Unix 专用测试，均未执行。 |
| cargo fmt --all -- --check | cargo 不存在，尚未通过 rustfmt 检查。 |
| cargo clippy --all-targets -- -D warnings | cargo 不存在，尚未通过 lint。 |
| npm run typecheck / npm run build | tsc 未安装，未完成类型检查或 Vite 构建。 |
| npm run tauri -- info / npm run desktop:build | tauri CLI 未安装，未完成原生构建。 |
| git diff --check | 通过；仅 whitespace 检查，不是编译或功能验证。 |

## 待关闭的验收项

1. 补齐 Rust/MSVC/SDK 与依赖下载权限，成功 npm install/cargo resolve 后生成并提交真实 package-lock.json、Cargo.lock。
2. 运行 rustfmt、cargo check/test、Clippy、前端 typecheck/build、Tauri info/build，修复所有实际错误；无桌面后端测试只是辅助，不能替代默认桌面特性检查。
3. 运行桌面 smoke：React → get_app_status → Rust → SQLite → 状态页；刷新、重启、第二实例 busy、初始化失败后重试；验证 data/cache/thumbnails 在预期根，未创建业务数据或修改媒体。
4. 验证迁移中断恢复/一致性备份和 Windows 路径/锁行为。上述未关闭前保持 Phase 1 未验收；不得自动进入 Phase 2。

## 新增文件（42）

```text
.editorconfig
.gitattributes
.gitignore
.npmrc
Cargo.toml
README.md
index.html
package.json
rust-toolchain.toml
tsconfig.json
vite.config.ts
docs/PHASE1_REVIEW.md
scripts/generate-icons.mjs
src/main.tsx
src/api/app-status.ts
src/api/contracts.ts
src/app/App.tsx
src/app/styles.css
src-tauri/Cargo.toml
src-tauri/build.rs
src-tauri/tauri.conf.json
src-tauri/capabilities/main.json
src-tauri/icons/icon.ico
src-tauri/icons/icon.png
src-tauri/migrations/0001_app_metadata.sql
src-tauri/src/lib.rs
src-tauri/src/main.rs
src-tauri/src/error.rs
src-tauri/src/commands/mod.rs
src-tauri/src/domain/mod.rs
src-tauri/src/domain/app_status.rs
src-tauri/src/platform/mod.rs
src-tauri/src/platform/paths.rs
src-tauri/src/services/mod.rs
src-tauri/src/services/app_status.rs
src-tauri/src/db/mod.rs
src-tauri/src/db/migrations.rs
src-tauri/src/db/migrations/tests.rs
src-tauri/tests/foundation.rs
tests/boundaries.test.mjs
tests/contracts.test.ts
tests/schema.test.mjs
```

## 修改文件（4）

- REQUIREMENTS.md：将当前授权改为 Phase 1，明确业务排除范围与最小 Schema。
- ARCHITECTURE.md：区分总体设计与实际基础模块，记录真实运行路径、连接/初始化与验证限制。
- PLANS.md：Phase 1 已编写但环境阻塞、未验收；记录测试及剩余验收，Phase 2 未开始。
- DECISIONS.md：D018–D020 记录范围收窄、存储生命周期和依赖/验证决策；历史禁止实施指令由最新授权取代。

AGENTS.md 已重新阅读且遵守，本轮无需修改长期规则。
