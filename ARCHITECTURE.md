# 系统架构设计

## 当前基线与本轮变更理由

### Phase 12 Folder Import 设计与 migration plan（实施前）

本次用户授权将“扫描外部路径”扩展为“复制入托管区并保留来源”。现有 ScanStore 用于物理对账及 missing 发布，不能拿来表示预览或来源目录，因此增加按用例拆分的 ImportService/ImportStore/ManagedStorage，复用 Scanner 的格式分类、Fingerprint 模块、MediaRepository 的事务插入、System Tag 观察约束、既有 thumbnail worker 和 app_settings。Event Service 不进入导入调用链。

1. **身份与内容**：只在 Folder Import 使用精确 filename+size+完整 SHA-256 复用。扩展现有 scanner::fingerprint 提供有界完整摘要/复制摘要，保留 quick-v1 格式。摘要绑定媒体内容版本与 stat，候选需重新验证当前文件，不给 hash 加 UNIQUE，不合并两个已有历史实例。旧外部媒体可物化至托管区，ID 和版本不变，原件保留；普通 Scanner 不改变默认独立实例规则。
2. **Storage**：默认 `<app-local-data>/data/media/<id>/<basename>`；注册持久 storage_roots，app_settings 选后续新文件目标根，job 冻结该根，旧根继续访问。设置不搬已有文件或数据库。逐祖先拒绝 symlink/reparse、规范化包含检查、文件 create_new、可用空间预检/逐文件复检。copy/hash 在事务外，单 worker、64KiB buffer、磁盘目录/文件队列，内存不随媒体总量增长。
3. **跨资源提交**：先在 job entry 保存保留 ID/目标路径，再写同目录 part、校验完整 hash/源证据、sync，原子无覆盖发布，最后一个短 SQLite IMMEDIATE transaction 写 media/物理 System Tag/scan_evidence/完整 hash/来源路径/Source Tag 关系和 checkpoint。复用媒体不写计数、imported_at、Custom Tag 或 playback_states。复制保持既有媒体 mtime，避免纯位置变动误失效内容版本。DB 失败保留文件发布凭据；重启将工作标 interrupted，显式 Resume 时核验自有完成文件后继续，不能把 part 当完整文件或盲删最终媒体。
4. **来源标签**：import_sources 表示规范化来源根；import_source_tags 的 (source_id,relative_dir) 唯一，父级同来源，只有导入服务创建。media_import_sources 以媒体+来源+相对文件路径幂等；media_import_tags 多对多并关联所有目录祖先。根目录为分组、不伪装成物理目录标签。来源树按页读取；Source Tag 可递归过滤 Library，游标/Quick Browse filter 签名继续复用现有查询系统，旧 filter JSON 的新字段默认空。
5. **v12 迁移**：只追加 storage_roots、import_sources、import_source_tags、media_import_sources、media_import_tags、media_full_hashes、managed_media、import_jobs、import_entries、import_plan_tags 及必要查询索引/来源关联代次触发器。队列/checkpoint 为可恢复工作状态，完整 hash 非唯一；无全库 hash/backfill、无旧媒体复制、无事件/状态清空。原 runner 在既有库升级前备份、每 migration 与 ledger 原子提交；v1–v11 原 SQL 不改。失败保留备份和原库，不自动降级。
6. **UI/权限**：官方 Tauri 2 dialog Rust 插件，React 仅调用选目录/预览状态/开始/取消/来源树/存储设置等类型化用例 API，不授予通用 fs/sql/shell 或 dialog open capability。后台预览与导入不阻塞现有库；状态轮询返回聚合和有限失败/来源页，不传全库文件列表。

迁移兼容和 21 项用户验收使用临时库/合成文件，包括已有三类事件/标签/进度、完整 hash 与 partial 相同但全文不同、复制失败/空间不足/取消/重启、源目标包含和 Unicode。保留 Release Review 未提交的文档与测试工作。

实施补充：v12 的 `import_reserved_target(target_path)` 索引使 Scanner 入队时能跳过尚未数据库发布的导入预留文件；发布完成后继续正常增量对账。真实故障注入复现了缺少该保护会另建身份，已保留回归，未重写 Scanner。Worker 启动按已存在任务的 rowid 上界恢复中断状态，不依赖可能跳变的系统时钟，也不会把启动后新建的任务误标中断。Resume 在短事务内校验任务状态，保留已完成项及未发布目标凭据。

身份、重复文件、保守匹配、Like、Activation View、Qualified Play、播放状态、completed、三层播放及 100,000+ 架构已获用户批准。Phase 11 已完成稳定性与性能检查；当前 Phase 12 在此基础上增加复制导入，不改变既有事件规则。各阶段历史范围和新增行为见对应小节。

### Phase 11 有界扫描、后台恢复与 v11 migration

