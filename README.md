# Media Manager

Tauri、React、TypeScript、Rust、SQLite 和 FFmpeg 构建的本地媒体库。支持标签、搜索/组合筛选/排序、稳定 Random、只读查看器、独立播放进度、Quick Browse、History、Continue Watching 和 Dashboard。

**Phase 12：Folder Import。** 现在可在桌面 UI 选择外部文件夹，预览后复制媒体至托管 data 存储，并建立只读来源标签。此前 [Release Review](docs/reviews/RELEASE_REVIEW_2026-11-13.md) 保留为历史记录；干净系统安装、签名和工具分发仍是正式发布限制。

## Installation

当前可构建 Windows x64 EXE 和 NSIS 测试安装包，尚未批准正式发布。运行需要 WebView2 Runtime；开发环境已有 WebView2，干净系统的依赖安装、安装/升级/卸载尚待隔离环境验收。EXE 和安装包未签名。

运行从本仓库构建的 EXE 时，启动 `target/release/media-manager.exe`。应用使用当前 Windows 用户的数据目录，首次启动自动创建 schema 12 空库，不附带示例或私人媒体。现有库会校验版本并按迁移升级；较新、无记录或校验不符的库会报错并保留。

## Folder Import

1. 在媒体库点击 **Import Folder → 选择外部文件夹**，通过系统原生选择器选择本地目录。
2. 等待递归扫描和完整内容校验，查看文件、新媒体、已存在媒体、重复、标签、冲突和所需空间预览，再点击“开始导入”。
3. 查看已处理/总数、当前文件和失败列表；**Cancel** 保留已完成项，**Resume / Retry** 恢复未完成项。任务可在后台继续，重启后可显式恢复中断任务。预览阶段失败项需重新预览，恢复时会回到预览确认。

源文件保留，不移动、不删除。同名（精确大小写）同完整内容复用稳定 ID 与所有历史，只增加来源关系；同名不同内容、异名同内容各自存储。来源目录保存为 **Source Tags**，支持树形浏览和含子目录筛选；物理 **System Tags** 与平面 **Custom Tags** 保持独立。来源根仅作分组，根内文件仍记录原始相对路径。

**Settings → 选择存储文件夹** 可配置后续新文件的媒体根。默认 `<app-local-data>/data/media/<media_id>/<原文件名>`，不依赖安装目录。更改设置不搬迁既有媒体或数据库，既有根继续使用；请保持这些磁盘可访问。来源与所有托管根不能相同或互相包含。链接/junction 不跟随，目录深度受实际文件系统限制。完整 Hash 校验会读取文件全部内容，大视频预览可能耗时。

UI 展示最近任务和前 20 个错误，暂不提供任务历史管理或自动清理。取消不删除已完成媒体；复制完成而数据库尚未发布时，文件保留为有预留记录的未完成项，使用 Resume / Retry 恢复。详细测试和边界见 [Folder Import 验收](docs/validation/folder-import-validation.md)。

普通用户运行已构建程序不需要 Node.js、Rust 或 MSVC；FFmpeg 按下文单独配置。目标平台目前仅在本机 Windows x64 验证，其他平台及最低支持系统版本尚未验证。

## Development

开发环境：

- Node.js >=22.18，当前验证为 24.22.2；使用项目本地 npm 工具。
- Rust stable，当前验证为 rustc/cargo 1.99.1，安装 rustfmt 和 Clippy。`rust-version=1.90` 是声明的最低版本，本轮未单独验证该最低版本。
- Windows MSVC C++ Build Tools、Windows SDK、WebView2 Runtime。
- 首次依赖安装可访问 npm registry 和 crates.io；保留真实 `package-lock.json` 和 `Cargo.lock`。

在仓库根目录执行：

```powershell
npm.cmd ci
cargo check --locked
npm.cmd run desktop:dev
```

`npm.cmd run dev` 只提供浏览器预览；媒体数据库与原件访问需要 Tauri 桌面服务。若 PowerShell 拒绝 npm.ps1，使用 npm.cmd，无需更改系统执行策略。

自动检查：

```powershell
npm.cmd test
npm.cmd run test:dev
npm.cmd run build
cargo test --locked -- --include-ignored --nocapture
cargo test --locked --no-default-features -- --include-ignored --nocapture
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --no-default-features --all-targets -- -D warnings
```

运行前配置下文 FFmpeg；`--include-ignored` 会执行真实工具用例。缓存完整时可追加 Cargo `--offline`，不能以此代替第一次安装。测试使用临时数据库和合成媒体，规模夹具位于忽略的 `.tools/validation/`，不应指向用户媒体库。

