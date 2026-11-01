# 系统架构设计

## 当前基线与本轮变更理由

身份、重复文件、保守匹配、Like、Activation View、Qualified Play、播放状态、completed、三层播放及 100,000+ 架构已获用户批准。本文是据此维护的设计基线，不代表已经实现；本轮只更新五份 Markdown，不开始 Phase 1。具体依赖版本与平台参数在对应 Phase 验证。

本轮必须修订旧提案的理由：
1. 不确定移动无限 pending 不符合批准的 false-negative 优先策略，改为对账结束后 missing + new。
2. Like 撤销/delta 与产品冻结语义冲突，移除相关载荷和约束。
3. View 改为 activation；Play 改为 5 秒/短片真实结束，废止 3 秒候选。
4. Qualified Play 插入时播放器尚未结束，须允许受限补全同一事件，不能再禁止其所有 UPDATE。
5. 单一全库重建写事务会长期阻塞写入，改为高水位、后台聚合及分媒体短事务发布。
6. 新增 remux/proxy 架构及缓存。当前无真实数据库，无数据迁移要执行；初始 migration 按新 Schema 创建，未来变更须正式迁移，禁止临时改库。

## 拟定项目结构

以下是未来目录，不在本轮创建：

```text
/
├── AGENTS.md / REQUIREMENTS.md / ARCHITECTURE.md / PLANS.md / DECISIONS.md
├── README.md                         # 初始化后记录真实命令
├── src/
│   ├── app/                          # 启动、路由、全局错误
│   ├── api/                          # 唯一类型化 IPC/通知入口
│   ├── components/                   # 通用展示、虚拟 Grid
│   ├── features/
│   │   ├── library/                  # SQLite 分页查询结果
│   │   ├── viewer/                   # 主媒体 activation、播放器适配
│   │   ├── activity/                 # 主动 Like、资格上报
│   │   ├── browse-session/           # 固定成员顺序与当前项
│   │   ├── tags/                     # System / Custom 分离
│   │   └── jobs/                     # 后台任务进度与取消
│   └── test/
├── src-tauri/
│   ├── src/
│   │   ├── commands/                 # 参数解析、权限、DTO/错误映射
│   │   ├── domain/                   # ID、事件、规则、不变量
│   │   ├── services/                 # 按用例拆分，非单一大服务
│   │   ├── db/                       # repository、写协调器、迁移
│   │   ├── scanner/                  # 枚举、差异、收尾
│   │   ├── identity/                 # 证据与保守 reconciliation
│   │   ├── metadata/                 # 图片/ffprobe
│   │   ├── thumbnails/               # 缓存与后台生成
│   │   ├── playback/                 # Direct/Remux/Proxy、资源访问
│   │   ├── browse_sessions/          # 快照、seed、排序
│   │   ├── statistics/               # 核验、分批重建
│   │   ├── jobs/                     # 有界队列、取消、恢复
│   │   └── platform/                 # 路径、文件 ID、进程/权限
│   ├── migrations/                   # 编号、校验和、不可改已应用文件
│   ├── capabilities/                 # 最小权限
│   └── tests/
└── tests/
    ├── fixtures/                     # 合成或许可明确的小型媒体
    └── e2e/
```

运行数据在应用数据目录：library.sqlite3、cache/thumbnails/、cache/playback/、临时 spool 和受控诊断日志。不得写媒体目录；本地数据库不放网络共享文件系统。显示路径与用于比较的规范化路径键分离，原生路径编码由 platform 模块封装。

## 模块边界与资源预算

React → src/api → Tauri commands → Rust services → repository / worker 接口。React 不直接 SQLite，不开放 SQL 插件、通用 SQL、任意文件读写或 shell。命令层不写 SQL，repository 不决定资格规则。

| 服务 | 职责 | 边界 |
| --- | --- | --- |
| CatalogQueryService | 查询、keyset、DTO | 不扫描或解码 |
| ScanService + IdentityResolver | 增量编排、证据、位置事务 | 不修改事件或标签历史 |
| TagService | Custom Tag 与目录投影查询 | 不改物理文件 |
| ActivityService | Like/activation/player session、资格、事件事务 | 不用进度保存触发计数 |
| PlaybackStateService | 位置、CAS、owner 接管 | 不插入 play_events |
| PlaybackService | Direct/Remux/Proxy、资源句柄 | 不改变媒体身份与 session |
| BrowseSessionService | 固定成员、seed、序号 | 不在分页时重新洗牌 |
| ThumbnailService | 缓存与生成 | 不承担身份匹配 |
| StatisticsService | 日志核验、重建 | 不改写事件计数事实 |