- 实测复现 by_path 全表扫描与 unseen 每步按根排序，v11 增加 `media_scan_path(file_path,missing) WHERE state!='superseded'` 和 `scan_unseen(root,media_id,generation)`。unseen 从证据表按媒体 ID seek；队列和观察记录继续使用已有索引。旧迁移与数据模型不变，正常升级备份、失败回滚。
- 真实 10k 小文件扫描证明逐文件 FULL 自动提交成本显著。入队、观察及消费 checkpoint、身份发布、missing 更新各按最多 100 项短事务提交；读取、证据核对和指纹全部在事务外，数据库仍使用 WAL/FULL。最多保留 100 个已准备项，不加载全库；观察记录和队列完成状态原子提交。失败回滚整批，取消保留已提交结果，下次重新枚举并对账。
- 高置信度移动覆盖不同活动目标时，在同一事务内先将旧目标实例标记 superseded，再移动源实例。源 ID/imported_at/标签/事件/进度保留，旧目标的历史仍保留；不删除任何原件或历史。不可靠匹配规则不变。System Tag 关联现在包含物理根目录本身；既有漏关联在下次扫描修复。
- Thumbnail/Quick Browse 启动恢复在各自后台线程执行及重试，不在返回应用状态前同步等待写锁。Thumbnail recovery 每批最多 1,000 行，旧版本 pending 项直接由原队列消费，避免恢复循环无法结束。播放器 claim 与 save 均先获取 IMMEDIATE 写事务，再校验和更新状态，避免读后升级的 SQLITE_BUSY。
- 保留既有 Grid 虚拟化、可见缩略图请求、共享单 FFmpeg 进程预算、磁盘随机排序和 Event Log 聚合架构。10k/100k、百万事件、进程异常退出、迁移及计数重建的实际证据见 [Phase 11 验收](docs/validation/phase11-validation.md)。History 每页仍重新聚合，首次随机排序仍需建磁盘索引；这些成本没有转移至 React。

### Phase 10 报表读模型与 v10 migration

- 新增 reports domain/service/repository 及 query_reports/get_dashboard 类型化 IPC。命令在 spawn_blocking 中通过独立 SQLite 连接执行，报表不占用业务写协调器；Repository 仅参数化读取，不判断事件资格或写事件。Dashboard 的汇总与五个预览在同一读事务中保持一致。
- v10 追加 Like/View 的 occurred_at、Play 的 qualified_at 覆盖索引，以及非零进度的 updated_at 索引。旧 SQL、媒体、事件、计数、进度、标签均不重写；升级前沿用正式 runner 备份，失败事务回滚。索引创建是一次升级工作，普通重开不重建。
- 时间段事件以 SQLite COUNT/MAX/GROUP BY 汇总媒体 ID，再按时间或数量加 ID keyset 返回 limit+1 行。All Time 同样读取日志，因此统计重建期间不依赖正在修复的 Counter Cache。SQLite temp_store=FILE、连接页缓存 4 MiB，Rust 只收集一页；不宣称 SQLite 全进程峰值内存已量化。
- report_generation 跟踪恢复状态及内容版本/时长/导入时间变化；与 library_generation 相加形成单调代次，绑定完整查询签名和冻结时间窗口。事件/标签/扫描变化沿用 library_generation。游标过期明确返回 stale_cursor，刷新从首页恢复，不静默合并不一致页。
- current_playback_progress 视图只联接 media.current_content_version_id 与 playback_states，采用恢复状态有效时长，缺失时回退当前 metadata 时长；按批准公式计算 completed。兼容旧客户端未更新 completed 的恢复行，不改 Event Log，也不做全表完成状态回填。历史 Play.completed 仍是该 session 的资格/终结事实。
- React 增加侧栏入口与 ReportsWorkspace；History/CW 使用虚拟行和 lazy thumbnails，Dashboard 最多 30 个预览条目。前端仅计算本地日历边界及单项进度显示，计数、筛选、排序在 SQLite。打开报表条目复用 ViewerShell、只读资源协议、进度串行保存和 source=history activation；本页最多 100 个可用 ID 固定查看器导航顺序。
- 真实续播验收复现进度事务与活动写入竞争：PlaybackRepository.save 改为先 BEGIN IMMEDIATE，再读取 owner/CAS/序号并写入，沿用短事务及 2 秒 busy_timeout。避免 DEFERRED 读取后的写升级失败，不改变保存/事件语义；回归用另一连接持有 150ms 写事务，校验保存等待后成功、同请求重试不增版本、零 Play。

当前工具链政策由用户最新授权改为 Rust stable，本机 rustc/cargo 为 1.99.1（D025）。rust-toolchain.toml 选择实际编译通道，Cargo.toml 的 rust-version=1.90 仍是最低版本声明，不能当作当前 pin 或最低版本已验收。npm/Cargo 依赖及真实锁文件均已到位；2026-11-04 类型检查、前端构建、完整 Rust 测试/Clippy、Tauri info、正式 release 构建及隔离的真实桌面 IPC/持久化/实例锁检查均通过。浏览器预览专项仍受工具阻塞，阶段状态见 PLANS.md。以下 Phase 1 版本描述为历史快照。

### Phase 1 实际落地边界

以下保留 Phase 1 历史范围；当前 v2 数据表与 Repository 见「Phase 2 数据库基线」。

