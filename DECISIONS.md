# 重要技术决策记录

## 记录规则与批准来源

仅收录影响数据模型、架构边界、持久化语义和重要选型的决策，不记录普通实现细节。状态使用：已批准（Approved）、提案、已取代。当前 D001–D017 均为本轮批准规则下的设计基线；实现参数和后续 UX 不能把这些规则重新标成 pending。

日期：2026-11-01。用户已批准产品规则并授权 Phase 1，现明确授权 Phase 2 数据层。D018–D020 保留 Phase 1 历史，D021–D024 记录本轮表名、存储与约束选择。原生验证仍受环境限制，不能将 SQL 测试结果等同于 Rust 编译/Repository 测试通过。

## D001 — 固定技术栈

- 状态：**已批准（Approved）**；来源：用户明确指定。
- 决策：Tauri、React、TypeScript、Rust、SQLite、FFmpeg。
- 影响：初始化锁定版本；具体库/包管理器/驱动是 Phase 1 选型，不得替换固定栈。

## D002 — 逻辑文件实例身份与重复内容分离

- 状态：**已批准（Approved）**；原 Q02 身份粒度已关闭。
- 决策：media_id 是逻辑文件实例；path、fingerprint、full hash 均不是身份。独立路径文件默认独立 ID，摘要值禁止 UNIQUE，不按内容自动 deduplicate。
- 理由：相同内容不能证明实例相同，避免历史归属污染。
- 变更：旧“实例/内容聚合待定”已撤销。高置信度移动保留原 ID/imported_at/全部关联；歧义在有界对账后 missing+new；同路径替换旧 superseded、新 ID/导入时间/零计数、无标签进度继承。
- 影响：manual relink 留待未来；即使 false negative 产生重复项，也不能误合并历史。

## D003 — 三类 Event Log 是 Source of Truth

- 状态：**已批准（Approved）**。
- 决策：计数以 like_events/view_events/play_events 为准；事件插入与对应 media Counter Cache 更新同事务提交或回滚，显式支持重建。
- 变更：旧 Like delta/撤销提案已被正式规则替代；Like 为主动动作次数，View 为 activation，Play 为 qualified player session，详见 D013。
- 影响：会话工作状态和 playback state 不是统计来源。任何性能优化不得拆开事实插入与计数事务。

## D004 — 只读媒体、增量和故障隔离

- 状态：**已批准（Approved）**。
- 决策：原媒体只读、不删除；普通启动不重建库，先 DB/UI 后后台增量；扫描/解码有界后台处理，坏文件隔离，Schema 走可追踪 migration。
- 理由：保护资产/历史、保证增长后的响应和恢复。
- 影响：缓存可写和可回收，但须与原件路径隔离；新增物理文件修改功能需另行产品设计。

## D005 — Rust 写入与明确服务边界

- 状态：**已批准（Approved）**；本轮规模/数据库规则和现有边界共同确认。
- 决策：React 仅类型化 IPC，Rust 用例服务拥有事务；本地 SQLite WAL、foreign_keys、busy_timeout、集中写协调器与有限读连接。
- 理由：集中不变量与权限，SQLite 单写者下用短事务与公平调度控制竞争。
- 影响：写协调器不承载业务；服务拆分见 D012。有界 worker/FFmpeg 预算共享，不以 async 包装同步阻塞 UI。

## D006 — System Tag 投影、Custom Tag 独立

- 状态：**已批准（Approved）**。
- 决策：System Tag 由真实目录及当前位置生成，directory_closure 只是可重建投影；Custom Tag ID 和媒体关联表独立于文件系统。
- 理由：不能有第二个可任意修改的目录事实来源。
- 影响：离线标注上次扫描，祖先/根名称展示和标签重名政策后续细化，不改变物理目录约束。

## D007 — 可丢弃的版本化媒体缓存

- 状态：**已批准（Approved）**；新增播放派生缓存沿用此原则。
- 决策：缓存键包含 media_id、内容版本、variant/profile 和生成器版本；后台生成、原子文件发布、cache miss 可再生，LRU 仅清理应用缓存。
- 理由：路径不应成为缓存身份，内容版本变化不得误用旧资源。
- 影响：文件/SQLite 无共同事务，允许孤儿和缺文件并进行纠正；播放句柄 pin 防清理使用中的缓存；绝不覆盖原件。