写协调器只调度短事务，不成为业务服务定位器。跨 repository 的业务事务由对应服务一次编排，禁止嵌套提交及循环服务依赖。文件 I/O、哈希、FFmpeg 都在事务外、有界 worker/子进程执行。

初始可配置工程默认值：扫描根并发 1、文件检查 worker 2、完整哈希每卷 1、thumbnail worker 2；remux/proxy 共享一个播放生成槽位，默认 1，可配置 1–2；全局 FFmpeg 子进程预算默认 2，由缩略图与代理共享，不能各自绕开。队列分页入库、内存只保留有限待执行项（初始 128），上限时施加背压。前台查询/活动写入高于后台批次；所有数值是可调资源参数，不改变产品语义。

## Identity 与 reconciliation

media.id 是稳定不透明 UUID，表示逻辑媒体文件实例；不是 path，也不是 fingerprint。不同独立文件即使完整内容相同也保留不同 ID。摘要索引为非唯一，算法/版本与内容版本分别存储。

同路径先比较身份连续性证据；不能仅凭路径、mtime、size 或 file ID 继承。volume + file ID + 可用 birth/change 信息、唯一候选、旧位置消失及无复用/替换迹象可形成高置信度同卷连续性。摘要只是辅助，不能证明复制后删除与移动的区别。硬链接/不支持的链接场景不自动聚合为一个实体。

高置信度 rename/move：在同一位置事务退役旧位置、登记新目录与位置，media_id、imported_at、事件、Custom Tags、同版本 playback state 不变；更新路径、filename、last_seen_at、stat 与目录投影。file_path 和 filename 从当前 location 返回，不在身份表维护另一个可冲突的路径副本。

跨根同卷移动可在有界 reconciliation 窗口内协调观察代次；提交时重新校验旧/新位置及内容版本。**对账结束后仍无高置信度：旧 media missing、新文件 new ID**，保留判定理由，不无限 pending、不自动 relink。根离线/权限失败只标 unavailable，不能无证据把旧媒体改 missing。manual relink 为未来功能，当前无命令。

同路径已确认不同内容媒体：退役旧 location，旧 media.state=superseded；创建新 ID、imported_at、初始版本、零计数，不复制标签和进度；事务内释放活动路径唯一槽位后登记新文件。旧日志和状态完整保留。经高置信度证明仍属同一逻辑实例的内容变化才增加版本；证据不足按新媒体处理，不让“version bump”绕过替换规则。

### Fingerprint 成本

普通扫描先 stat，未变文件不读取内容；不在启动/查询/播放前同步整文件哈希。快速指纹 v1 使用 size 与首/中/尾各最多 1 MiB 的非重叠采样，记录采样偏移和版本；<=3 MiB 一次顺序读取。大视频快速核验最多读 3 MiB，但网络盘 seek 延迟仍需测量。

完整 SHA-256 流式、可取消、按卷限流，仅按需用于候选佐证、明确重复检测或深度核验。首次导入不强制完整哈希所有视频；结果绑定内容版本，前后 stat 改变则丢弃重试。旧文件已消失且没有完整摘要时不能补造证据，最终按 missing + new。不能因 full hash 相同自动合并。

普通增量不保证发现所有同大小同时间戳改写；显式深度核验作为后台任务。False negative is preferable to false positive，误分成两项的成本优于污染历史。

## Incremental Scanning 与 System Tag

每根一个运行代次、同根最多一个扫描提交者。分批枚举、对比已存观察数据，变化项才进入昂贵处理；worker 结果携带媒体/版本/扫描代次，陈旧结果拒绝提交。取消或崩溃保留已提交批次，遗留 running 标 interrupted，恢复走同一增量流水线。

| 结果 | 判定与动作 |
| --- | --- |
| new | 初次发现或有界对账后未可靠归属现有实例；新 ID、新 imported_at、零计数，无继承。 |
| existing | 身份连续性成立，位置/内容未变；更新 last_seen_at。 |
| moved | 高置信度同一实例换父目录/根；保留历史、更新 System Tag。 |
| renamed | 高置信度同父目录改名；改目录同时改名可同时记录 moved+renamed。 |
| modified | 内容变化观察；高置信度同实例则新版本，确认不同媒体/证据不足则 replacement/new，不继承。仅 mtime 改变而内容相同只改 stat。 |
| missing | 完整枚举父目录并核实旧实例消失，无已确认新位置；保留 tombstone 和关联。 |
| restored | missing 实例高置信度重新出现；保持 ID/历史；不可靠则作为 new，不自动恢复旧关联。 |
| pending | 仅供尚在扫描/读文件/有界对账中的临时状态；完成后产生 matched 或 conservative split 结论。 |