- 已编写 commands/domain/services/db/platform 模块；前端仅 app/api。未创建业务功能空壳或注册未来业务 command。
- 使用 Tauri 2、Vite/React、TypeScript strict、Rust 1.90、rusqlite bundled SQLite。直接依赖精确版本已写入 manifest；受网络限制，完整依赖锁文件尚无法生成，不能宣称依赖闭包已锁定或构建通过。
- app_local_data_dir 下创建 data/library.sqlite3、data/instance.lock、cache/thumbnails/；只有升级既有库时创建 data/backups/。不使用当前工作目录或用户媒体目录。
- 唯一 command 是 get_app_status；通过 spawn_blocking 调用状态服务，惰性且串行初始化目录/数据库，UI 可先显示连接状态。初始化失败保留可重试状态，不缓存伪造成功；状态响应不暴露实际路径或 SQL。
- Phase 1 为单 Mutex 保护连接，应用 lifetime 文件锁限制写实例；后续业务按原架构扩展写协调/读连接。初始 migration 仅 app_metadata，schema_migrations 为 runner 自举表；不提前实现媒体/事件业务 Schema。
- 校验历史优先于持久 PRAGMA，逐 migration 事务、SHA-256、升级前 SQLite backup。现有较新/损坏/无版本记录库拒绝，不覆盖。详见 D018–D020 与 README。
- 代码、类型检查、完整原生编译/自动测试、release 构建及真实桌面启动/IPC 检查已通过。剩余浏览器专项上轮被 URL 自动审批检查终止，最新单项重试在浏览器 host/原生 pipe 连接阶段受阻，仍未验收。测试使用独立标识及应用目录覆盖，不改变正式运行路径或权限；具体结果在 PLANS.md。

本轮必须修订旧提案的理由：
1. 不确定移动无限 pending 不符合批准的 false-negative 优先策略，改为对账结束后 missing + new。
2. Like 撤销/delta 与产品冻结语义冲突，移除相关载荷和约束。
3. View 改为 activation；Play 改为 5 秒/短片真实结束，废止 3 秒候选。
4. Qualified Play 插入时播放器尚未结束，须允许受限补全同一事件，不能再禁止其所有 UPDATE。
5. 单一全库重建写事务会长期阻塞写入，改为高水位、后台聚合及分媒体短事务发布。
6. 新增 remux/proxy 架构及缓存。当前无真实数据库，无数据迁移要执行；初始 migration 按新 Schema 创建，未来变更须正式迁移，禁止临时改库。

## 拟定项目结构

