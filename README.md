# Media Manager

Tauri + React + TypeScript + Rust + SQLite 项目。当前有 Phase 1 状态页与 Phase 2 数据模型/Repository；没有 scanner、Event Service、媒体查看/播放、标签 UI 或 Quick Browse 业务。React 仍只有 `get_app_status`。

## 环境与安装

- Node.js >=22.18（当前环境为 24.22.2），npm。
- Rust stable（`rust-toolchain.toml`；本轮实际为 1.99.1）及 rustfmt、Clippy。Cargo manifest 的 `rust-version=1.90` 是声明的最低版本，不固定实际编译器；最低版本尚未单独验证。
- Windows：MSVC C++ Build Tools、Windows SDK、WebView2 Runtime。其他平台按 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) 安装系统依赖；当前尚未验证跨平台构建。
- 可访问 npm registry 和 crates.io/Rust 下载站。FFmpeg 不属于本阶段安装或执行范围。

首次安装：

```powershell
npm ci
cargo check
```

当前 Rust stable、MSVC/SDK 和 WebView2 已存在。十个 npm 直接依赖现已安装，`package-lock.json` 已存在；TypeScript 和 Tauri CLI 均为项目本地 devDependencies，无需全局安装。类型检查、前端构建和 Tauri info 已实际通过。

此前的 crates.io 连接失败已定位为执行沙箱的网络限制。经批准在沙箱外下载依赖后，真实 `Cargo.lock` 已生成并核对直接依赖版本；后续使用 `cargo check --locked`、`cargo test --locked`。本轮完整 Rust 测试、Clippy、正式 Tauri release 构建和隔离的真实桌面 IPC/重启/实例锁检查均通过；浏览器预览专项仍受工具限制。PowerShell 若因执行策略拒绝 npm.ps1，使用 `npm.cmd` 执行同名命令；无需修改系统执行策略。

## 开发与检查

在仓库根目录执行：

| 命令 | 用途 |
| --- | --- |
| `npm run desktop:dev` | 启动 Vite 与 Tauri 桌面窗口，检查真实 IPC。 |
| `npm run dev` | 仅前端预览；浏览器中明确提示需桌面服务，不伪造健康数据。 |
| `npm test` | Node 内建测试：DTO 契约、权限边界、真实初始 SQL。无需 npm 依赖。 |
| `npm run test:dev` | 实际 Vite 文件监听回归：监听源码，忽略根目录 Cargo target（需本地开发依赖）。 |
| `npm run typecheck` | TypeScript 严格类型检查（需依赖）。 |
| `npm run build` | 类型检查与 Vite production build。 |
| `cargo check` / `cargo test` | 默认含桌面特性的 Rust 编译检查与测试。 |
| `cargo test --no-default-features` | 不链接 WebView 的后端测试；仍需 Rust 与 SQLite C 编译工具。 |
| `cargo fmt --all -- --check` | Rust 格式检查。 |
| `cargo clippy --all-targets -- -D warnings` | Rust lint。 |
| `npm run tauri -- info` | 查看 Tauri 工具链/依赖状态。 |
| `npm run desktop:build` | Tauri production build，暂不生成安装包。 |
| `npm run icons` | 重建可重复的基础应用图标。 |

桌面 smoke check：2026-11-04 已在独立测试标识和工作区测试目录下确认状态页就绪及 schema=2、刷新/重启/WebView 重载不重建数据库、第二实例返回 busy、锁释放后重试恢复，见 [桌面证据](docs/validation/phase12-2026-11-04.md)。浏览器 desktop_required 专项随后重新尝试，当前被浏览器 host 与 computer-use 原生 pipe 连接错误阻塞，尚未进入 URL 自动审批，见 [专项重试](docs/validation/browser-preview-2026-11-04.md)。

## 目录与通信

```text
src/app/                 React 状态页
src/api/                 唯一 Tauri invoke / DTO / 错误处理入口
src-tauri/src/commands/   get_app_status（唯一 IPC command）
src-tauri/src/domain/     状态 DTO
src-tauri/src/services/   状态用例与惰性初始化
src-tauri/src/platform/   安全路径创建
src-tauri/src/db/         连接、单实例锁、migration runner
src-tauri/src/db/repositories/  媒体、标签、只读事件、快照、设置
src-tauri/src/domain/library.rs  持久化模型
src-tauri/migrations/     v1 应用元数据 + v2 数据模型
src-tauri/tests/          后端集成测试
tests/frontend/          TypeScript DTO 契约测试
tests/architecture/      IPC 与权限边界测试
tests/database/          实际 migration SQL 测试
docs/reviews/            历史阶段审查
docs/validation/         验证命令与结果
archive/scaffolds/       保留的默认模板，不参与正式构建
```