只有成功完整枚举的子树可据“本代未见”判 missing。离线、权限失败、取消及崩溃不能批量误标。扫描不是文件系统原子快照，收尾重查关键位置，监听溢出/竞争由后续增量纠正。默认不跟随 symlink/junction、拒绝重叠根；异常文件独立记录失败。

System Tag 来自 directories 和当前 location，directory_closure 仅为真实父子关系派生的可重建索引。历史位置不成为当前标签；离线显示最近成功扫描状态。Custom Tag 另表关联 media_id，操作不触及文件系统。目录重命名只由扫描观察更新，不存在编辑 System Tag 来移动文件的 API。

## 总体 Schema（逻辑设计，不是实际 DDL）

ID 使用 TEXT UUID；时间点 UTC 毫秒 INTEGER；媒体位置/时长和观看时间使用有单位的 REAL seconds，输入拒绝 NaN/无穷/负数。content_version_id 为每个 media 内递增正整数，复合键 (media_id, content_version_id) 唯一确定版本。FK 默认 ON DELETE RESTRICT，布尔 CHECK IN (0,1)。? 表示可空。

| 表 | 主要字段与约束 |
| --- | --- |
| schema_migrations | version PK, checksum, applied_at；已应用脚本不可改。 |
| db_sequence | id=1 PK, value INTEGER；仅写事务分配单调 change sequence，不用时钟充当高水位。 |
| catalog_state | id=1 PK, query_generation INTEGER；live cursor 一致性代次。 |
| library_roots | id PK, path_display, path_key UNIQUE, volume_key?, status, created_at。 |
| directories | id PK, root_id FK, parent_id FK?, relative_path_key, display_name；UNIQUE(root_id,relative_path_key)，同根/无环。 |
| directory_closure | ancestor_id FK, descendant_id FK, depth；PK(ancestor_id,descendant_id)，depth>=0。 |
| media | id PK, kind, state CHECK(active/missing/superseded), current_content_version_id, imported_at, created_seq UNIQUE, last_seen_at, like_count/view_count/play_count INTEGER DEFAULT 0 CHECK>=0, updated_at。 |
| media_revisions | media_id FK, content_version_id, observed_size_bytes, created_at；复合 PK，版本不复用。 |
| media_locations | id PK, media_id FK, content_version_id, root_id FK, directory_id FK, relative_path_key, path_display, filename, volume_key?, file_key?, size_bytes, mtime_native, availability, last_seen_run_id FK?, last_seen_at, retired_at?。 |
| media_metadata | media_id, content_version_id, width?, height?, duration_seconds?, codec?, container?, rotation?；复合 PK/FK。 |
| media_fingerprints | media_id, content_version_id, algorithm, algorithm_version, kind(quick/full), digest, computed_at；复合 PK(媒体/版本/算法/算法版本/类型)，digest 及包含 digest 的内容键不设 UNIQUE。 |
| scan_runs | id PK, root_id FK, generation, mode, status, started_at, finished_at?, error_code?；UNIQUE(root_id,generation)。 |
| scan_directories | run_id FK, directory_id FK, enumeration_status, error_code?；复合 PK，complete 才支持缺失判定。 |
| scan_candidates | id PK, run_id FK, directory_id FK, relative_path_key, observed_metadata, resolution_status, reconciliation_deadline?。 |
| scan_candidate_matches | candidate_id FK, media_id FK, rule_version, evidence_kind, evidence_payload；复合 PK(candidate_id,media_id,evidence_kind)。 |
| identity_resolutions | id PK, candidate_id FK, old_media_id FK?, resulting_media_id FK?, rule_version, decision(matched/new/conservative_split/replacement), evidence_payload, decided_at；无当前人工 relink 入口。 |
| custom_tags | id PK, name, normalized_name, created_at；重名/层级政策后续用正式 migration 收紧。 |
| media_custom_tags | media_id FK, tag_id FK, created_at；复合 PK。 |
| viewer_activations | id PK, creation_request_id UNIQUE, viewer_session_id, media_id FK, surface, ordinal, activated_at, closed_at?；UNIQUE(viewer_session_id,ordinal)。 |
| like_events | id PK, ingest_seq UNIQUE, request_id UNIQUE, media_id FK, occurred_at, recorded_at, policy_version；无 delta、撤销或 toggle 字段。 |
| view_events | id PK, ingest_seq UNIQUE, request_id UNIQUE, media_id FK, activation_id FK UNIQUE, occurred_at, recorded_at, policy_version。 |
| player_sessions | id PK, creation_request_id UNIQUE, media_id FK, content_version_id, activation_id FK, policy_version, state, opened_at, started_at?, ended_at?, start_position?, last_position?, watched_seconds DEFAULT 0, duration_seconds?, real_ended DEFAULT 0, last_sequence, last_evidence_at?。 |
| play_events | id PK, ingest_seq UNIQUE, request_id UNIQUE, media_id FK, content_version_id, player_session_id FK UNIQUE, policy_version, started_at, qualified_at, ended_at?, start_position, end_position, watched_seconds, completed, end_reason?；只在资格达成后存在。 |
| playback_states | media_id, content_version_id, position_seconds, duration_seconds?, updated_at, completed, state_version, owner_session_id FK?, last_sequence；复合 PK/FK。 |
| browse_sessions | id PK, creation_request_id UNIQUE, seed?, order_algorithm_version, filter_snapshot, sort_snapshot, snapshot_at, status, item_count, current_ordinal?, expires_at?；Random 必须 seed。 |
| browse_session_items | session_id FK, ordinal, media_id FK, content_version_id, frozen_sort_key；PK(session_id,ordinal)，UNIQUE(session_id,media_id)。 |
| thumbnail_entries | cache_key PK, media_id, content_version_id, variant, generator_version, status, relative_cache_path?, byte_size?, last_accessed_at?。 |
| playback_cache_entries | cache_key PK, media_id, content_version_id, mode(remux/proxy), profile_version, status, relative_cache_path?, byte_size?, last_accessed_at?。 |
| jobs | id PK, kind, status, dedupe_key, payload_version, payload, attempts, progress, progress_version, error_code?, created_at, updated_at；活跃 dedupe_key 唯一。 |
| statistics_rebuild_jobs | id PK, scope, target_media_id FK?, high_watermark, state, per_log_cursors, publish_cursor?, created_at, updated_at；全库最多一个重建，单媒体请求也经同一协调协议。 |
| statistics_rebuild_base | rebuild_id FK, media_id FK, kind, count；PK(rebuild_id,media_id,kind)，磁盘聚合，不是新事实来源。 |
| statistics_rebuild_deltas | rebuild_id FK, media_id FK, kind, count；同上 PK，记录重建期间已提交的新增事件计数。 |

