# 分阶段开发计划

## 当前状态（Phase 8）

Phase 8 完整查询已通过 Windows 验收。三类搜索、组合筛选、15 种排序和稳定 Random 已验证。前端 48/48，两种 Rust 配置各 70/70，桌面 10 项检查通过。测试窗口、1420 服务和截图会话已清理，本轮停止。详见 [Phase 8 验证记录](docs/validation/phase8-validation.md)。

## Phase 4 验收历史（2026-11-08）

用户本次授权的 **Phase 4 = metadata 与 thumbnail cache 已完成**，替代旧路线的同名编号范围。全套适用测试通过，结果见下方 Phase 4；本轮到此停止，不推进播放或浏览 UI。此前 Phase 1 浏览器专项的限制保留，不把历史记录视为本轮阻塞或实施范围。

## 历史状态与门槛（Phase 1/2）

Phase 0 已将用户批准规则写入设计并通过 15 项 architecture review：**Ready for Phase 1: YES**。用户现明确授权 Phase 2：仅数据模型与数据库层，不包含 scanner、Event Service 或 UI 业务。

最新 Phase 1 复查（多伦多时间 2026-11-04 20:00）通过 Node 30/30、Vite 监听回归 1/1、TypeScript/Vite 构建、Rust 格式与编译检查、两种配置各 24/24 及 Clippy。本轮浏览器工具报告 Work 设置已禁用本地浏览器访问，未能观察预览页面。**Phase 1 仍待浏览器专项验收。Ready for next phase: NO。** 详见 [Phase 1 检查日志](docs/validation/phase1-recheck-2026-11-04.md#最新复查2026-11-04-2000-多伦多时间)。

基础复查（多伦多时间 2026-11-04 19:04 起，对应 UTC 11-05）已通过 Node 30/30、typecheck/build、cargo fmt/check、两种配置 Rust 测试各 24/24 及 Clippy。随后 `@Chrome` 重试仍报 host 不可用。最新 host 调查（本地 19:22 起）确认桌面宿主 pipe 已启动、官方 sky 客户端可枚举应用，但 `cua_repl` 重置后仍失败；故障缩小到工具连接或会话绑定，具体失效点尚未确认，详见 [host 连接诊断](docs/validation/browser-host-diagnosis-2026-11-04.md)。本轮 Node 30/30 与 typecheck 再次通过，没有取得页面或进入 URL 自动审批。**Phase 1：仅浏览器 desktop_required 专项受工具阻塞，整体待验收；Phase 2：此前当前授权数据层在 Windows 上验收通过。Ready for next phase: NO。** 本轮未重跑 Rust、开发监听、正式 release 构建或桌面 smoke；历次记录见 [Phase 1 检查日志](docs/validation/phase1-recheck-2026-11-04.md)，此前真实 IPC、刷新、busy/重试、重启和重载证据保留于 [完整复验](docs/validation/phase12-2026-11-04.md)。未实施后续业务。

后续诊断补充：用户已确认 Chrome 显示 Manage。运行库中的 browser host 错误实际在缺少访问授权 pipe 地址或校验身份时、连接浏览器之前抛出。当前执行器由 11/02 启动的独立 app-server-daemon 承载，桌面应用重启没有替换该服务；本任务 shell 中三项访问授权环境变量均不存在。直接失败条件已定位为授权上下文缺失，其上游注入原因及恢复尚未验证。未主动重启可能影响其他会话的后台服务；具体证据及恢复步骤见上述 host 连接诊断，Phase 1 验收状态不变。

状态区分：设计完成、未开始、进行中、待验收、已验收、阻塞。完整页面清单、后续平台矩阵和性能实测不阻止基础工程阶段；应在它们所属 Phase 细化并验收。重大变化先说明原因并维护决策历史。

2026-11-02 网络诊断阶段：同一 curl/Cargo 命令在沙箱内连接失败，在经批准的沙箱外成功；隔离探针实际下载 fs2 0.4.3 及其依赖，确认连接阻塞来自执行沙箱的网络限制。后续联网 Cargo 命令使用经批准的沙箱外执行；缓存到位后的 locked/offline 构建已在沙箱内通过。详见 [Cargo 网络诊断](docs/validation/cargo-network-2026-11-02.md)。以下历次验证表保留当时结果，当前状态以上述最新复验为准。

## Phase 0 — 规范、批准语义与架构复审

- **状态：设计完成；本轮复审通过。**
- 交付：五份文档职责、目录/模块、总体 Schema、Tauri 边界、增量扫描、指纹、缓存、会话事件与阶段路线。
- 验收结果：逻辑实例身份、重复内容分离、保守移动、替换隔离、累加 Like、Activation View、Qualified Play、独立进度、completed 和三层播放已正式批准；新 15 项检查无 Phase 1 blocker。
- 后续功能 UX 和量化预算按下列阶段推进，不要求关闭所有未来问题才初始化骨架；实施范围以用户最新指令为准，当前仅 Phase 2 数据层。

## Phase 1 — 工程骨架与数据库基础

- **状态：基础代码、自动测试、发布构建与真实桌面检查通过；仅浏览器预览专项待验收**；Phase 0 与实施授权已满足。
- 目标：建立 Tauri/React/TypeScript/Rust/SQLite 最小骨架、类型化 IPC、最小权限、数据库生命周期与 migration runner；当前构建目标为 Windows x64 MSVC，FFmpeg 留待 Phase 3。
- 验收：骨架可构建启动；README 记录实际命令；已有 DB 普通重启不重建；foreign_keys/WAL/busy_timeout 正确；空库/旧库/迁移失败与 checksum 测试通过；不兼容库拒绝写入，不静默新建空库；React 无 SQL/任意文件权限。
- Schema 范围按最新要求收窄（D018）：v1 只建立 schema_migrations 与 app_metadata；总体媒体/标签/事件/缓存模型保持批准设计，按所属 Phase 用新 migration 引入，不提前实现业务表/command。
- 不要求本阶段完成真实播放、全量 UI、100,000 性能实测或最终平台编码 profile；这些不属于 Phase 1 基础验收。

### Phase 1 已落地内容

- frontend app/api 与 backend commands/domain/services/platform/db 分层；唯一 IPC get_app_status，后台惰性初始化，浏览器预览不伪造成功。
- Tauri app_local_data_dir 下 data/、cache/、cache/thumbnails/ 安全幂等创建；文件、链接/reparse point 与 SQLite sidecar 路径检查。
- rusqlite bundled SQLite、2000ms busy_timeout、foreign_keys、WAL/FULL、应用 lifetime 写实例锁；先验证历史，再改持久 PRAGMA。
- 连续编号/name/SHA-256/时间 ledger；每 migration 与记录同事务；既有库升级前 backup；checksum/gap/新版本/未知库/损坏库拒绝，不覆盖。
- 统一可序列化错误，前端隐藏未知异常详情；基础图标、开发/检查命令与运行说明。
- 基础层 13 项 Windows 适用 Rust 测试现已在完整 Cargo 测试中通过（另有 2 项 Unix 链接测试未在 Windows 执行）；覆盖重开保留、配置/锁、回滚/恢复、校验/备份、损坏库、重试初始化、错误脱敏等。

### Phase 1 当时验证记录（历史，2026-11-01）

| 命令 | 结果 |
| --- | --- |
| node scripts/generate-icons.mjs | 成功生成 PNG/ICO。 |
| npm test | 6/6 通过：状态契约、脱敏、权限/IPC 边界、初始 SQL；使用 Node 内建测试/SQLite，不代表 Rust runner 已通过。 |
| npm install --no-audit --no-fund | 环境失败：registry.npmjs.org 请求 EACCES，依赖无法安装。 |
| cargo check / cargo test | 未运行到编译：cargo/rustc 未安装或不在 PATH；未发现本机 Rust，MSVC/Windows SDK 也缺失。 |
| cargo fmt --all -- --check / cargo clippy --all-targets -- -D warnings | 同上，不能声称格式/lint 已通过。 |
| npm run typecheck / npm run build | 未运行到编译：依赖未安装，tsc 不可用。 |
| npm run tauri -- info / npm run desktop:build | 未运行到原生检查：Tauri CLI 依赖未安装；尚不能启动验证真实 WebView IPC。 |

### 完成验收前仍需执行

此前已通过 desktop:dev、真实 IPC、刷新、重启、第二实例 busy/重试和 WebView 重载，数据库未重建。剩余仅为真实浏览器 desktop_required 专项。最新指定 `@Chrome` 的重试及 REPL 重置仍失败；宿主日志和系统清单确认 pipe 已启动，官方 sky 客户端可以枚举应用，但 `cua_repl` 通道仍报 host 不可用。故障缩小到工具连接或会话绑定，具体失效点尚未确认，详见 [host 连接诊断](docs/validation/browser-host-diagnosis-2026-11-04.md)。本轮 Node 30/30 与 typecheck 通过，没有进入 URL 自动审批或读取页面。Phase 1 整体保持待验收；历次记录见 [再次检查日志](docs/validation/phase1-recheck-2026-11-04.md)，此前已通过的桌面证据见 [完整复验](docs/validation/phase12-2026-11-04.md)。

### 重新验证（2026-11-02；旧 1.90.0 pin 时期历史记录）

环境：Node 24.22.2、npm 11.19.1；Rust/Cargo stable 1.99.1，rustfmt/Clippy 组件已安装。MSVC 14.51.36231、Windows SDK 10.0.26100.0 与 WebView2 可执行文件已找到。Cargo bin 未进入当前 shell PATH，执行时仅补当前进程 PATH，不修改系统环境。

| 实际执行命令 | 结果 |
| --- | --- |
| npm install --no-audit --no-fund | FAIL：获取 https://registry.npmjs.org/@tauri-apps%2fapi 返回 EACCES。 |
| npm ls --depth=0 | FAIL/ELSPROBLEMS：十个直接依赖均未安装，包含 React、TypeScript、Vite、Tauri API/CLI 及类型包。 |
| npm test | PASS：30/30，格式修复后再次成功。 |
| npm run typecheck | BLOCKED：tsc 不可用。 |
| npm run build | BLOCKED：前置 typecheck 的 tsc 不可用，未进行 Vite 构建。 |
| cargo fmt --all -- --check | BLOCKED：项目要求的 1.90.0-x86_64-pc-windows-msvc 未安装。 |
| cargo +stable fmt --all -- --check | 首次 FAIL（项目格式问题）；执行 cargo +stable fmt --all 后重跑 PASS。仅证明 stable 1.99.1 格式检查通过，不冒充固定 1.90.0 验证。 |
| cargo check | BLOCKED：固定 Rust 1.90.0 未安装，未进入编译。 |
| cargo test / cargo test --no-default-features | BLOCKED：同一固定工具链缺失；22 项 Rust 测试未执行。 |
| cargo clippy --all-targets -- -D warnings | BLOCKED：固定 Rust 1.90.0 未安装。 |
| cargo +stable check（补充诊断） | FAIL 于取依赖前：无法访问 index.crates.io:443，下载 config.json 失败，首个报错依赖 fs2。仅使用工作区临时 Cargo cache，不改变项目配置。 |
| npm run tauri -- info | BLOCKED：tauri CLI 不可用。 |
| npm run desktop:dev / npm run desktop:build | BLOCKED：tauri CLI 不可用，未启动桌面或进行原生构建。 |
| git diff --check | PASS；文档更新后再次执行确认。 |

本轮代码修复仅 rustfmt 格式调整；未改变应用行为、migration SQL、依赖版本或 rust-toolchain.toml 来绕过环境。完整记录见 docs/validation/2026-11-02.md。不得推进扫描或下一阶段。

### 再次检查（2026-11-02，第二轮，旧 pin 历史记录）

- 范围确认：services 仍仅 app_status，EventRepository 仅 counts/likes/views/plays 查询，Tauri 仍仅 get_app_status；没有事件记录、资格判断或 Event Service。
- 项目修复：SystemTagRepository 原先的 starts_with(root)+parent 比较会接受 root/..。提取纯标准库的 platform/observation.rs 校验，拒绝 root/path 中 ParentDir；补上 Repository 回归断言。无扫描、磁盘操作或 Schema/migration 改动。
- 实际执行：npm install 再次 EACCES；npm ls 再次 ELSPROBLEMS；npm test 30/30 通过。cargo fmt/check/test、无桌面测试和 Clippy 均再次被固定 Rust 1.90.0 缺失阻塞；前端 typecheck/build 与 Tauri info/dev/build 仍因 tsc/tauri 不存在失败。cargo +stable check 仍无法连接 index.crates.io:443。
- 补充验证：cargo +stable fmt --all 后，其 --check 成功；rustc +stable --edition=2021 --test src-tauri/src/platform/observation.rs -o .tools/observation-tests.exe 编译成功，实际执行该测试二进制 4/4 通过。没有用 mock SQLite、虚假声明或改工具链 pin 代替完整检查。
- 当前 Cargo 测试共 26 项：原 22 项仍未执行，新 4 项只完成 stable 下的模块独立测试；完整 cargo test、固定 1.90.0 与 Tauri 链路仍未验收，不把独立测试混记为 Cargo 通过。
- git diff --check 在文档更新后执行；详情追加于 docs/validation/2026-11-02.md。本轮到此停止。

### 使用最新稳定 Rust 与安装开发依赖（目录整理前历史记录）

- 用户现已明确授权 Rust 升级，rust-toolchain.toml 改为 stable（实际 rustc/cargo 1.99.1）；Cargo.toml 的 rust-version=1.90 只是声明的最低版本，不是 active pin。D025 记录原因和跟踪策略。
- 已执行 npm install --no-audit --no-fund，以及 npm install --save-dev --save-exact typescript@7.0.2 @tauri-apps/cli@2.12.2 --no-audit --no-fund；两者均返回 EACCES，失败 URL 为 https://registry.npmjs.org/@tauri-apps%2fapi。TypeScript/Tauri CLI 仍未安装，不能称为完成。
- 当前默认命令 cargo fmt --all -- --check 成功，不再需要 +stable；实际纯路径模块使用当前 rustc 编译并执行 4/4 成功；npm test 再次 30/30 成功。
- cargo check、cargo test、cargo test --no-default-features、cargo clippy --all-targets -- -D warnings 均实际执行，均在 index.crates.io:443 下载 config.json 时失败（首个依赖 fs2），尚无完整编译/测试结果。
- npm run typecheck/build 仍因 tsc 不可用失败；Tauri info/build 仍因 tauri 不可用失败；npm ls --depth=0 仍报告十个未安装直接依赖。仅安装工具链不能解决 registry 网络权限。
- 已有 26 项 Cargo 测试，仅 4 项完成独立模块执行，不冒充完整 cargo test。无 Event Service/scanner，未变更 Schema 或产品规则。详情见 docs/validation/2026-11-02.md 相应记录，完成后停止。

### 目录整理后完整复验（当前结果，2026-11-02）

根目录为唯一正式工程；重复的默认模板从 MediaManager/ 完整移到 archive/scaffolds/media-manager-template/，35 个文件移动前后 SHA-256 一致。Node 测试按 frontend/architecture/database 分类，审查与验证文档分目录；导入、脚本、TypeScript include、Cargo exclude 和 Vite watch 已同步。没有修改业务代码、Schema、migration SQL 或依赖版本。

| 实际执行命令 | 当前结果 |
| --- | --- |
| npm.cmd ls --depth=0 | PASS：十个直接依赖已安装。 |
| npm.cmd test | PASS：30/30。 |
| npm.cmd run typecheck | PASS。 |
| npm.cmd run build | PASS：类型检查及 Vite production build 均完成。 |
| cargo fmt --all -- --check | PASS。 |
| cargo metadata --no-deps --format-version 1 | PASS：workspace 只有 src-tauri 正式包，归档未被引入。 |
| cargo check | BLOCKED，exit 101：index.crates.io:443 无法连接，fs2 依赖未取得。 |
| cargo test | BLOCKED，exit 101：同上，未进入完整测试。 |
| cargo test --no-default-features | BLOCKED，exit 101：同上。 |
| cargo clippy --all-targets -- -D warnings | BLOCKED，exit 101：同上。 |
| rustc --edition=2021 --test src-tauri/src/platform/observation.rs -o .tools/validation/observation-tests.exe；执行产物 | PASS：4/4，仅纯标准库路径模块。 |
| npm.cmd run tauri -- info | PASS：确认 React/Vite、stable Rust、MSVC 和 WebView2。 |
| npm.cmd run desktop:build | BLOCKED，exit 1：前端 hook 成功，Cargo 下载依赖失败。 |
| npm.cmd run desktop:dev -- --no-watch --no-dev-server-wait --config .tools/validation/tauri-dev-check.json | BLOCKED，exit 101：进入 cargo run 后同一网络错误；临时配置仅关闭前端启动 hook，未启动桌面 UI。 |
| git diff --check | PASS，文档更新后实际执行成功。 |

整理完整性检查通过：归档 35/35 文件摘要一致，12 份文档的 23 个本地链接有效，package-lock 直接依赖与 package.json 一致。结果归档在 [最新验证记录](docs/validation/organization-2026-11-02.md)。没有将环境失败转换成通过；本轮到此停止，不开启后续 Phase。

## Phase 2 — 数据模型与 Repository（当前）

- **状态：当前授权的数据模型与数据库层已验收（2026-11-04，Windows）。** SQL、7 项 Rust Repository 测试、完整 Rust 测试/Clippy、正式 release 构建与 schema=2 真实桌面检查均通过；Phase 1 浏览器专项仍单独待验收，不自动推进后续阶段。
- 目标：media、system_tags、custom_tags、两种关联表、三种日志、quick_browse_sessions、app_settings、migration metadata 与必要辅助表；保持身份/版本/状态边界。
- 交付：0002_library_model.sql 新增 15 张表（含版本、位置、播放状态、快照项和 sequence），v1 不改；Domain 模型、六类 Repository、数据库触发器、Node SQL 测试与 7 项 Rust Repository 测试。
- 核心验收：指定 media 字段完整；fingerprint 非唯一；System Tag 任意树深度/同根/防环；Custom Tag 无子标签、可 CRUD、多对多；唯一/FK/cascade 正确；事件 INSERT/计数原子且日志受保护；保存进度不形成 Play；快照 seed/ordinal 不隐式改变。
- 级联策略：删标签只删关联；System 父目录有子目录时 RESTRICT；有日志/快照的媒体 RESTRICT，不清历史；Repository 不开放媒体硬删除或文件操作。
- 本轮实际验证：npm test 30/30，通过实际 v1/v2 SQL 的约束、事务回滚、升级失败回滚和 FK 检查；包括 recursive_triggers 关闭时 INSERT OR REPLACE 不得改写媒体/日志的回归。
- Rust 源码共有 26 项测试（含原 22 项和新 4 项纯路径测试，2 项 Unix 专用）；本机 Windows 完整 cargo test 默认桌面与无默认特性两种配置均实际执行并通过 24/24。7 项 Repository 测试全部通过；2 项 Unix 专用用例不计入 Windows 通过数。
- 范围排除：无 scanner/FFmpeg/事件资格/Event Service/业务 IPC/UI；QuickBrowse Repository 只存给定成员，不选择/排序/激活媒体；SystemTag Repository 不验证磁盘存在。
- 本轮验收：数据层要求和真实桌面 schema=2/持久化/实例锁检查已通过；未改业务代码、Schema、migration、锁文件或依赖版本。Phase 1 浏览器 desktop_required 专项被工具阻塞；Unix 专用用例、最低 Rust 版本、后续扫描与规模验收不在本轮 Windows 数据层通过范围。详见 [最新测试记录](docs/validation/phase12-2026-11-04.md)。Phase 2b 仍未开始、未授权。

## Phase 2b — Scanner 与 reconciliation（另行授权）

- **状态：未开始，未授权。** 原 Phase 2 扫描范围移至此处，避免与最新数据层指令冲突。
- 目标：真实目录观察、根/扫描状态、指纹、有界增量队列及恢复；所需新表使用后续 migration。
- 验收：new/existing/moved/renamed/modified/missing/restored、跨根/覆盖/文件 ID 复用、false-negative 优先、无损历史、离线不误标与大视频 I/O 预算。
- 当前只预留 Repository seam，禁止以数据库测试为由提前实现遍历/匹配。

## 历史路线 — 三层播放缓存（仍待单独授权）

- **状态：未开始**；依赖：Phase 2 数据层及后续授权的媒体输入/扫描基础。
- 目标：Direct→Remux→Proxy、后台缩略图、受限 range 资源、FFmpeg 隔离与可再生缓存。
- 验收：目标平台 Direct/容器不兼容 Remux/codec 不兼容 Proxy 三条路径；Remux 不重编码；失败可升级、取消/超时可回收；切换播放资源不改变 activation/session。
- 代理默认并发 1、可配置至 1–2，受全局 FFmpeg 预算约束；按键去重、可报进度；原件永不作为输出。完成验证后才发布 ready。
- 版本变化失效、cache miss 重建、孤儿清理和有界 LRU/pin 均正确；代理时间线与原媒体一致。平台 profile/字幕音轨/HDR 能力及 FFmpeg 获取/许可方案在本阶段记录，发布前完成许可核查。

## Phase 4 — Metadata 与 Thumbnail Cache（本次授权）

- **状态：实现及本轮 Windows 验收完成（2026-11-08），停止。** 本阶段按用户最新指令执行；现有 v3 scanner 为输入，不实施播放、浏览 UI 或 Event Service。
- 图片 width/height；视频 width/height/duration/video_codec，附 audio_codec/container/frame_rate；未知值 NULL。ffprobe/FFmpeg 均以参数数组调用，不经 shell。
- v4 migration 新增必要媒体字段及 media_derivatives 持久队列；旧 migration 不改。升级前沿用一致性备份；仅旧的无版本 thumbnail_path 失效，事件、标签、播放状态和稳定身份保留。
- 应用初始化后启动独立连接的后台 worker；每次一项、短事务、全局一个 ffprobe/FFmpeg 槽位。扫描仅由 trigger 排队，不等待解码。进程默认超时 30 秒，可取消并 kill/wait；stdout <=1 MiB、stderr <=64 KiB、单次分配限制 64 MiB、像素上限 100M。
- 320 边界内等比 JPEG，视频优先约 10% duration，取帧失败可回退首帧，未知时长取首帧。cache/thumbnails/<sha256>.jpg，key 含 media ID、content version、fingerprint、size/mtime、variant、generator version；不以 path 作 identity，不共享独立文件的缓存键。
- 同目录 .part 写入、sync、rename 后短事务标 ready；不写原件，不向 SQLite 存 binary。内容证据变化清 metadata/path 并重新排队；旧 worker 用版本/指纹/路径/CAS 拒绝迟到发布。移动/重命名复用；ready 文件丢失后 request_thumbnail 重新排队；失败显式 retry，running 在重启恢复。
- 已覆盖正常/失败/恢复、损坏图片/视频、缺工具、未知 codec/时长、进程失败/超时/取消/输出上限、缓存写失败、文件丢失、重启、移动后标签/事件/进度保留、同路径替换隔离、版本切换与事务回滚。真实 FFmpeg 测试用颜色变化验证 10% 帧，并独立解码输出 JPEG。
- 工具来自 PATH 或 MEDIA_MANAGER_FFMPEG_DIR；测试用便携 FFmpeg/ffprobe 9.0.2 位于忽略的 .tools/ffmpeg/，不修改系统 PATH，不纳入发布包。本轮不宣称发布许可、播放器能力、LRU 或 100,000 UI 规模验收。

| 最终实际执行命令 | 结果 |
| --- | --- |
| cargo test --locked --offline -- --include-ignored | PASS，40/40；全部 4 项真实 FFmpeg 集成用例执行，无跳过项。 |
| cargo test --locked --offline --no-default-features -- --include-ignored | PASS，40/40；同样包含全部真实工具用例。 |
| cargo clippy --locked --offline --all-targets -- -D warnings | PASS。 |
| cargo fmt --all -- --check | PASS。 |
| npm.cmd test | PASS，30/30。 |
| npm.cmd run test:dev | PASS，1/1。 |
| npm.cmd run build | PASS，TypeScript 严格检查与 Vite production build。 |
| git diff --check | PASS。 |

测试环境及故障/恢复证据见 [Phase 4 验证记录](docs/validation/phase4-2026-11-08.md)。本轮 Clippy 另发现既有 scanner 参数过多及无效 drop；仅聚合 ScanRun 上下文、移除无效 drop，未改身份判定或扫描规则。没有新增依赖或改动锁文件，旧 migration 保持不变。Windows 结果不含 Unix 条件用例；本轮未重跑真实 UI smoke 或正式 release 打包。

## 后续路线 — 媒体浏览、标签与稳定会话（未授权）

- **状态：未开始**；依赖：Phase 2–3。
- 目标：主媒体导航、虚拟 Grid、SQLite 过滤/排序/搜索、目录投影和 Custom Tag、稳定 Random/Quick Browse。
- 验收：100,000 合成媒体下默认约 100 项 keyset 分页、深页无巨大 OFFSET；卡片不逐个 IPC；前端不全量排序/过滤，Grid/缩略图资源有界。
- 相同 seed/成员/算法可复现；ready 会话分页/扫描/点赞/改标签/重渲染不重排，新增不插入、缺失项不压缩；building 不可见；有界 spool/会话配额与可取消构建成立。
- 冻结页面/MVP、搜索字段、标签重名/祖先 UX、会话保留和恢复交互；追加约束走 migration，不反改已批准业务语义。

## Phase 5 — 基础 Media Library（已完成）

- **状态：实现与本轮 Windows 验收完成，停止。** 六个组件、只读树形 System Tags/递归父目录筛选、平级 Custom Tag CRUD/单项与批量关联、类型化查询 API 已实施。
- 每页 100 条 keyset，SQLite 筛选/排序，陈旧游标明确拒绝；前端仅保存当前页及有界游标历史，可见行虚拟化，lazy thumbnail 批量请求，不读取原始高清图。Like Count 只读，无 Event 或播放器实现。
- 新增 v5 migration（查询代次/索引），保留历史 migration。Custom Tag 批次事务失败回滚，原媒体/目录关系/事件均不变；asset scope 只开放缓存 JPEG。
- 最终通过前端 Node 37/37、开发监听 1/1、backend 两种 feature 配置各 45/45（包含真实 FFmpeg）、严格 build/Clippy/fmt/diff-check；后端 100,000 合成媒体 1000 页无漏项，首次实测 query p50=0.475ms、p95=0.658ms、末页=0.442ms（本机热查询，非全平台性能承诺）。
- 清理后使用 Phase 1 隔离 Tauri 方式完成 UI 11/11 DOM 断言（含双媒体批量增删、分页/虚拟化）；修复夹具在标签内存更新后过早查 DOM 的竞态，不删除或跳过断言。另用 111 个合成媒体通过正式 SQLite/Tauri IPC、递归目录筛选、100＋11 分页、真实 thumbnail/视频 metadata 与标签添加验收，原图摘要不变、事件数仍为零。
- 上轮 Chrome browser host/URL 审批问题仍作为通道限制保留，不宣称它已修复；本轮原生桌面验收已独立通过。测试结束两个 Tauri 进程正常退出、测试窗口为空、1420 无监听。详见 [Phase 5 验证与截图](docs/validation/phase5-validation.md)。

## Phase 6 — ImageViewer、VideoPlayer 与进度（验收历史）

- **状态：已验收（Windows，2026-11-08）。** 图片 previous/next/zoom/pan/fit/100%/fullscreen，视频 play/pause/timeline/volume/mute/speed/fullscreen/previous/next，二者有 Details/Custom Tag。
- 受限 media URI 只接受打开后的短期 token/mode，不接收原始路径；后端只读句柄和 bounded Range 请求，关闭即撤销。原媒体目录不加入 asset scope。
- Direct → Remux → H.264/AAC Proxy，后台/可取消/超时，复用全局 FFmpeg gate；缓存按 ID/version/fingerprint/stat/profile 隔离，验证后原子发布，有界容量与活动句柄 pin。
- v6 migration 仅加入 playback_owners。PlaybackState 按稳定 ID/版本独立保存，owner + state_version + sequence 防止迟到写；前端串行合并 checkpoint，失败先按同一请求重试。保存点包含 5 秒周期、pause/ended、切换、关闭、pagehide/visibility，以及原生关闭前最多 3 秒尽力 flush。
- 前端 Node 42/42、两种 Rust feature 配置各 54/54（含真实 FFmpeg）、开发监听 1/1、build/Clippy/fmt 已通过。真实 Tauri 回归 15 项通过，并验证图片拖动、图片/视频全屏和退出保存 24.5 秒；事件/播放计数为零、5 个合成原件指纹不变。修复仅音频可解码时的黑屏回退与重载会话回收，见 [验证与截图](docs/validation/phase6-validation.md)。没有新增 Play/View/Like Event 或 completed 资格判断；不推进下一 Phase。

## Phase 7 — Event Log + Counter Cache（验收历史）

- **状态：已验收（Windows，2026-11-09）。** 三个事件日志为事实来源，media 计数仅为 Counter Cache。新增 v7 migration，旧事件与约束保留；Counter Cache 继续由原触发器维护，服务不重复累加。
- add_like 按 request_id 幂等；4 并发调用者累计 100 次、日志=100/cache=100。LikeButton 和 L 共用服务，无 toggle/撤销，Ctrl/Alt/Meta 组合及键盘自动重复不误记主动 Like。
- record_view 按 viewer ordinal/activation 幂等，媒体成为当前主项后才提交；hover/预取/rerender 不记，A→B→A 三次，source 支持全部五种批准值。
- record_play/open_player_session 基于正常播放状态与单调经过时间；普通/未知 duration >=5 秒，短片真实 ended，seek/缓冲/暂停/恢复不虚构观看。资格 INSERT，一次 finalize_play 补全最后状态，无重复计数；completed 使用批准公式，独立进度路径仍不写事件。
- rebuild_like/view/play_count、各 all_counts 及 rebuild_statistics 后台高水位/base/delta/checkpoint；1000 日志/100 媒体短事务，有界清理、取消、失败与崩溃续跑，不在普通启动全库重建。单媒体计数/时间原子发布，全库逐批修复。
- 12 项新增 Rust 场景包含 Like/View/Play 事务失败、COMMIT 失败、重建并发/恢复/零项、播放与 activation 边界；完整前端 47/47、backend 两种配置各 66/66，build/test:dev/Clippy/fmt 通过。100,002 媒体/2,502 事件在 1,009 步完成，桌面 10 项真实 IPC 回归与原生 L 检查通过，退出查库全部缓存及最近时间零差异。
- 验收截图、实测和清理见 [Phase 7 记录](docs/validation/phase7-validation.md)。不新增 Quick Browser/History 页面或文件操作，到此停止。

## Phase 8 — 完整查询系统（当前授权）

- 三类搜索和组合筛选。Custom Tag 多选 AND/OR，包含端点的计数及导入日期范围，已查看和未查看状态。
- 15 种排序采用 SQLite keyset。Migration 8 建立排序和类型索引，游标绑定过滤、排序、查询代次和 seed。
- Random 按 SHA-256 v1 和稳定 seed 排列，首次建立临时索引，热分页复用。Shuffle Again 保存新 seed。
- 普通和随机各遍历 100,000 项及 1,000 页，无重复和遗漏。桌面配置普通 p95 为 1.261 ms，随机热分页 p95 为 1.950 ms，随机建索引加首页为 1,562.122 ms。组合筛选的 2,001 项结果和实际 EXPLAIN 检查均通过。
- 自动测试、构建、Clippy、格式检查与隔离桌面截图见 [Phase 8 验证记录](docs/validation/phase8-validation.md)。

## 后续路线 — 恢复、规模与发布验收（未授权）

- **状态：未开始**；依赖：Phase 1–5。
- 目标：目标平台兼容、故障注入、备份恢复、长期运行、性能及打包分发。
- 验收：至少 100,000 媒体、足够事件量和真实大视频；提前约定硬件与预算，记录冷/热查询 p50/p95、深页、Grid DOM/内存、扫描/哈希读取量、代理吞吐、会话构建磁盘/WAL、统计批次及前台等待。
- 启动先数据库/UI、后后台扫描；rebuild 不长期独占写入；有界队列无无界内存增长。升级/恢复、坏文件/进程失败、离线磁盘、路径越界和缓存误删防护通过。
- FFmpeg 构建许可、目标 WebView 编解码格式、自动更新和发布包验证完成；未通过不得宣称性能/兼容已验收。

## 依赖与状态维护

主路径 0→1→2→3→4→5→6→7→8。数据层接口在依赖满足后细化。每次推进记录完成范围、实际验证结果和剩余风险。

当前授权为 Phase 8 完整查询系统，性能验证后停止。