## D008 — 设计就绪与实施授权分离

- 状态：**已批准（Approved）**；原则保留，旧轮仅设计的执行范围已由 D018 替代。
- 决策：Ready=YES 不自动授予实施权限；用户现已明确授权 Phase 2 数据层，仍不自动授予 scanner 或其他阶段权限。
- 变更：旧“完整页面/所有未来问题未定即 NO”的门槛已收窄；仅真正改变 Phase 1 基础架构的业务 blocker 可阻止放行。性能实测属于后续验收。
- 影响：实施与验收分别记录；本轮范围以 D021 为准，不把设计批准等同于全部实现已验收。

## D009 — 内容版本与独立 Playback State

- 状态：**已批准（Approved）**；由原 pending 转为 approved。
- 决策：media_revisions 和 playback_states 独立建模，版本键统一为 (media_id,content_version_id)；position_seconds/duration_seconds/updated_at/completed 满足产品字段要求。CAS/owner 拒绝陈旧进度。
- 理由：保留同内容移动后的恢复位置，将进度与 Play 生命周期解耦。
- 变更原因：原稿仅有 position_ms、缺 duration/completed，现明确秒单位；同路径确认不同媒体不能只 bump version，必须新 ID，无状态继承。旧版本状态保留但不自动套新版本。
- 影响：旧 content_revision 命名改为 content_version_id；当前只改设计，未来正式 DDL 用复合外键，不迁移/复制历史给新实体。

## D010 — 分层指纹与保守收尾

- 状态：**已批准（Approved）**；由原 pending 转为 approved，匹配失败处理按新规则修改。
- 决策：stat 优先、快速采样最多 3 MiB、可信同卷连续性、按需流式完整摘要；算法版本明确、摘要非唯一。
- 理由：整视频读取不应成为普通扫描、启动或播放门槛，摘要相同不能证明同实例。
- 变更原因：旧“证据不足继续 ambiguous、等待人工”不是最终业务结果；改为有界对账后 missing+new。完整 hash 也不能替代实例证据。
- 影响：保守策略接受 false negative，manual relink 不在当前 Phase；快速采样无法检测所有改写，深度核验异步进行。

## D011 — Stable Random 与 Quick Browse 快照

- 状态：**已批准（Approved）**；与本轮规则一致，由 pending 转为 approved。
- 决策：持久化 seed、算法版本、冻结成员与 ordinal；Random 使用确定性摘要键和 ID tie-break。后台分批构建，ready 后分页不重排。
- 理由：单有 seed 不足以抵御扫描/标签变化引起的成员变化。
- 影响：O(N) spool/排序/磁盘项、有界配额与可取消 job；不逐页 RANDOM、不前端拉全库。会话 UX/保留期后续细化，不是 Phase 1 blocker。

## D012 — 扫描分类与分离用例服务

- 状态：**已批准（Approved）**；由 pending 转为 approved，歧义最终状态修正。
- 决策：new/existing/moved/renamed/modified/missing/restored 可组合位置/内容维度，规范化候选/证据关联；扫描、查询、标签、活动、进度、播放、浏览、缩略图、统计按职责分离。
- 理由：防止混淆移动与内容替换，避免巨型 LibraryService。
- 变更原因：ambiguous 仅中间态，结束后必须 matched 或 conservative split；root 离线/权限失败不等于 missing。旧单 services 目录已明确拆分。
- 影响：身份证据不是活动事实日志，不能借 reconciliation 改写事件或自动合并标签。

## D013 — 冻结事件资格与可重建统计