所有版本引用用复合 FK 到 media_revisions；media 当前版本 FK 延迟到事务提交校验，初次建 media/revision 同事务完成。事件、activation、player session 的媒体/版本/类型归属也用复合 FK 或等效触发器约束，不能只靠 UI。created_seq 和事件 ingest_seq 均由 db_sequence 在对应业务事务内分配，回滚时一并回滚；计数重建不依赖毫秒时间排序。

media 每实例一个未 retired 位置，活动路径 (root_id,relative_path_key) 部分唯一；missing 位置保留占位，路径被新实例替代时先退役旧位置。availability 区分 available/missing/unreadable/unavailable；media.state 表示身份实体状态，根离线不擅自改为 missing。不同路径的独立文件不因 hash 或 file ID 聚合。

### 索引、启动与 migration

- 至少索引：当前位置 (directory_id,availability,media_id)、(root_id,last_seen_run_id)、目录 parent_id、closure(descendant_id,ancestor_id)、标签(tag_id,media_id)、fingerprint(kind,algorithm,algorithm_version,digest) 非唯一；事件(media_id,ingest_seq) 与 ingest_seq、任务(status,kind)、会话项(session_id,ordinal)、LRU(status,last_accessed_at)。
- media(imported_at,id)、(like_count,id)、(view_count,id)、(play_count,id) 等按实际批准排序建立；组合过滤用 EXISTS/半连接避免关系行膨胀，逐项 EXPLAIN QUERY PLAN，不承诺所有组合覆盖索引。
- 每连接显式 foreign_keys=ON，数据库本地 WAL、写入串行，busy_timeout 初始 2000ms；超时有界重试或返回 retryable busy，不阻塞 UI 线程。事件优先 FULL 同步；WAL/驱动版本初始化时锁定和验证。
- 普通启动先打开已有 DB、校验/执行必要 migration，再让 UI 查询；后台恢复 interrupted 工作并安排增量扫描。非破坏性启动不重建统计/全库，扫描完成不是 UI 开放条件。必要 migration 若耗时需可响应升级界面，不静默新建空库。
- migration 持有升级锁、检查已应用 checksum，编号/时间可追踪；事务化结构变更与版本记录原子提交。升级前做 SQLite 一致性备份，不能只复制运行中的主 DB 文件。失败停止写入、保留历史并可恢复备份；不兼容新版拒绝写入。
- 大表用正式 expand/backfill/validate/contract 和可恢复 checkpoint，不通过临时改列绕过版本；只承诺从已支持版本前向升级，默认不承诺任意降级。测试空库、旧库、中断、校验失败和引用完整性。

## 事件、会话与 Counter Cache

### Like 和 View

每次主动 Like 新 request_id，一条不可变 like_event，计数 +1；相同动作重试复用 request_id，多次动作绝不合并。移除 delta/reverses_event_id，不提供 Unlike。