仓库根目录是唯一正式工程。五份主规范保留在根目录，详细文档入口见 [文档索引](docs/README.md)。原 `MediaManager/` 默认模板完整移动到 [归档目录](archive/README.md)，35 个文件逐一校验 SHA-256 一致；没有合并其 greet/opener 示例。依赖与生成文件保留在忽略的 node_modules/、target/、dist/、.tools/ 中。

React → `src/api/app-status.ts` → `get_app_status` → 阻塞 worker → Rust 状态服务 → SQLite → camelCase DTO。UI 先显示连接状态，目录/数据库首次初始化在 worker 执行；失败不缓存伪造结果，可在修复环境后重试。前端没有数据库路径、SQL、文件系统或 shell API。

运行目录以 Tauri `app_local_data_dir` 为根（Windows 通常 `%LOCALAPPDATA%/com.mediamanager.desktop`）：

```text
<app-root>/
├── data/
│   ├── library.sqlite3
│   ├── instance.lock
│   └── backups/          仅既有数据库升级前创建
└── cache/
    └── thumbnails/
```

不存在时创建；已有目录和文件保留，受管理路径若为文件/符号链接/Windows reparse point 则拒绝。不使用工作目录或媒体目录保存应用数据。Phase 1 不创建 playback cache，不执行缓存清理。

## Migration 约定

- 应用 lifetime 文件锁排除第二个写实例；SQLite 同时显式设置 foreign_keys、WAL、FULL、2000ms busy_timeout。
- 先只读校验迁移历史，再改变持久 PRAGMA。未知较新版本、checksum/name/gap 异常、无记录的既有库和损坏文件均拒绝，不清空/重建。
- `schema_migrations` 为 runner 自举表；每份编号 SQL 与对应记录同一事务。SQL 失败由事务回滚，下一次从最后已提交版本继续。
- 既有库升级前通过 SQLite backup API 保存包含 WAL 数据的一致性备份，成功后才迁移。备份失败不继续升级，回退使用备份，不承诺自动降级。
- 已应用 SQL 不修改。新增 migration 必须加入 runner 的静态清单；名称、SHA-256 与应用时间记录在库中。`.gitattributes` 固定 LF，避免换行改变校验和。
- 当前 schema=2，v1 ledger/app_metadata 保留；新增 media、两类 tags 与关联、三个事件表、quick_browse_sessions、app_settings，以及版本/位置/播放状态/快照成员辅助表。源 SQL 为 `0002_library_model.sql`，没有修改 v1。

## Phase 2 数据层

`Database::repositories()` 提供 Media、CustomTag、SystemTag、只读 Event、QuickBrowse 和 Settings Repository；均为 Rust 存储接口，不注册到 Tauri。分页默认建议 100、上限 200，媒体用 keyset，不加载全库。media 有完整指定字段，ID 独立于 path/fingerprint。

Custom Tag 平面多对多，可 CRUD；名称 trim 后按 SQLite NOCASE 唯一（只对 ASCII 大小写折叠）。System Tag 只持久化未来 scanner 的真实目录观察，检查直接物理父目录；本阶段不验证磁盘或遍历目录。删除标签级联关联行，删除有事件/快照的媒体被 FK 拒绝；Repository 无媒体硬删除入口。

事件 INSERT 的 SQLite trigger 原子维护 Counter Cache，未来 Event Service 不得再手工累加。本轮没有事件写 Repository、资格服务或重建服务。独立 playback_states 更新 media.playback_position 投影但不产生 Play。QuickBrowse Repository 只存储给定序号与 seed，不生成顺序或激活媒体。

## 当前验证状态

最新一次 Phase 1 复查（多伦多时间 2026-11-04 19:04 起）重新通过 Node 30/30、typecheck/build、cargo fmt/check、两种 Rust 配置各 24/24 和 Clippy。原生窗口枚举已恢复；browser host 仍不可用，读取测试 Chrome 窗口时被 URL 自动审批终止，Phase 1 仍仅浏览器专项待验收。该轮未重跑桌面 smoke、release 构建、开发监听或 Tauri info，详见 [本轮检查日志](docs/validation/phase1-recheck-2026-11-04.md)。

2026-11-04 复验通过：Node 测试 30/30、开发监听回归 1/1、frontend typecheck/build、cargo fmt/check、默认桌面及无默认特性 Rust 测试各 24/24、Clippy、Tauri info、正式 release 构建及隔离的真实桌面 IPC/持久化/实例锁检查。正式发布产物为 `target/release/media-manager.exe`；窗口测试使用 dev 配置，未启动正式标识的应用。另有 2 个 Unix 专用 Rust 测试未在 Windows 执行。

此前全套结果见 [Phase 1/2 测试与桌面证据](docs/validation/phase12-2026-11-04.md)，最新浏览器结果见 [再次检查日志](docs/validation/phase1-recheck-2026-11-04.md)。Phase 2 当前授权数据层已在 Windows 上验收；Phase 1 仅浏览器预览专项待补，未开始 scanner、Event Service 或下一阶段。