- 状态：**已批准（Approved）**；原候选政策被本轮正式决策替换。
- Like：每主动动作一事件，无 unlike/toggle/delta；重试幂等不压制新的主动动作。
- View：Media Activation，同 activation 一次；A→B→A 三次；render/resize/fullscreen/预加载不产生 activation，不用时间 debounce 定义。
- Play：普通真实 >=5 秒，<5 秒短片真实 ended；同一 player session pause/resume/seek/重播不增事件。打开播放器不是 Play，进度保存不是 Play。
- 变更原因：废止旧 min(3 秒,时长一半) 及“重播自动新 session”候选，保证按产品定义统计。qualified INSERT+counter 同事务；生命周期补全见 D017。
- 影响：count 统一按三个日志 COUNT；必须支持单媒体 Like、全库 Like 和统一统计 rebuild。新资格版本只影响新会话，不重解释历史。

## D014 — 100,000+ 基线与评审放行

- 状态：**已批准（Approved）**；原 pending 转为 approved。
- 决策：SQLite keyset/索引、约 100 项分页、虚拟 Grid/lazy thumbnails、有界缓存和独立后台队列；启动优先 DB/UI，不等全库扫描。
- 理由：禁止无界对象加载、全库前端排序和无界 FFmpeg，确保可执行的规模设计。
- 变更原因：原全库统计单写事务与 UI/资源约束冲突，采用 D016；原 review 的 NO 由本轮已批准语义及新 15 项复审取代。
- 设计结论：**Ready for Phase 1: YES**。性能/兼容实测留作对应 Phase 验收；用户后续已授权 Phase 1，实际实现状态见 PLANS.md。

## D015 — Direct / Remux / Proxy 三层播放

- 状态：**已批准（Approved）**；三层策略来源：用户明确批准；以下 profile 为工程首选基线。
- 决策：Direct 可用先 Direct；codec 可用而容器不兼容先 stream-copy Remux；否则按需 Transcode Proxy。首选 H.264 + AAC MP4、yuv420p，具体 profile/分辨率/硬件加速/许可按平台验证，不保证所有 WebView 无条件兼容。
- 理由：避免不必要重新编码，同时覆盖不能直接播放的源媒体。
- 影响：cache/playback/<media_id>/<content-version>.mp4 或含 profile 版本的等效隔离路径；可删除再生、版本失效，原件只读。播放生成默认 1 job、可调 1–2，受全局 FFmpeg 预算；回退不新建 activation/session。
- 发布门槛：目标 WebView 编解码/时间线/range 与 FFmpeg 构建许可验证，不阻止 Phase 1 数据基础。

## D016 — 高水位、并发补偿与分媒体统计修复

- 状态：**已批准（Approved）**的 bounded rebuild 要求下采用的架构基线。
- 变更原因：替换旧全库单事务和“全库一瞬间发布”提案，避免长写锁令前台长期不可用；不改变事件事务原子性。
- 决策：全局 sequence 捕获 H；<=H 日志分批聚合至磁盘 base；active job 期间事件 INSERT+Counter Cache+delta 同事务；每媒体短事务发布 base+delta、原子推进 cursor，新增媒体不在旧目标。
- 影响：新增 db_sequence、重建 job/base/delta 表；内存/批次有界，故障按 checkpoint 恢复，取消不回滚已完成修复。全库修复是渐进发布，不谎称全局原子；单媒体目标统计仍原子。
- 验收：并发插事件、零事件、巨大单媒体日志、队列背压、崩溃/取消、磁盘耗尽均不得丢事实或重计。scratch 不是 Source of Truth。

## D017 — Play 单事件的受限终结补全与 completed

- 状态：**已批准（Approved）**的 play_events 字段/生命周期要求下采用的架构基线。
- 变更原因：qualified_at 时尚未知 ended_at、最终 end_position/watched_seconds，旧“所有字段永不 UPDATE”无法表达一 player session 一个完整事件。
- 决策：资格达成时唯一 INSERT 并加计数；结束只对同一行补全结束元数据一次，禁止改 id/媒体/版本/session/资格事实，禁止删除；终结不再计数，结束后冻结。异常退出只使用持久化证据，标 interrupted。
- completed：有效 duration 下 progress>=95% OR remaining<=min(10s,duration*5%)；未知/无效时长 false、可后续按有效元数据计算。95% 完成不等于短视频真实 ended 资格。
- 影响：like/view 仍不可变；Play 触发器限制终结允许字段和状态。playback_states 独立存相同完成定义，但不能用另一个会话的进度覆盖本事件终结数据。