Viewer 的导航控制器持有 viewer_session_id 与 activation 序号，而非 React render/effect 分配业务身份。真正切换主媒体时发 activate_media，Rust 按 creation_request_id 幂等建立 activation，**同一事务建立 view_event 并更新 view_count**。A→B→A 为三个 activation；rerender/resize/fullscreen/恢复订阅复用 activation。Grid 预加载不调用 activate；Quick Browse 变更当前项时调用一次。Rust 校验 viewer 当前序号，拒绝陈旧导航，通知不能重建一个 activation。

### Qualified Play 生命周期

Player session 是某媒体的一次播放器生命周期，不是一次按钮点击，也不是浏览顺序快照。pause/resume/seek/循环/同 session 内重播沿用 ID；切换媒体、关闭或显式 end 才关闭 session。回到该媒体的新 player session 可产生新事件。Direct→Remux→Proxy 切换只换资源，沿用 activation 和 player session。

ActivityService 接收有 sequence 的播放适配证据，去重/拒绝陈旧片段，用单调经过时间与正常播放状态交集累计 watched_seconds，限制每段不能超过真实经过时间。暂停、buffering、seek、恢复、仅进度差不增加观看时间；2x 播放使用实际播放经过的时间，不把大幅位置跳跃当观看。WebView 上报不是抵御篡改客户端的可信证明，后端负责正常客户端下的顺序、状态和范围校验。

规则：duration>=5 秒（或未知）累计真实播放 >=5 秒 qualify；0<duration<5 秒真实播放至 ended qualify，seek/恢复到尾本身不能触发。completed 与 qualification 分开，95% 不等于短片 ended。打开视频只有 activation/view，不生成 Play。资格达成时插入唯一 player_session_id 的 play_event，started_at 是首次真实播放，qualified_at 是达标时刻。

资格后关闭/切换时受限补全同一行 ended_at、end_position、watched_seconds、completed、end_reason；不会增加第二条事件。资格前结束只关闭 player_session，无 play_event。异常退出由下次恢复将未关闭 session 标 interrupted，仅用最后持久化证据补全，不能虚构未保存播放时间。

### 原子写入协议与日志保护

1. 在写协调器执行 BEGIN IMMEDIATE，事务内读取身份/会话及幂等状态。
2. request_id 已存在且语义载荷相同则返回已有结果，不再计数；冲突载荷拒绝。View activation_id、Play player_session_id UNIQUE 还防止换 request_id 重复记录。
3. 插入事件，分配 ingest_seq；同事务更新 media 对应 Counter Cache +1。若统计重建 active，同事务写入对应重建 delta；任何一步或 COMMIT 失败整体 rollback，未提交不得应答成功。
4. commit 后才返回/发通知；通知失败不回滚事实，重试/快照查询恢复。整数溢出等错误显式返回，不饱和截断，不忽略约束错误。

like_events/view_events 禁止 UPDATE/DELETE。play_events 的 id、媒体/版本、session、ingest_seq、started_at、qualified_at、request_id 等计数事实不可改、禁止 DELETE；仅允许从未结束到已结束的一次终结补全，watch 总量不得减少，重试一致则幂等、冲突则拒绝，结束后整行冻结。服务和触发器双重约束；终结事务只更新结束元数据，不修改计数，也不分配新计数事件。此规则替代“任何日志字段都不得更新”的旧设计，使一 session 一事件与完整生命周期兼容。

Source of Truth 始终是三个事件表；player_sessions 是资格工作状态，playback_states 是恢复状态，均不能替代日志推导统计。终结时 completed 根据该 session 最后有效位置计算；不读取另一个播放器可能已更新的 playback_states。

### 核验和 bounded statistics rebuild

公式：like_count=COUNT(like_events)，view_count=COUNT(view_events)，play_count=COUNT(play_events)。play_events 只有 qualified rows，不能统计按钮点击、未资格 session 或终结次数。没有事件的媒体重建为 0。check_statistics 在同一读快照比较日志与缓存，报告差异而不篡改事件。

必须有 rebuild_like_count(media_id)、rebuild_all_like_counts()、rebuild_statistics(scope=all|media|kind)；均为后台 job，普通启动不调用。单媒体也不能因事件量大而独占长期写事务。