## Production Build

```powershell
npm.cmd run desktop:build -- -- --locked
```

该命令执行严格 TypeScript/Vite production build 和 Tauri Release 构建，产物为 `target/release/media-manager.exe`。脚本显式使用 `--no-bundle`，仓库默认 `bundle.active=false`。

可用独立配置验证 NSIS 打包，不修改正式配置：

```powershell
New-Item -ItemType Directory -Force .tools | Out-Null
@{bundle=@{active=$true;targets=@('nsis')}} | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 .tools/release-bundle.json
npm.cmd run tauri -- build --ci --bundles nsis --config .tools/release-bundle.json -- --locked
```

本轮实际生成 `target/release/bundle/nsis/Media Manager_0.1.0_x64-setup.exe`。打包器可能下载 NSIS 工具和 WebView2 引导程序；Cargo 离线参数不保证打包阶段无需网络。构建成功与干净系统安装验收分别记录，签名和分发策略尚未完成。测试覆盖目录/标识的配置仅用于隔离验证，不应作为正式数据位置发布。

## FFmpeg

图片/视频 metadata、缩略图及 Remux/Proxy 依赖 **ffmpeg 和 ffprobe 两个程序**。可放入 PATH，或在启动应用的进程环境中设置：

```powershell
$env:MEDIA_MANAGER_FFMPEG_DIR='C:/Tools/ffmpeg/bin'
& './target/release/media-manager.exe'
```

双击运行需要应用继承相应用户环境变量；配置后重新启动应用。不要把 bin 目录写成单个 exe 的路径。本轮验证的外部工具为 9.0.2；其他版本、编解码器和 WebView 支持须实际验证。

当前 EXE/NSIS 不捆绑 FFmpeg。缺少工具时应用、库查询和 Direct 路径仍可使用；需要处理的条目记录 `ffmpeg_missing`，转换不可用。原件不会被修改。工具修复后，失败条目需要显式 retry；现有 UI 未提供独立缩略图重试入口，后端重试已有测试。

验证环境中的 FFmpeg 构建自带 LICENSE，声明 GPL v3。正式分发前需要确定工具来源、分发方式及相应许可材料；仓库尚未完成该分发审查，也未替产品选择新的许可证。

## data/、cache/ 与 Database location

正式应用以 Tauri `app_local_data_dir` 为根，Windows 通常为：

```text
%LOCALAPPDATA%/com.mediamanager.desktop/
├── data/
│   ├── library.sqlite3
│   ├── library.sqlite3-wal    # SQLite 运行期间可能存在
│   ├── library.sqlite3-shm
│   ├── instance.lock
│   ├── media/               # 默认托管副本；Settings 可改后续新文件目标
│   └── backups/             # 既有库升级前的一致性备份
├── cache/
│   ├── thumbnails/          # 可再生 JPEG
│   ├── playback/            # Remux/Proxy，按内容版本隔离
│   └── browse/              # 临时 Quick Browse spool
└── EBWebView/               # WebView2 profile，视运行环境而定
```

`data/` 保存数据库、媒体托管副本、来源关系、任务与设置，是需要备份的用户数据。自定义媒体根可以在 data/ 之外，也必须单独备份。`cache/` 保存可再生派生资源；导入的外部原件仍在用户原目录。应用不会把工作目录当作正式数据目录。

当前 schema=12。应用使用 WAL/FULL、foreign_keys 和 2 秒 SQLite busy_timeout；文件锁限制同一数据目录的写实例。普通启动不重建全库，未完成导入等待显式恢复。不要删除 data/ 来修复缩略图问题。

Random 的 seed 还保存在 WebView 本地存储；保留 data/ 可以保留固定 Quick Browse 会话，若需要保留独立 Library Random 偏好，还应在应用完全退出后备份整个应用根目录。

## Backup and restore

升级旧库前，程序通过 SQLite backup API 在 `data/backups/` 创建一致性备份，包含已提交 WAL 数据。备份失败则不继续升级；migration 和 ledger 每步同事务，失败可从最后已提交版本重试。已有 SQL 的名称/校验和不能改写。相同 schema 的普通重启不自动生成新备份。

手工备份：

1. 完全退出应用，确认没有同目录后台进程。
2. 将整个 data/ 复制到独立备份位置；需要偏好时复制整个应用根目录。保留当时存在的 SQLite sidecar，不只复制运行中的主库。
3. 备份所有自定义媒体根和需要保留的外部原件。仅有 library.sqlite3 或 migration 的数据库备份不包含托管副本，缓存不能替代媒体；恢复时还需保持注册根的路径可访问。