## D018 — Phase 1 授权与最小 Schema 范围

- 状态：**已批准（Approved）**；来源：用户最新“现在开始 Phase 1，只实现 Phase 1”。
- 决策：只建立基础模块、SQLite/migration、运行目录、统一错误和 get_app_status；不实现 scanner、媒体查看/播放、标签、活动和 Quick Browse。
- 变更原因：旧计划把媒体/事件全部 Schema 提前列入 Phase 1，与最新“最小干净骨架”范围不符；v1 仅含 ledger/app_metadata，未来按所属 Phase 正式迁移，不改变批准的总体数据模型。
- 历史影响：当时不安装/运行 FFmpeg、不创建 cache/playback，Phase 2 未开始；现已由 D021 明确授权数据层，仍不包含 scanner/业务服务。

## D019 — Phase 1 存储生命周期与阻塞工作隔离

- 状态：**实施决策**；在用户授权的 Phase 1 范围内细化 D005。
- 决策：rusqlite bundled SQLite；Tauri app_local_data_dir 下 data/cache/thumbnails；唯一状态用例通过 spawn_blocking，Mutex 串行化连接和初始化，文件锁存活至连接关闭。
- 理由：无需前端 SQL 或外部 SQLite 安装，避免 UI 线程做文件/数据库 I/O；最小阶段不引入未使用的读连接池/通用任务系统。
- 影响：SQLite 原生编译仍需要 C 工具链。后续有业务负载再按既定模块边界扩展写协调器及有界读连接，不将状态服务演变成巨型业务服务。
- 迁移策略：先校验历史再改变 WAL；版本/name/SHA-256/时间追踪，每 migration 与 ledger 同事务；现有库升级前 online backup，未知/损坏库拒绝。ledger 自举 DDL 是框架基础，业务 DDL 只来自编号 SQL。

## D020 — 依赖版本与验证诚实性

- 状态：**历史实施决策**；其中 Rust 1.90.0 pin 已由用户最新授权的 D025 替代，其余依赖和验证诚实性原则保留。
- 决策：固定 Rust 1.90.0，Cargo resolver 3 按 MSRV 解析；直接依赖精确版本见 manifest。Node 基线 >=22.18，当前 24.22.2。Tauri 2.12.5 / tauri-build 2.7.4，rusqlite 0.41.1；前端 React 19.3.1 / Vite 8.3.5 / TypeScript 7.0.2，来源已核对官方版本。
- 理由：避免初始依赖隐式漂移，rusqlite API 以所选版本为准；无桌面特性可单独测试基础后端，但不能替代桌面链路验证。
- 限制：当前终端无 Rust/Cargo、MSVC/SDK；npm 请求被系统 EACCES 拒绝。不能获取完整依赖、生成真实 npm/Cargo 锁文件或完成原生构建，因此 Phase 1 必须保持待验证，不能以静态审查冒充通过。
- 后续：环境补齐后生成并提交两个真实 lockfiles、执行全部要求检查/修复和桌面 smoke，再更新验收。不得用虚假类型声明、stub React/SQLite 或跳过错误脚本获取“绿色构建”。

## D021 — Phase 2 只实现数据模型与数据库层

- 状态：**用户已授权**；替代原 PLANS 中 Phase 2 自动包含 scanner 的执行范围。
- 决策：通过新 migration v2 建立指定十类表、必要关系/版本辅助表和 Rust Repository；不实现 scanner、Event Service 或 UI 业务，不新增业务 Tauri command。
- 理由：用户明确要求先验证数据结构与数据库行为，不应提前实现业务资格或文件遍历。
- 影响：Phase 1 原生验证限制不被抹去；仅推进已明确授权的数据层。scanner 改为另行授权的 Phase 2b，完成后停止。

## D022 — 当前媒体快照与历史/版本分离