全库重建协议（替代全局单事务）：
1. 短写事务创建 active rebuild，记录全局 sequence 高水位 H。目标媒体为 created_seq<=H 的稳定集合（或其中指定 ID），新增媒体不需要重建；并发事件从此同事务累计重建 deltas。
2. worker 按各日志 ingest_seq keyset 读取 <=H 的事件，每批初始最多 1000，聚合到磁盘 base。每批累加与扫描 checkpoint 同事务，崩溃不会重复累加。查询用短读快照，内存与事务批次有上限，可暂停取消。
3. base 完成后按 media.id keyset 每批初始 100 个目标媒体发布。在短写事务里读取 base+当前 deltas，更新该媒体的所有目标统计与 publish_cursor；零值同样写入。发布期间事件插入不能穿过写事务，之后事件仍正常 +1，所以不会漏计或双计。此前已发布媒体不再重复覆盖。
4. 事件继续为 active job 写 delta，即使该媒体已发布也不影响正确性，因为该媒体不会再发布；最后短事务标 complete，停止捕获，后台清理 scratch。新媒体 created_seq>H 始终由正常事件事务保持计数，不纳入旧目标。
5. 崩溃续跑按持久化 checkpoint；取消与停用 delta 捕获在同一短事务完成，保留已修复的媒体，不影响日志/正常计数；再次重建用新 H。

全库修复是逐媒体发布，不宣称整个库瞬时全有或全无；单个媒体的目标计数一起原子切换，UI 显示 job 状态，不能把进行中的修复称为全部完成。scratch/delta 只作可丢弃派生数据。写协调器在批次间优先前台事务，监测事务时长并缩小批次；海量单媒体也按日志批处理，禁止整库计数先载入内存或长时间占写锁。磁盘不足/失败须显式停止，既有日志不受影响。

## Playback State 与 completed

PlaybackStateService 独立写 (media_id,content_version_id)，至少 position_seconds、duration_seconds、updated_at、completed；无插入 play_events 权限。定时节流、pause/seek/切换/正常退出尽力刷新；异常退出可能损失最后一个保存间隔的位置，但不能损失已提交事件。

state_version + owner_session_id + last_sequence 拒绝旧 owner/旧版本 heartbeat；显式接管才能换 owner，不用前端时间戳 last-write-wins。同版本移动保留状态，替换新媒体不继承；旧版本位置保留但不能静默应用到新版本。保存位置/恢复位置与资格上报是不同命令路径。

duration>0 且有效时 position 限制在 [0,duration]，progress=position/duration，remaining=duration-position：
completed = progress >= 0.95 OR remaining <= min(10, duration*0.05)。
无效/未知时长 completed=false，获得时长后重算；duration=2 秒时 1 秒处不完成，1.9 秒处完成，但只有真实 ended 才使短片 qualify。seek 可改变 completed，不直接产生 Play。

## 三层播放与 Thumbnail/Playback Cache

PlaybackService 先读 metadata 和目标 WebView 能力，再使用实际加载/播放错误校正：
- Direct Play：原始媒体受限只读资源句柄，支持 range。
- Remux：codec 可用、container 不兼容时优先 FFmpeg stream copy，不重新编码。remux 失败或输出仍不支持才升级 proxy。
- Transcode Proxy：按需生成兼容资源。D015 记录首选 H.264 + AAC MP4、yuv420p、适合流式起播的封装；具体编码 profile/分辨率/硬件加速按平台验证，不保证所有 WebView 无条件支持。

缓存逻辑键含 media_id、content_version_id、mode/variant、generator/profile version；示例 cache/playback/<media_id>/<content-version>.mp4 为默认 profile 的路径，多 profile 必须另目录或加版本后缀防互相覆盖。代理派生于原内容，不创建新 media，不改变播放计数归属、版本或位置时间线；代理时长/时间基与原内容对齐，超出可接受偏差则报失败而非保存错误进度。

生成先写缓存内临时文件，完整验证后原子 rename 再标 ready；生成期间 UI 展示 pending，可取消，不要求 UI 等待。不能把未完成普通 MP4 伪装为可 seek 的 ready 文件。全局 FFmpeg 和专门播放槽位限流、同键去重。原文件不作为输出目标；参数数组启动、不经 shell，限制本地协议、超时、像素/输出上限；超时/取消终止回收子进程，单文件失败隔离。

thumbnail 和 playback_entries 是可丢弃派生数据，文件与 SQLite 没有共同事务；ready 丢文件视为 cache miss 重建，孤儿临时文件可清理。内容版本变化后旧键不可作为当前资源；LRU 按容量/配额分批回收，播放中句柄 pin 住文件，先释放后删除。清理严格限制应用 cache 子目录并验证链接/路径边界，绝不删原媒体。Direct/Remux/Proxy 全失败返回可恢复错误，不崩溃或偷偷修改原文件。

## 100,000+ 查询、Grid、Random 与 Quick Browse

默认 page=100，上限 200；(sort_value,media_id) keyset cursor 绑定过滤/排序/版本/查询代次，固定 NULL 规则。深分页不用巨大 OFFSET，不把全库对象传前端。受影响目录/标签/计数等事务递增 query_generation；live 游标过期显式 stale_cursor，避免悄悄跳项。sorting/filtering/search 在 SQLite 进行，搜索字段/FTS 选型 Phase 4 按需求定案。