恢复：关闭应用，先完整保留当前 data/。将选定备份恢复到一个新的干净 data/，主库文件名使用 `library.sqlite3`，再启动兼容版本。不要把另一份数据库的旧 WAL/SHM 与恢复的主库混用。旧版本恢复点会由正式 migration 升级；不承诺自动降级。

迁移测试覆盖 v1–v11 到 v12、升级前备份重开、失败回滚及旧库恢复后再迁移。v12 只追加导入表/索引，不复制或重建既有库。计数重建只修复从 Event Log 可推导的 Counter Cache，不能恢复已丢失的日志或修复损坏的 SQLite 页面、外键事实或迁移历史。

## Troubleshooting

| 现象 | 检查与处理 |
| --- | --- |
| 新安装后媒体库为空 | 使用 Import Folder 选择来源，等待预览并开始导入；刷新仅重新查询 |
| 导入空间不足/复制失败 | 检查目标磁盘空间和权限，修复后 Resume / Retry；单文件失败不取消其他文件。已有副本和外部原件保留 |
| 来源离线/预览后文件变化 | 恢复来源访问后重试；内容变化请重新选择文件夹生成预览，不沿用旧内容判断 |
| 导入中意外退出 | 重启后 Import Folder 中 Resume；只核验有持久发布记录的自有文件，不覆盖已有文件。不保证断电时尚未持久的内容 |
| busy / database locked | 退出另一个使用同一数据目录的实例，等待短写事务后重试。instance.lock 是生命周期锁文件，不以删文件代替关闭进程 |
| migration / database error | 保留 data/ 和 backups/，检查磁盘空间、权限和应用版本；先备份再诊断，不删除数据库或改 ledger 以跳过 migration |
| 计数与日志不一致 | 使用媒体库页脚“重建统计”，等任务完成。重建保持日志；无法修复日志本身的损坏 |
| 原件 missing / 磁盘离线 | 连接原磁盘并核对路径/权限。后端扫描对高置信度移动保留关联；当前 UI 的刷新不发起重新扫描 |
| 移动后显示为新媒体 | 不可靠身份匹配采用旧 missing + 新 ID；旧标签/历史/进度仍保留在旧实例。Windows 覆盖移动可能改变创建时间，触发这条保守规则。当前没有手工 relink |
| 缩略图不可用 | 检查 ffmpeg/ffprobe、源文件和缓存权限；损坏/缺失缓存请求时可重新排队，生成失败需后端显式重试。完整 JPEG 熵数据损坏不一定被轻量结构校验发现，UI 会显示读取失败占位 |
| 视频无法播放 | 检查 WebView 编解码支持和 FFmpeg；应用依次尝试 Direct/Remux/Proxy。格式白名单及不支持的 codec 会逐项报错，不覆盖原件 |
| 刷新提示 stale_cursor | 扫描、标签或事件已改变结果代次，重新加载首页，避免混用旧分页 |
| 搜索不到 Unicode 变体 | 中文字面匹配及 Unicode 文件访问已测试；SQLite NOCASE 仅处理 ASCII，不提供完整 Unicode 大小写折叠/规范化 |
| 大库/重建较慢 | 使用 SQLite 分页和虚拟列表。历史每页仍需聚合事件；百万 View 及耐久写入的尾延迟见 Phase 11 记录 |
| 程序意外退出 | 重启后后台恢复已有工作，进度恢复到最后持久化 checkpoint；瞬间断电前尚未提交的进度不能保证保存 |

## Data safety and project layout

React 仅通过 `src/api/` 的类型化 IPC 调用 Rust，没有任意文件系统、SQL 或 shell 权限。原生选择器也在 Rust 用例内调用，未授予通用 dialog/fs 权限。Rust 原件只读；物理删除仅用于应用缓存/spool、自有未完成 part 和写权限探针，未提供删除完成媒体的入口。导入不创建 Like/View/Play，不清零计数或播放状态。这不是对任意本地恶意进程或损坏硬件的保证。

`src/` 是正式前端；`src-tauri/src/` 是业务服务、仓储与资源访问；`src-tauri/migrations/` 保存追加式迁移；`src-tauri/tests/` 与 `tests/` 保存自动和桌面回归。`archive/` 为不参与构建的模板归档，`.tools/`、target/、node_modules/、dist/ 是忽略的验证/生成目录。

当前需求、架构、计划、决策分别见 REQUIREMENTS.md、ARCHITECTURE.md、PLANS.md、DECISIONS.md。[文档索引](docs/README.md) 保留历史阶段结果；[Phase 11](docs/validation/phase11-validation.md) 的开发构建测量不替代本轮生产构建验证。
