# 分阶段开发计划

## 当前状态与门槛

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

## Phase 3 — 元数据、Thumbnail 与三层播放缓存

- **状态：未开始**；依赖：Phase 2 数据层及后续授权的媒体输入/扫描基础。
- 目标：Direct→Remux→Proxy、后台缩略图、受限 range 资源、FFmpeg 隔离与可再生缓存。
- 验收：目标平台 Direct/容器不兼容 Remux/codec 不兼容 Proxy 三条路径；Remux 不重编码；失败可升级、取消/超时可回收；切换播放资源不改变 activation/session。
- 代理默认并发 1、可配置至 1–2，受全局 FFmpeg 预算约束；按键去重、可报进度；原件永不作为输出。完成验证后才发布 ready。
- 版本变化失效、cache miss 重建、孤儿清理和有界 LRU/pin 均正确；代理时间线与原媒体一致。平台 profile/字幕音轨/HDR 能力及 FFmpeg 获取/许可方案在本阶段记录，发布前完成许可核查。

## Phase 4 — 媒体浏览、标签与稳定会话

- **状态：未开始**；依赖：Phase 2–3。
- 目标：主媒体导航、虚拟 Grid、SQLite 过滤/排序/搜索、目录投影和 Custom Tag、稳定 Random/Quick Browse。
- 验收：100,000 合成媒体下默认约 100 项 keyset 分页、深页无巨大 OFFSET；卡片不逐个 IPC；前端不全量排序/过滤，Grid/缩略图资源有界。
- 相同 seed/成员/算法可复现；ready 会话分页/扫描/点赞/改标签/重渲染不重排，新增不插入、缺失项不压缩；building 不可见；有界 spool/会话配额与可取消构建成立。
- 冻结页面/MVP、搜索字段、标签重名/祖先 UX、会话保留和恢复交互；追加约束走 migration，不反改已批准业务语义。

## Phase 5 — Activity、Playback State 与 Statistics

- **状态：未开始**；依赖：Phase 1、Phase 4；资格语义已经冻结，不再以旧 Q03–Q05 阻塞。
- Like：五次主动动作产生五事件，相同动作重试只一次；无 toggle/撤销。View：A→B→A 三次，render/resize/fullscreen/预加载零次。
- Play：打开不计，普通视频 >=5 秒真实播放才计，<5 秒短片真实 ended 才计；恰好 5 秒、未知时长、2x、seek、pause/resume、失败/重试有边界用例。同 session 最多一个，切换/结束后新 session 可再计。
- 生命周期：资格 INSERT+Counter Cache 同事务；结束只补全同一行，不再计数；终结重试、崩溃 interrupted、禁止改身份/资格字段测试通过。
- PlaybackState 独立保存，无事件也可恢复；CAS/owner/旧版本拒绝陈旧覆盖；completed 覆盖 95% OR remaining<=min(10s,5% duration)，短片 seek 到近尾不能替代真实 Play 资格。
- 统计：注入 INSERT、计数更新、重建 delta、COMMIT 失败均整体 rollback。rebuild_like_count、rebuild_all_like_counts 与统一 rebuild 正确含零项；高水位/base/delta/checkpoint 在并发事件、取消、崩溃续跑下不漏计双计，发布每媒体原子、全库分批，UI 保持可用。

## Phase 6 — 恢复、规模与发布验收

- **状态：未开始**；依赖：Phase 1–5。
- 目标：目标平台兼容、故障注入、备份恢复、长期运行、性能及打包分发。
- 验收：至少 100,000 媒体、足够事件量和真实大视频；提前约定硬件与预算，记录冷/热查询 p50/p95、深页、Grid DOM/内存、扫描/哈希读取量、代理吞吐、会话构建磁盘/WAL、统计批次及前台等待。
- 启动先数据库/UI、后后台扫描；rebuild 不长期独占写入；有界队列无无界内存增长。升级/恢复、坏文件/进程失败、离线磁盘、路径越界和缓存误删防护通过。
- FFmpeg 构建许可、目标 WebView 编解码格式、自动更新和发布包验证完成；未通过不得宣称性能/兼容已验收。

## 依赖与状态维护

主路径 0→1→2→3→4→5→6；数据层接口可在依赖已满足时细化，不能因阶段重排绕过事务和模块边界。每次推进记录完成范围、实际验证命令/结果和剩余风险。

当前只授权 Phase 2 数据模型与数据库层；本轮该范围已在 Windows 上验收，Phase 1 浏览器专项未完成如实保留，不能自动推进 scanner、Event Service 或下一阶段。阶段授权与阶段验收分别记录。