Grid 必须 virtualization，有限 overscan、懒加载缩略图、有界页面 LRU、取消过期请求；列表 DTO 只有卡片所需字段，批量查询关联，不每格 IPC，不解码所有缩略图。离屏播放器卸载/暂停，回收资源句柄。精确总数可独立按需异步查询。

Random 保存 seed 与算法版本；排序 v1 为 SHA-256(domain/seed/media_id 的长度前缀编码)，摘要字节排序、media ID tie-break。相同成员+seed+版本顺序相同，不能每页 ORDER BY RANDOM 或 React render 洗牌。

Quick Browse 后台读一次有限生命周期快照，将成员/版本/排序值写临时 spool 后释放读事务；有界/外部排序再按 ordinal 分批持久化 building 会话，全部完成短事务变 ready。只返回页，不返回全库 DTO；不让长时间用户浏览 pin 住 SQLite 读事务。会话构建 O(N) 磁盘与排序成本，必须有可取消 job、并发/磁盘配额，不能假装任意 seed 排序都能走 media 索引。

ready 成员/ordinal 不变，点赞/标签/扫描不重排；新增不插入，missing/superseded 项保留序号并标不可用。内容已变化不能冒充原版本播放。主动改过滤或 reshuffle 创建新会话；重开同 session 可恢复 seed/顺序/当前项。到期返回 session_expired，不静默换顺序；保留期和恢复 UX 在 Phase 4 完成，不阻止基础架构。

## Tauri Command / API 边界

DTO 版本化，TypeScript strict，Rust 最终校验。ID/计数/sequence 用无损字符串跨 IPC，seconds 使用有限数值。错误形状 {code,message,retryable,requestId?,jobId?}，稳定区分 invalid_input/not_found/permission_denied/unsupported_media/stale_cursor/conflict/busy/cancelled/migration_failed/internal；不向 UI 泄露 SQL/堆栈或多余私人路径。

| API | 输入 → 输出 | 权限与事务 |
| --- | --- | --- |
| choose_library_root / register_library_root | 原生选择 → 短期令牌 → Root DTO | 后端授权、规范化，拒绝越界/重叠。 |
| list_library_roots / get_directory_tree | 有界查询 → 状态/节点 | System Tag 只读，无物理 rename/delete。 |
| start_scan / get_job / list_jobs / cancel_job | root/范围/request_id 或 job_id → job/状态 | 排队立即返回，范围不可越界。 |
| list_media / get_media | 过滤、排序、cursor/limit 或 ID → DTO | 参数化 SQL、排序白名单，不暴露任意 SQL。 |
| create_custom_tag / set_media_custom_tags | 名称/ID 关联 → DTO | 只改逻辑标签。 |
| record_like | media_id、request_id → event_id、权威计数 | 每动作一事件，无 unlike/toggle。 |
| activate_media / close_activation | viewer_session、ordinal、media_id、surface、request_id → activation/计数 | activation 建立 + View + count 同事务；相同激活重试不重复。 |
| open_player_session / report_playback / close_player_session | activation/版本/request_id、sequence、正常播放证据 → session/资格结果 | ActivityService 资格入口；不能直接请求任意 play_count 增量。 |
| get_playback_state / save_playback_state / claim_playback_owner | 媒体/版本、位置/时长、expected_version/owner/sequence → 状态 | 独立状态路径，不创建 Play。 |
| resolve_playback | media_id/版本 → direct/remux/proxy ready 句柄或 pending job | PlaybackService，不产生新的 activation/session。 |
| get_thumbnail | media/variant → 受限资源或 pending/failed | 缓存 miss 入队，不经 IPC 传大二进制。 |
| create/get/list_browse_session_items | request_id/冻结配置/seed，session_id/ordinal/方向/limit → job/快照/页 | 同请求幂等，ready 不可就地洗牌。 |
| rebuild_like_count / rebuild_all_like_counts / rebuild_statistics | media_id?/scope/request_id → job_id | 显式后台维护，统一高水位协议。 |
| check_statistics | 范围/request_id → job_id | 同快照只读差异查询。 |

内部 record_view/qualify_play 不注册为可绕过 activation/session 的 Tauri command。资源句柄绑定授权根、媒体、位置与内容版本，受限协议提供 range；不提供可拼接任意路径。capabilities/来源/窗口权限与 Rust 路径校验共同生效。