- 状态：**实施决策**；按用户明确字段要求细化 D002/D009。
- 决策：media 存放指定的当前字段，id/imported_at/created_seq 不可改；fingerprint 非唯一，活动路径仅作占位约束。media_revisions 独立版本，media_locations 由 trigger 记录位置历史；不是 path/hash identity。
- 变更理由：原草图把当前 path/metadata 完全分表，现需要直接可查询的 media 记录；保留历史实体和 FK，而非丢弃已确认身份模型。playback_position 只是当前版本 playback_states 的投影。
- 影响：新表只经 0002 创建，0001 不改。时间点毫秒、播放单位秒；未知元数据 NULL，计数/进度默认零。不存在原文件操作或自动 identity matching；同路径替换仍需新 ID，旧 superseded。

## D023 — 目录投影、平面标签和删除策略

- 状态：System/Custom 规则**用户明确批准**；唯一性/级联细节为实施决策。
- 决策：system_tags 替代 directories 命名，parent_id+source_root 复合 FK、防环/唯一目录；Repository 只接收绝对根内的直接父子观察，不能手工建立 UI 目录。Custom Tag 无 parent_id，trim 后非空、SQLite NOCASE 名称唯一，标准关联表双向索引。
- 理由：一个事实来源，避免字符串标签和虚拟目录混入物理结构。ASCII 大小写不敏感、非 ASCII 精确比较的行为明确而可测试。
- 影响：标签删除 CASCADE 关联行，System 父节点有子节点则 RESTRICT；媒体如有事件或快照引用则 RESTRICT，不能级联删除历史。无历史媒体的关联/版本/位置/状态可随数据库行删除清理，但 Repository 不开放媒体硬删除，更不删原文件。

## D024 — 数据库 trigger 保证事件与缓存原子性

- 状态：**实施决策**；细化 D003，不增加 Event Service。
- 决策：三类日志 INSERT 的 trigger 同事务更新各自 Counter Cache；view/play 同步 last_*。日志不可变及 Play 一次终结约束由数据库保护。EventRepository 只读，不提供记录/资格/重建方法。
- 理由：只有表却无一致性入口容易产生“事件已插、缓存未更新”；以数据库约束覆盖所有未来写入口，并测试溢出/外层 rollback。以后服务**不得重复执行 +1**。
- 影响：未来 Event Service 负责资格/幂等和 sequence 分配，统计 rebuild 的 delta 捕获仍需后续正式 migration 接入；当前没有 active rebuild 或业务事件入口。当前 FK 保护媒体/内容版本；工作 activation/player session 表尚未引入，后续加 FK 必须迁移/回填，不丢日志。

## D025 — 用户授权切换最新稳定 Rust，并安装本地开发依赖