以下是总体目录规划；Phase 1 创建基础模块，Phase 2 增加 domain/library 与 db/repositories，其余业务目录按阶段引入，不代表已经实现。根目录是唯一正式工程，重复的默认模板完整保留在 archive/scaffolds/，并排除在 Cargo workspace 与 Vite watch 之外：

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
├── tests/
│   ├── frontend/                     # 现有 TypeScript DTO 契约
│   ├── architecture/                 # 现有 IPC / 权限边界
│   ├── database/                     # 现有实际 SQL 测试
│   ├── fixtures/                     # 后续合成或许可明确的小型媒体
│   └── e2e/                          # 后续桌面集成
├── docs/
│   ├── reviews/                      # 阶段审查历史
│   └── validation/                   # 实际检查结果
└── archive/scaffolds/                 # 原始初始化模板，不参与构建
```

运行数据在应用数据目录：data/library.sqlite3、cache/thumbnails/；后续再引入 cache/playback/、临时 spool 和诊断日志。不得写媒体目录；本地数据库不放网络共享文件系统。显示路径与规范化路径键分离，原生路径编码由 platform 模块封装。

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

System Tag 由 system_tags 和 media_system_tags 保存物理目录观察（取代原 directories 命名）；历史位置不成为当前标签，离线显示最近成功扫描状态。未来 closure 只能是派生索引。Custom Tag 独立关联 media_id，操作不触及文件系统。目录重命名未来由 scanner 更新观察，不存在编辑 System Tag 来移动文件的 API。

## Phase 2 数据库基线（v2 实际 DDL）

当前仅实现数据模型与 Repository；scanner、Event Service、业务 UI 均未实现。真实 Schema 见 [0002_library_model.sql](src-tauri/migrations/0002_library_model.sql)，v1 SQL 保持不变。以下实际模型取代旧总体草图中不同的表名/存储位置；后续新增表仍必须 migration，不能把旧草图当作已经存在的表。

### 本轮调整理由与字段映射

用户指定 media 当前字段、system_tags、平面 Custom Tag 和 quick_browse_sessions。为便于有索引的卡片/元数据查询，media 现在是当前文件快照的权威行：包含 path/name/fingerprint/尺寸等指定字段；media_locations 保留位置历史投影，media_revisions 保留内容版本，playback_states 仍独立。改变的是当前字段的存放位置，不改变稳定媒体 ID、历史归属、事件事实来源或保守匹配原则。

system_tags 本身就是原 directories 设计的物理目录投影，不再建第二份可编辑目录树；directory_closure 如以后确有性能需要只能作派生索引。Custom Tag 无 parent_id，无子标签，当前名称使用 SQLite NOCASE（ASCII 大小写不敏感、非 ASCII 精确比较），trim 后非空且唯一。

时间点为 UTC 毫秒 INTEGER；created_time 是可空的文件系统创建时间，modified_time 是文件系统修改时间，与 imported_at 分离。duration、playback_position 和事件位置/观看时间使用秒。未知宽高/时长/thumbnail_path 允许 NULL；三个计数、playback_position 和 missing 有明确零值默认。

| 当前表 | 关键内容 / 边界 |
| --- | --- |
| schema_migrations / app_metadata | v1 ledger 与应用元数据，v2 保留；编号、name、SHA-256 和应用时间可追踪。 |
| db_sequence | 单行单调序列，媒体创建由 Repository 同事务分配；未来 Event Service 也必须在事件事务内分配 ingest_seq。不是 identity。 |
| media | id、fingerprint、file_path、file_name、media_type、file_size、created_time、modified_time、imported_at、last_seen_at、width、height、duration、thumbnail_path、like_count、view_count、play_count、last_viewed_at、last_played_at、playback_position、missing，以及 state/current_content_version_id/created_seq。 |
| media_revisions | PK(media_id,content_version_id)，保存版本大小/指纹。media 当前版本复合 FK 延迟到提交检查；首次插入 trigger 同事务建立初始版本。 |
| media_locations | 自动记录当前/历史路径、文件名及观察/退役时间；每媒体最多一条未退役记录。位置变化或 superseded 同事务维护，不是第二个 identity。 |
| system_tags | id、parent_id、source_root、directory_path、name、source=filesystem、last_seen_at；同根复合 FK、唯一目录路径/根、无自引用/环。无固定层级深度上限。 |
| custom_tags | id、唯一平面 name、created_at、updated_at；可创建/改名/删除，ID 关联保留，永不移动媒体文件。 |
| media_system_tags / media_custom_tags | PK(media_id,tag_id)，标准多对多与反向索引，禁止逗号字符串或 JSON 代替标签关联。 |
| like_events | 不可变 id/request_id、media_id、ingest_seq、occurred_at、recorded_at、policy_version；没有 toggle/delta。 |
| view_events | 在 Like 通用字段之外，activation_id 唯一；同 activation 不能插第二次。 |
| play_events | id、request/ingest、media/version、唯一 player_session_id、started_at、qualified_at、ended_at、start_position、end_position、watched_seconds、completed、end_reason、policy_version；media/version FK。没有资格判断代码。 |
| playback_states | PK(media_id,content_version_id)，独立 position_seconds/duration_seconds/completed/updated_at/state_version。media.playback_position 是当前版本的投影，写状态/切版本/删状态均由 trigger 同步，不产生 Play。 |
| quick_browse_sessions | seed、sort_mode、algorithm_version、filter_json、building/ready/closed、item_count、current_ordinal、created_at；Random 必须 seed。仅存储，不生成随机顺序。 |
| quick_browse_session_items | PK(session_id,ordinal)、UNIQUE(session_id,media_id)，引用媒体/版本；顺序显式存储，ready 后不可增删改，发布必须 ordinal 连续且 count 一致。 |
| app_settings | 非空 key PK、受 json_valid 约束的 value_json、updated_at；设置可保存结构化配置，不承载媒体标签关系。 |

### 唯一性、级联与事实保护

- media.id 才是身份，且 id/imported_at/created_seq 不可被 UPDATE 改写。fingerprint 仅有非唯一查询索引；不同路径的同内容实例完全允许。
- 当前 active file_path 唯一，missing/superseded 仍保留旧行但释放活动路径；这是位置占用约束，不是 identity。路径比较为数据库 BINARY；未来 scanner 必须按平台提供规范化观察值，当前 Repository 不替代磁盘判定。
- 删除 Custom Tag 或叶 System Tag 只级联关联行，不删除媒体。System Tag 父目录有子节点时 RESTRICT，按观察结果自底向上清理须由未来 scanner 决定，不能把用户标签删除当物理目录删除。
- 无事件/快照引用的媒体硬删除可级联清理所属版本、位置、状态与标签关联；有事件或快照引用时 RESTRICT，绝不级联删历史。Repository 不提供媒体硬删除/文件删除接口，SQL 测试仅验证规则。
- like/view 禁止 UPDATE/DELETE；Play 身份/资格事实不可改，仅允许一次终结补全，之后冻结。Source of Truth 始终是三个事件表。
- 每个事件 INSERT 的 AFTER trigger 在同一 SQLite statement/transaction 内更新对应 Counter Cache，view/play 同时更新 last_* 时间的 MAX；失败、溢出或外层回滚同时撤销事件与缓存。**未来 Event Service 不得再手工 +1，避免重复计数**。
- 本轮 EventRepository 只读，未暴露事件写入、资格判断、重建或 Tauri 业务 API。未来 Event Service 负责 activation/player session 的资格和请求幂等，以及 D016 重建期间的 delta hook；应先追加正式 migration，再启用这些服务。
- thumbnail_path 只是可丢弃缓存引用，不是原媒体路径；Phase 4 已实现生成、内容失效和 miss 重建，配额/LRU 回收仍属后续范围。playback_position 不是 Play 事实，不能由保存进度创造事件。

### Repository 边界

Database::repositories() 借出类型化、受连接生命周期约束的 Rust Repository，不开放裸连接给 React。MediaRepository 提供插入/按 ID 读/按(imported_at,id) keyset 分页/存储已决位置/缺失状态；不匹配身份、不读文件。CustomTagRepository 提供平面 CRUD、双向分页关系查询及原子替换关联；FK 失败保留原标签集合。

SystemTagRepository 仅接收 trusted ObservedDirectory，检查绝对根、拒绝 root/path 中的 ParentDir（..）、同根及直接物理父路径，并允许读取树/存储已观察关联；没有手工目录创建 UI/Tauri command。路径校验为纯词法检查，不访问磁盘；来源真实性和链接解析由未来 scanner 提供，不声称验证了物理存在。

EventRepository 只读三种日志与计数；QuickBrowseRepository 仅保存调用者给定成员/序号并原子发布，未选择/排序/激活媒体；SettingsRepository 保存结构化 JSON。分页上限 200，默认调用建议 100；所有值参数化，字段列表只来自内部常量。

### 尚未落地的后续表

根库管理/扫描 runs/candidates、活动工作会话、缩略图/播放缓存索引、后台 job 与统计 rebuild base/delta/checkpoint 等仍属于后续实现；当前 activation_id/player_session_id 只有唯一性，尚无工作会话表 FK。后续建立会话表时须正式 migration/backfill，并继续保留媒体/版本 FK 和既有事件事实，不能静默丢弃历史。当前没有 Event Service 或 active rebuild，因此不存在绕过重建 delta 的生产事件入口。

### 索引、启动与 migration

- v2 已有索引：media(imported_at,id)/(like_count,id)/(view_count,id)/(play_count,id)/(missing,id)、非唯一 fingerprint、活动路径部分唯一、位置历史(media_id,id)、System Tag(parent_id,id)、两种标签反向(tag_id,media_id)、三个事件(media_id,ingest_seq)、快照(session_id,ordinal)。扫描代次、closure、后台任务和缓存 LRU 索引随对应未来表新增，不声称当前存在。
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
3. 插入事件，分配 ingest_seq；同事务由数据库 trigger 更新 media 对应 Counter Cache +1，服务不得重复累加。若统计重建 active，同事务写入对应重建 delta；任何一步或 COMMIT 失败整体 rollback，未提交不得应答成功。
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

本节是后续业务 API 规划；Phase 2 没有新增这些 command。当前注册表仍只有 get_app_status，Rust Repository 方法不等于前端可调用入口。

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

**Ready for Phase 1: YES（设计就绪）**。用户现已授权 Phase 2 数据层，实际验收见 PLANS.md；后续 scanner 与业务服务未开始。无需等待性能实测或完整页面清单才能建立工程骨架，不能擅自改产品语义。

## Remaining risks 与后续验证

- 高置信度证据依赖平台文件标识质量；保守分离可能多出媒体项，这是批准的取舍，不得以 hash 去重“修复”。
- 同大小同时间戳改写普通扫描可能漏检，显式深度核验成本需测量；根离线不能误标全库 missing。
- WebView codec、HDR/音轨/字幕、时间基和 remux 兼容性按平台矩阵验证；H.264/AAC 编码器构建许可和可用性发布前核验。
- 实际播放证据受 WebView 采样精度与异常退出影响；严格去重，宁不虚构观看量。终结补全需测试中断/重试/触发器约束。
- 重建高水位、base/delta/checkpoint、并发事件与取消恢复需故障注入；scratch 磁盘占用和批次延迟需要规模测量，不回退长期全库写事务。
- Quick Browse 构建会占 spool/WAL/磁盘，必须限额与清理；100,000+ 查询索引组合、内存、冷/热响应和代理吞吐在后续 Phase 验收。
- migration、路径编码/大小写/链接越界、坏文件与缓存孤儿恢复必须测试。以上是工程风险，无当前 Phase 1 未解决的业务阻塞项。

## Phase 4 已实施边界（2026-11-08）

- schema v4 增加 media.video_codec/audio_codec/container/frame_rate，以及可丢弃的 media_derivatives 队列（pending/running/ready/failed、独立 metadata_status/error、thumbnail_error、cache_key、generator_version、attempts）。width/height/duration/thumbnail_path 继续使用原字段；没有 binary/BLOB。
- metadata 模块只做本地 ffprobe/FFmpeg 调用与解析；thumbnails 服务编排观察、生成、发布；DerivativeStore 只执行参数化存储及事务。扫描 INSERT/内容证据更新由 trigger 排队/失效；不在扫描事务中解码。
- 当前一个 thumbnail worker，独立 SQLite 连接共享同一 lifetime 实例锁；事务外运行子进程，前台状态连接不等待 I/O。全局进程槽位暂为 1（低于既定预算 2），未来播放须复用同一限流入口；不启动额外无界任务队列。
- 缓存键为 SHA-256(JSON(media_id,content_version_id,fingerprint,size,mtime,variant,generator_version))；路径不参与键，独立同内容实例不共享键。mtime/size 补充 quick fingerprint 证据；源文件前后 stat 比较与 DB CAS 拒绝过期结果。数据库内容证据变化立即清空当前 metadata/path；同路径替换沿用 scanner 新 ID 规则。
- 320×320 内等比 JPEG，视频优先 0.1×duration 秒，取帧失败回退首帧；未知 duration 取首帧。图片强制单文件 demuxer，兼容 Windows 扩展路径及含 % 的名字。允许单文件图片/视频 demuxer 和 file 协议，禁止网络/播放列表/序列输入。
- 缩略图经受限 stdout 读取，检查 JPEG 标识、帧尺寸与扫描头，再写应用 thumbnails 目录的 .part、sync、同目录 rename，最后提交路径与 ready。只检查并操作生成的哈希路径，拒绝链接/reparse point；不会向原件写入或递归清理。文件/数据库之间无共同事务，迟到生成可留孤儿缓存，后续配额回收另行实施。
- ready 路径请求检查命中，不重新生成；丢文件/错误键入队。failed 显式重试；启动恢复 running 及旧 generator；取消保留 pending。工具缺失不阻止应用/扫描，逐项记录稳定错误码，已有 metadata 可在 thumbnail 失败时保留。启动本轮不新增业务 IPC 或 UI。
- 默认单进程超时 30 秒，取消/超时 kill + wait；stdout 1 MiB、stderr 64 KiB、单次分配 64 MiB、像素 100M。工具由 PATH 或 MEDIA_MANAGER_FFMPEG_DIR 提供，测试下载不代表发布包已分发或许可验收。

以下是实现所依照的官方工具文档，缓存/队列方案为本项目设计：

- [ffprobe JSON 与 show_entries](https://ffmpeg.org/ffprobe.html)。
- [FFmpeg 输入 seek 与单帧输出](https://ffmpeg.org/ffmpeg.html)。
- [FFmpeg protocol_whitelist](https://ffmpeg.org/ffmpeg-protocols.html)。

## Phase 5 基础 Library 边界

- schema v5 新增 library_generation 与 state+recent/name/likes 查询索引，保留 v1–v4 原 SQL；媒体排序/筛选事实、目录/标签关联变化使游标代次失效。metadata/thumbnail 进度不改变排序代次。
- query_library 使用短读事务、默认前端 100 项/后端上限 200 的 keyset；游标携带 generation、filter/sort signature、排序值与稳定 ID。System Tag 以递归 CTE 包含 descendants，EXISTS 避免多标签重复。搜索只在 SQLite 对文件名作字面 substring（ASCII lower 大小写折叠），不在前端排序/过滤全库。
- list_library_tags 按层级/ID 分页；System 仅查 children，前端展开时按需查询。Custom 无 parent，支持标签搜索及指定媒体的 attached 状态。卡片每媒体最多附 8 个标签及完整数量；详情的 TagSelector 用分页查询访问其余标签。
- mutate_custom_tag 只操作 custom_tags/media_custom_tags。批次最多 200 个媒体、10 个标签且最多 1000 个组合；单事务先验证活动媒体/标签，失败整体回滚，幂等 add/remove 不改任何物理文件或事件表。
- 六个组件通过 src/api 内的类型化调用访问后端；command 用 spawn_blocking 和独立短连接，共享应用 lifetime 实例锁，AppState 只负责连接生命周期。无 SQL/fs/shell 前端权限，无 System Tag mutation、Like/Event 或播放器入口。
- 网格每页最多 100 项，换页替换旧数据；可见行加 2 行 overscan，图片 loading=lazy/decoding=async。选择最多 200 项，历史游标最多 256。缩略图对可见集合批量请求（上限 100），pending 每 2 秒有界重查，离开视图取消后续轮询；仅受验证的缓存 JPEG 经 asset protocol 展示。
- asset protocol 仅开放 $APPLOCALDATA/cache/thumbnails/*.jpg，CSP 仅增加 asset:/http://asset.localhost 图片来源，不给原媒体目录资源权限。Tauri protocol-asset feature 引入真实锁定 http-range 0.1.6，不变更既有版本或伪造锁文件。
- 测试入口 tests/ui/library.html 使用合成、按需生成数据和测试用 API adapter，仅供开发验证，不被产品入口或 production build 导入，也不修改原生 Tauri internals。真实桌面验收使用独立 identifier 与 appDirectoriesOverride，沿用 Phase 1 隔离方式；不能把 mock UI 通过等同真实 IPC 通过。

## Phase 6 已实施的查看器与恢复状态

- v6 migration 只新增 playback_owners(media_id,content_version_id,owner_id,last_sequence,last_position,last_duration)，复用既有 playback_states 和 media.playback_position trigger。owner 接管、CAS 和严格递增 sequence 拒绝迟到写；同序号同事实重试幂等。completed 字段保持既有值，本阶段不实现资格或事件。
- open_viewer 只为当前活动媒体打开后端只读 File，校验源文件证据，发放 128-bit 随机 token；最多 8 个短期会话。media URI 只接受 token/direct|remux|proxy，不能以 path 读任意文件；原媒体不进入 asset scope。关闭/退出撤销资源，逐请求校验内容版本与文件句柄证据。
- 异步协议只在 main WebView 响应，I/O 用 blocking worker；单读槽位，视频每个 Range 最多 4 MiB，单图片最多 64 MiB，HEAD/206/416 正确返回。响应 no-store/nosniff，未知/过期 token 拒绝。移动后的相同只读句柄可继续读取；替换或版本改变不能冒充旧资源。
- VideoPlayer 在原生加载失败或 metadata 显示无可解码视频画面时依序 Remux（stream copy）与 Proxy（libx264/AAC、yuv420p、最长边 1280×720 内、偶数尺寸），不把仅音频播放视为视频成功，不改变媒体/恢复 owner。FFmpeg 与 thumbnail 共享全局一个进程槽位，播放生成另有一个槽位；默认 600 秒超时、取消 kill/wait。输出最多 512 MiB，cache/playback 容量 2 GiB，回收只处理受管理哈希名且未 pin 的文件，每批最多 64，临时输出也计入预算。
- 缓存键含媒体 ID/version/fingerprint/size/mtime/mode/profile，先写 .part.mp4，ffprobe 验证尺寸、时长（已知源时长偏差 <=max(1 秒,1%)），再原子 rename。原件不作为输出，转换失败/源变动/取消不会发布未完成资源；失败可以重试。当前使用软件编码，发布包 FFmpeg 许可仍须后续单独验收。
- ImageViewer 只加载当前原图，支持 fit/100%/锚点缩放/受限平移及全屏；VideoPlayer 用受限资源 URL，提供自定义播放/暂停/seek/音量/静音/速率/全屏。前后项在 SQLite 按原筛选/排序 keyset 查邻居，后台改变时 stale_cursor 明确拒绝，用户主动标签修改后刷新导航，不偷偷重排 Quick Browse（本轮不实现该会话）。
- ProgressSaver 以单 in-flight + 一个最新快照合并串行写；失败先重试相同 sequence/expected_version/事实，再保存最新位置。5 秒 checkpoint、pause、ended、切换、关闭、pagehide/visibility 都可刷新；原生 CloseRequested 阻止立即退出，最多等 3 秒尽力 flush，再 destroy 窗口。强制终止/断电可能损失最后一个 checkpoint 间隔，不能因此虚构 Play。
- 图片/视频 Details 与 Custom Tag 使用既有 Library 接口；本阶段没有 Like/View/Play 写入口，也不从真实播放或位置持久化增加 play_count。

## Phase 7 已实施的事件与统计

- v7 migration 保留既有事件身份、日志不可变触发器及 Counter Cache 触发器，仅为 View 添加受约束 source 与 generated viewed_at（等于 occurred_at），增加 viewer_activations、player_sessions 和 statistics_jobs/base/delta。旧 View 缺乏来源事实，迁移默认 normal_viewer，不能将该默认当作已观察到的历史来源；不改原事件时间、媒体或计数事实。
- ActivityService 决定资格，ActivityTransaction 负责参数化存储；后台连接共享短事务 activity_gate，SQLite IMMEDIATE 事务避免读后升级竞态。Like 的新动作分配新 request_id；同请求同媒体重试返回原结果，冲突拒绝。事件 INSERT、sequence、既有 trigger 的 +1 和重建 delta 同事务；没有服务层第二次累加。
- ViewerShell 的 ActivationController 在明确切换时推进 ordinal，渲染、Details、缩放、全屏和兼容资源切换复用身份。当前主媒体挂载后，经 media token 验证媒体/版本，再调用 activate_media；旧 ordinal 拒绝，同 activation 幂等，离开后再回来分配新身份。内部 record_view 不开放为绕过资源验证的 IPC。source 类型支持全部批准值，本轮不实施 Quick Browse/History 页面。
- 每 activation 的视频只有一个 player session，pause/resume/seek/重播及 Direct/Remux/Proxy 共用。report_playback 上报有序单调时钟、位置、速率与 playing/paused/waiting/seeking/ended 边界；前端保留每个边界，失败按相同 payload/sequence 重试并暂停播放提示。后端以客户端单调间隔、服务端单调经过时间及正常位置推进/速率的交集累计实际观看，每段最多 2 秒；缺失大段证据保守不补算，seek、恢复、缓冲、暂停不增加观看。
- duration>=5 或未知时 watched>=5 才 record_play；duration<5 时须 observed ended，并有实际播放推进至尾的证据，seek/恢复本身不能触发。浏览器 pause-before-ended 沿用上一有效尾部播放证据；不把 completed 的 95% 当作短片资格。qualified_at 时仅 INSERT 一次；关闭/切换后 finalize_play 一次补全 ended_at/end_position/watched_seconds/completed/end_reason，数据库冻结后续更新且不再计数。completed 严格使用 95% OR remaining<=min(10s,5% duration)。异常恢复在后台按最多 100 个旧 session 一批，只终结最后已持久化事实，不虚构播放时间。
- StatisticsService 提供三个 kind 的单媒体/全库函数及统一 rebuild_statistics。普通启动不新建重建任务，只续跑持久化任务。每任务冻结 db_sequence 高水位 H；三日志按 ingest_seq<=H 每批最多 1000 聚合磁盘 base，同事务推进 checkpoint。并发 INSERT 的 trigger 同事务写 delta。按 id 每批最多 100 个 created_seq<=H 的媒体，原子发布所选计数与时间字段；包含零事件媒体，之后的事件继续正常 +1。已发布媒体不再覆盖，新媒体不进入旧目标。
- 发布结束先停 delta 捕获，base/delta 按最多 1000 行清理。崩溃按 checkpoint 续跑；取消同一短事务停捕获并清理，失败标记 failed、保留日志和已发布修复，后续后台有界清理派生 scratch，可用新 request 重试。全库修复逐媒体生效，UI 展示 running/cleaning/complete/cancelled/failed，不宣称全库瞬时原子切换。
- LikeButton 和 L 都进入 activityApi.addLike → 同一 add_like service；响应更新当前计数，不以 optimistic increment 代替事务成功，不合并主动动作。重试保留失败 request_id，前台自身 Like 后刷新导航代次；其他后台变化仍明确报告 stale cursor。重建按钮只启动后台 job，不进行全库前端统计或原件读取。原件资源/标签/进度边界保持不变。
- 验证：100 并发/快速 Likes、Like/View/Play 故障回滚、COMMIT 失败、activation、资格/终结、重建并发及恢复、100,002 媒体分批实测、真实 Tauri 交互与解码。记录与截图见 [Phase 7 验收](docs/validation/phase7-validation.md)。WebView 上报用于正常客户端顺序/状态校验，不声称可抵御修改客户端伪造播放证据。

## Phase 8 查询实现

- LibraryFilter 支持三个搜索字段、媒体类型、目录后代、最多 20 个 Custom Tag 的 AND/OR、三个计数区间、导入时间区间及查看状态。SQL 参数绑定所有值，内部白名单提供列、方向和索引。递归目录查询和关系 EXISTS 保证结果按媒体去重。文件名和标签使用字面子串匹配。
- v8 迁移添加四个虚拟排序键，替换文件名索引，建立 state 加 View Count、Play Count、Created、Recently Viewed、Recently Played 的排序索引，以及 state/type/Imported 的组合索引。沿用关系表索引。新增字段变化与既有扫描、标签、Like 变化共同推进查询代次。迁移通过既有备份和 checksum 机制执行。
- 查询以排序键和 media ID 做双向 keyset。游标绑定筛选签名、代次及随机版本/seed。数值键用十进制字符串跨 IPC，避免丢失 SQLite i64 精度。页面最多 200 项，应用每页 100 项。查看器读取邻居与代次在同一快照内完成，进入媒体时捕获排序锚点，自己的 View、Like 或合格 Play 改变排名后仍按该锚点找邻居。
- Random v1 的域为 `MediaManager/random/v1`。对域、由 32 个十六进制字符组成的 seed 和 media ID 的 UTF-8 字节分别加 u64 大端长度前缀，再做 SHA-256。SQLite 按摘要十六进制文本与 ID 升序。桌面保留一个专用查询连接和一份磁盘临时排序索引，首次访问构建，后续页面按索引 seek。新增 ID 按 created_seq 增量补齐。切换 seed 替换临时排序数据。构建在阻塞工作线程的读快照内进行，临时 SQLite 页缓存设置为 4 MiB。
- 初始 seed 是固定的 v1 值。Shuffle Again 生成新 seed 并存入 WebView 本地设置。筛选、刷新、分页、切换排序及重开应用复用此设置。当前目录是实时查询，新增媒体按其哈希键加入，已有媒体的相对随机顺序保持稳定。

## Phase 9 Quick Browse 实现

- v9 追加迁移，不改 v1–v8。为既有 quick_browse_sessions 增加 started_at/ended_at、query_sort、视频选项、构建 checkpoint/取消/错误与父会话引用；旧行按 created_at 和原排序族回填。保留旧 sort_mode 的 imported/random 约束兼容既有 Repository，query_sort 保存完整 Phase 8 排序枚举，filter_json 保存实际查询。已发布成员、版本和顺序继续不可变。
- QuickBrowseService 创建持久化请求，统一短事务写门协调 BrowseStore；LibraryRepository 复用 QueryPlan 执行完整筛选和排序，只逐行返回 ID/版本。独立后台 worker 同时最多一个构建。单次源读快照默认最多 120 秒，50 ms watchdog 可中断 SQLite 查询；浏览时不保留源读事务。取消、超时、配额、缓存失败及中断均进入有界清理，重启恢复未完成构建。
- 只在应用 cache/browse/<hash(session ID)>.sqlite3 创建磁盘 spool，默认 512 MiB 上限、4 MiB 页缓存；随机临时索引沿用 Phase 8 的磁盘/4 MiB 缓存策略。释放源快照后，每批最多 1,000 项写入 building 会话，checkpoint 与批次同事务，最后验证 ordinal/count 并短事务发布 ready。失败项按最多 1,000 行清理，spool 成功或失败均回收；拒绝链接/重解析目录。已发布会话保留元数据和成员，不自动删除历史；保留期/恢复页面仍未实施。
- 浏览 API 按 (session_id,ordinal) 主键 seek，最多 200 个轻量成员；产品每次仅获取当前一项，打开一个原件，不在前端排序或保存全库对象。missing、版本变更或损坏原件保留固定位置，可跳过；open_quick_browse_item 校验当前 ordinal 和冻结版本后复用只读 ViewerService。
- 父 QuickBrowseShell 持有会话 ActivationController，真正切换主项后 ViewerShell 复用资源授权/activate_media，source=quick_browser。数字键、Details、缩放、设置和全屏不创建新 activation；A→B→A 递增 activation ordinal。LikeButton/L 继续调用 Phase 7 add_like；视频复用兼容回退、播放资格和独立进度，不另建 Event 逻辑。关闭时先 flush，再释放资源，最后结束浏览会话，避免父子重复写已释放的 owner。
- app_settings 的 quick_browse.v1 保存 autoplay、mute 和最多九个数字→Custom Tag ID 绑定；标签名查询时解析。TagService 决定 Toggle，QuickTagStore 在一个事务校验当前稳定成员、改变关联并保存 quick_tag_operations 回执；重试同 request ID 返回原结果，冲突拒绝。回执保留当时名称/结果，不因标签删除而删除媒体或改动文件。
- R 建立带新 seed/父会话引用的新构建请求；原会话在新列表完成前不结束。新列表发布与旧会话 ended_at 同事务，失败则保留原列表，成功后前端切换。取消和 Esc 不生成新 seed。普通重渲染或后台更新不重排已发布列表。

## 技术依据

- [Tauri Capabilities](https://tauri.app/security/capabilities/) 与 [Command Scopes](https://tauri.app/security/scope/)：权限配置及命令内部校验。
- [SQLite Transactions](https://www.sqlite.org/lang_transaction.html)、[WAL](https://www.sqlite.org/wal.html)、[Foreign Keys](https://www.sqlite.org/foreignkeys.html)：单写者、事务与连接设置。
- [SQLite Row Values](https://www.sqlite.org/rowvalue.html)、[Query Planner](https://www.sqlite.org/queryplanner.html)、[Online Backup](https://www.sqlite.org/backup.html)：游标、索引与一致性备份。
- [FFmpeg Streamcopy / Transcoding](https://ffmpeg.org/ffmpeg.html)：remux 与重编码的不同处理路径；具体缓存/调度方案为本项目设计。
- [React StrictMode](https://react.dev/reference/react/StrictMode)：开发期额外 render/effect 要求业务 activation 与组件生命周期分离。