通知 {version,topic,jobId?,entityId?,sequence,status,payload}，包括 job.progress/finished、library.changed、thumbnail.ready、playback.ready、media.counters_changed。事务提交后才发；通知不是持久化业务 Event Log。不保证跨任务全局顺序或 exactly-once，前端先订阅再查快照，按序去旧，漏消息靠查询恢复，组件卸载必须取消订阅。

## Architecture Review — 当前 15 项

本表取代上轮 20 项审查的 NO 结论；旧问题与修改理由保存在 DECISIONS.md。通过表示设计检查通过，不表示实现/性能测试通过。

| # | 审查项 | 当前结论 |
| --- | --- | --- |
| 1 | 身份是否依赖 path/fingerprint | 通过；逻辑实例 UUID，路径/摘要仅位置/证据。 |
| 2 | Duplicate content | 通过；不同实例不合并，digest 非唯一。 |
| 3 | rename/move 历史 | 通过；高置信度切换位置，保留 imported_at、所有关联和进度。 |
| 4 | ambiguous move | 通过；有界对账后 missing+new，manual relink 不在当前 Phase。 |
| 5 | same-path replacement | 通过；旧 superseded、新 ID/导入时间/零计数，无继承。 |
| 6 | Like 冻结 | 通过；每主动动作一条正向事件，无 toggle/撤销。 |
| 7 | View activation | 通过；A→B→A 三次，rerender/预取不计数，activation UNIQUE。 |
| 8 | Play qualification | 通过；真实 >=5 秒或短片真实 ended，每 player session 最多一次。 |
| 9 | Playback state 独立 | 通过；独立表/服务/CAS，不通过保存进度插事件。 |
| 10 | completed | 通过；95% OR remaining<=min(10s,5% duration)，不是资格。 |
| 11 | Event Log 事实来源 | 通过；统计只读三个日志，Play 仅允许一次终结补全。 |
| 12 | Counter transaction/rebuild | 通过；插事件+计数+重建 delta 同事务，有界后台核验/重建。 |
| 13 | Playback fallback | 通过；Direct→Remux→Proxy，版本缓存、只读原件、全局限流。 |
| 14 | 100,000+ 可执行设计 | 通过；keyset/索引/虚拟化/lazy load/独立有界队列；后续实测验收。 |
| 15 | Phase 1 业务 blocker | 无；未决定的展示/平台 profile/预算属于后续细化，不改变已冻结模型。 |

**Ready for Phase 1: YES（仅基础阶段设计就绪）**。用户本轮明确禁止开始实现，因此 Phase 1 仍未开始。无需等待性能实测或完整页面清单才能设计工程骨架；版本/驱动初始化选择属于 Phase 1 工作，不能擅自改产品语义。

## Remaining risks 与后续验证

- 高置信度证据依赖平台文件标识质量；保守分离可能多出媒体项，这是批准的取舍，不得以 hash 去重“修复”。
- 同大小同时间戳改写普通扫描可能漏检，显式深度核验成本需测量；根离线不能误标全库 missing。
- WebView codec、HDR/音轨/字幕、时间基和 remux 兼容性按平台矩阵验证；H.264/AAC 编码器构建许可和可用性发布前核验。
- 实际播放证据受 WebView 采样精度与异常退出影响；严格去重，宁不虚构观看量。终结补全需测试中断/重试/触发器约束。
- 重建高水位、base/delta/checkpoint、并发事件与取消恢复需故障注入；scratch 磁盘占用和批次延迟需要规模测量，不回退长期全库写事务。
- Quick Browse 构建会占 spool/WAL/磁盘，必须限额与清理；100,000+ 查询索引组合、内存、冷/热响应和代理吞吐在后续 Phase 验收。
- migration、路径编码/大小写/链接越界、坏文件与缓存孤儿恢复必须测试。以上是工程风险，无当前 Phase 1 未解决的业务阻塞项。

## 技术依据

- [Tauri Capabilities](https://tauri.app/security/capabilities/) 与 [Command Scopes](https://tauri.app/security/scope/)：权限配置及命令内部校验。
- [SQLite Transactions](https://www.sqlite.org/lang_transaction.html)、[WAL](https://www.sqlite.org/wal.html)、[Foreign Keys](https://www.sqlite.org/foreignkeys.html)：单写者、事务与连接设置。
- [SQLite Row Values](https://www.sqlite.org/rowvalue.html)、[Query Planner](https://www.sqlite.org/queryplanner.html)、[Online Backup](https://www.sqlite.org/backup.html)：游标、索引与一致性备份。
- [FFmpeg Streamcopy / Transcoding](https://ffmpeg.org/ffmpeg.html)：remux 与重编码的不同处理路径；具体缓存/调度方案为本项目设计。
- [React StrictMode](https://react.dev/reference/react/StrictMode)：开发期额外 render/effect 要求业务 activation 与组件生命周期分离。