- 日期：2026-11-02；状态（2026-11-04 复验）：**用户已授权，Rust 配置已实施；npm/Cargo 依赖与真实锁文件已到位，命令行及真实桌面检查通过，浏览器预览专项待补**。
- 来源：用户询问使用 Rust 最新版本替代，并要求代装 typecheck/Tauri。
- 决策：rust-toolchain.toml 从 1.90.0 改为 stable，使用已安装的稳定编译器；本轮本机 rustc/cargo 为 1.99.1，且 [Rust 官方 stable 源版本](https://raw.githubusercontent.com/rust-lang/rust/stable/src/version) 同为 1.99.1。不修改 Cargo.toml 的声明最低版本 1.90，不宣称该最低版本已测试。
- 原因：旧 pin 未安装，而用户现已明确授权升级；这不同于前两轮未经许可为避错更换编译器。无需重新安装本机已有的最新 stable，不改用户全局默认或产品数据模型。
- 影响：stable 通道可随 rustup 更新；后续升级记录实际 rustc/cargo 版本，并重新跑完整检查。此政策按用户授权替代固定编译器策略，不等于依赖闭包已经锁定或新版兼容性全部通过。
- TypeScript/Tauri CLI：manifest 声明精确本地版本。先前 npm install 和指定包安装因 EACCES 失败；目录整理复验时，用户环境中的十个直接依赖及真实 package-lock.json 已到位，npm ls、typecheck、frontend build、Tauri info 均成功。没有全局安装、虚假 tsc/tauri stub 或绕过类型检查脚本，也未修改依赖版本。
- 目录整理时验证进展：普通 cargo fmt --all -- --check、独立路径 Rust 测试 4/4、npm test 30/30 成功；当时 cargo check/test/Clippy 和 Tauri dev/build 均在 index.crates.io 下载 config.json 时失败。原始结果见 [目录整理复验](docs/validation/organization-2026-11-02.md)。目录整理仅改变工程布局，不新增技术决策或数据迁移。
- 2026-11-02 完整复验：网络错误确认为执行沙箱限制；经批准在沙箱外下载正式 Rust 依赖，生成真实 Cargo.lock，直接版本未变。cargo check、两种 feature 配置的测试各 24/24、Clippy 和正式 Tauri release 构建均通过；Node 30/30 与新增 Vite 监听回归 1/1 通过。当时真实桌面 IPC/smoke 因 UI 工具自动审批检查终止而未完成，历史结果见 [11-02 记录](docs/validation/retest-2026-11-02.md)。
- 2026-11-04 复验：工具链、依赖和 migration 均未变；上述命令行检查再次通过，并以独立测试目录通过真实桌面 IPC、刷新、重启、WebView 重载、第二实例 busy 与锁释放后重试恢复。仅浏览器预览的 URL/窗口读取再次被自动审批检查终止；Phase 2 当前数据层在 Windows 上验收，Phase 1 浏览器专项仍待补。证据见 [本次测试记录](docs/validation/phase12-2026-11-04.md)，不自动推进下一阶段。
- 数据库影响：无 Schema/migration 或业务变更。继续停留 Phase 2 数据层，不实现 Event Service/scanner。

## D026 — Phase 4 metadata 与可再生缩略图

- 日期：2026-11-08；状态：**用户明确授权，已实施**。最新 Phase 4 定义为 metadata/thumbnail，旧浏览/播放路线不随编号获得实施授权。
- 原因：现有 scanner 需要持久 metadata、内容关联的缓存与单文件故障隔离；原 Schema 只有尺寸/时长/path，缺少 codec、生成状态和恢复依据。
- 决策：新增可追踪 0004 migration，保留原 width/height/duration/thumbnail_path；增加基础播放字段与独立可丢弃工作状态。图片和视频统一通过 ffprobe/FFmpeg，无新增 Rust/npm 依赖，Windows 图片强制单文件 demuxer。
- 影响：工具来自 PATH 或 MEDIA_MANAGER_FFMPEG_DIR；missing/failure 是逐项错误，不是启动门槛。一个后台 worker、全局一个进程、短事务，默认 30 秒超时及输出/像素上限；未来播放必须复用共享预算。输出只在应用 cache/thumbnails/，key 包含身份、内容证据、variant 和生成器版本，路径迁移不改变 key。
- 迁移：既有数据库升级前备份，新增列及队列表，为活动媒体排队，旧无版本 thumbnail_path 清空后再生；不改旧 migration、身份/事件/标签/播放进度。内容变化 trigger 失效，发布 CAS 防止迟到 worker 覆盖；文件 miss/中断可恢复，失败显式 retry。LRU、播放器和业务 UI 不属于本轮。
- 验证：临时数据库/合成媒体，真实 FFmpeg/ffprobe 9.0.2；实际结果见 PLANS.md 和本次验证记录。测试工具只放忽略的 .tools，不修改系统 PATH，也不视为发布许可核查。

## D027 — Phase 5 有界 Library 查询与 Custom Tag 编辑

- 状态：用户明确授权；本次 Phase 5 指基础 Media Library，旧 Activity/播放器路线不随编号获得授权。
- 决策：v5 migration 加查询代次和 state+排序索引，keyset 游标绑定 filter/sort/generation；父 System Tag 默认递归包含 descendants，System Tag 仅查询。Custom Tag 编辑事务只修改标签及关联，不触碰文件或事件。
- 理由与影响：后台扫描和标签修改可能使 live cursor 失效，明确 stale_cursor 优于静默漏项；前端每页 100 项、虚拟行/lazy thumbnails、有界批量请求及 200 项选择，避免加载全库原图。搜索为 SQLite 文件名字面 substring，排序提供最近导入、文件名、Like Count。
- 资源边界：Tauri protocol-asset 仅授权应用 cache/thumbnails/*.jpg；新增锁定依赖 http-range 0.1.6，由 Cargo 实际解析下载，其他依赖版本保留。无原件资源 scope、System Tag mutation、Like Event 或播放器 command。
- 验证纪律：mock API 的 DOM 回归与真实 SQLite/Tauri IPC 分别记录。沿用 Phase 1 独立 identifier/appDirectoriesOverride；测试结束关闭应用并确认 1420 释放。UI 夹具等待真实 DOM 更新，不能以内存数据更新代替界面已渲染。详情和结果见 Phase 5 验证记录。

## D028 — Phase 6 只读 Viewer 与独立进度

- 状态：用户明确授权。ImageViewer/VideoPlayer 和 playback_position 本阶段实施，Play Event 明确留到下一 Phase。
- 原因与迁移：既有 playback_states 可复用，但缺少防迟到 owner/sequence；追加 0006，不改旧 SQL/事件表，不迁移或继承替换媒体的状态。media.playback_position 继续只由既有状态 trigger 投影。
- 决策：只读文件句柄 + 短期 opaque token 的异步 media 协议；不扩大原件目录 asset scope。Direct/Remux/Proxy 同一逻辑媒体/owner，缓存版本隔离、后台有界生成与容量回收，活动句柄 pin，原件永不覆盖。
- 保存：5 秒 checkpoint、pause、切换、viewer close、原生关闭前最多 3 秒等待及 pagehide/visibility 尽力写。串行/合并快照，owner+CAS+sequence 防旧会话覆盖，错误可重试。打开/暂停/seek/保存不写事件；completed 资格和计数留给下一阶段。
- 验证：合成媒体与临时数据库、真实 FFmpeg、隔离 Tauri 解码/交互及退出后只读查库；结束关闭窗口/端口并重置截图会话。结果见 PLANS.md 和本阶段验证记录，不把命令行通过冒充 UI 播放通过。

## D029 — Phase 7 事件写入与有界统计修复

- 状态：用户明确授权 Event Log + Counter Cache；沿用已冻结的 activation 和五秒/短片 ended 语义，不采用三秒时间 debounce 或 Like toggle。
- 理由/迁移：v2 已有不可变事件与事务内计数 trigger，但缺少工作会话和重建 delta。追加 0007，为 View 添加 source/viewed_at、建立 activation/player 工作状态及磁盘 base/delta/checkpoint；不修改旧 migration、事件身份或历史时间。既有无 source 的 View 默认 normal_viewer，迁移默认不代表已知历史来源。
- 决策：Like request 幂等；View ordinal/activation 幂等；Play session/sequence/单调正常播放证据达到资格才 INSERT，终结只受限补全同一行。保留既有 trigger 作为计数更新唯一入口，服务不得重复 +1。播放进度仍是独立命令及状态。
- 重建：高水位冻结目标，1000 日志/100 媒体的短事务，事件事务同步 delta，逐媒体发布及后台清理；普通启动只恢复已有任务，不全库重建。启动时旧播放器会话在后台分批以最后持久化事实标 interrupted。
- UI/验证：Like 与 L 共用服务；后台重建状态可见。使用独立合成库和真实 Tauri IPC，100 rapid Likes、故障/COMMIT 回滚、资格边界、重建并发与 100,002 媒体规模实测；最终完整测试及清理证据见本阶段验收记录。原件默认只读、未增加后续浏览模式或文件操作。

## D030 — Phase 8 索引化查询和可复用随机顺序

- 完整查询使用 SQLite 组合条件和 keyset。为新增排序追加 migration 8，保留已应用迁移及事件数据。虚拟键使缺失时间、ASCII 文件名大小写规则和排序索引一致。导入日期输入按用户本地日历的包含端点区间转换为毫秒。
- 任意 seed 的哈希顺序需要先生成排序键。专用查询连接缓存一份磁盘临时索引，首次请求承担建索引成本，热分页复用排名。新增媒体只补新键，过滤条件变化沿用 seed。种子在 WebView 本地设置中持久化，Shuffle Again 是唯一换 seed 入口。
- 目录和标签子串沿用 SQLite 字面匹配，组合条件走关系索引和当前排序索引。以实际查询的 EXPLAIN 和 100,000 媒体实测确认性能。验收结果见 Phase 8 记录。

## D031 — Phase 9 固定成员会话与幂等数字标签

- 状态：用户明确授权 Quick Browse，2026-11-11 实施。沿用稳定 ID/版本、Phase 8 查询、Phase 7 Event Service 和既有只读查看器。
- 原因/迁移：v2 已能保存固定 ordinal，但缺少完整查询排序、生命周期时间、后台构建恢复及数字键重试依据。v9 只追加字段/回执表与索引，旧行回填，保留旧约束和历史关联；不重建媒体表或事件表，不改历史 migration/依赖锁文件。
- 决策：源读快照只流式写 ID/版本至有配额磁盘 spool，释放后按 1,000 项短事务发布；单构建、可取消和超时，building 不可浏览。前端只取当前成员/原件，固定快照不随活动、标签或扫描变化。R 新建 seed/会话，成功发布时原子结束父会话，失败不丢原列表。
- 数字标签使用独立 TagService 用例与事务回执，保证快速连续 Toggle 不合并、IPC 重试不再反转。标签绑定、视频自动播放及默认静音保存为应用设置；默认不自动播放、默认静音。键盘设置中的文本编辑不触发浏览动作。
- 影响：会话的 O(N) 成员落盘成本被后台构建承担；历史快照继续保留，不在本轮猜测保留期或增加恢复历史页面。异常进程中止时 ended_at 可为空；未完成构建下次启动清理，不静默重新查询或洗牌。正常 Esc/关闭保存结束时间和已有播放进度。
- 验证：合成库、故障/取消/恢复及 100,001 成员实测；真实桌面回归包含视频退出的保存顺序回归。结果和截图见 [Phase 9 验证记录](docs/validation/phase9-validation.md)。

## D032 — Phase 10 事件时间报表与当前进度投影

- 状态：用户授权 History、Continue Watching、Dashboard；本轮不推进下一阶段。
- 原因：已有事件/Counter Cache 和独立进度能够支撑报表，但时间范围查询缺少覆盖索引，旧恢复状态的 completed 字段未持续更新。不能加载完整日志到前端，也不能用历史 Play.completed 代替当前恢复位置。
- 决策：v10 仅追加时间索引、进度查询索引、报表代次及当前进度视图，不改旧 migration 或日志。历史范围全部基于 SQLite COUNT/MAX；Dashboard 一致读事务返回汇总及每组最多 6 项。当前 completed 读投影使用批准的 95%/剩余时间公式，无事件写入或全库状态回填。
- 口径：本地日历 Today/含今天的 7/30 日，Play 使用 qualified_at；历史按媒体聚合并保留不可用实例。目录媒体总数含 missing、不含 superseded，Like 总量保留全部日志；Continue Watching 仅 active/current-version。并列按 ID 稳定排序，变更拒绝旧游标。
- 迁移影响：正常升级备份并事务创建索引/视图/触发器，旧事实及恢复行保持原值；无依赖/锁文件变化。实际验证及限制见 PLANS.md 与 Phase 10 验收记录。
- 桌面验收修复：立即 seek/关闭时，活动事务可阻止 DEFERRED 进度事务升级；进度保存先获取 IMMEDIATE 短写事务，随后校验原 owner/CAS 并保存。150ms 并发写锁回归修复前稳定失败、修复后通过；不增加 Play 或放宽幂等条件。

## 后续事项

版本/驱动选型在 Phase 1 锁定；平台播放 profile/许可、会话保留/恢复页面及进一步规模实测留待后续单独授权。未来 manual relink、原文件编辑/删除或内容聚合必须另立决策和迁移，不是当前功能。当前无待冻结的身份或 Like/View/Play 核心业务语义。
