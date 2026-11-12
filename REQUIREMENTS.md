# 产品需求与业务行为

## 状态与文档范围

当前授权为 Phase 10：History、Continue Watching、Dashboard，完成测试后停止。此前 Phase 的授权描述保留为历史范围。

### Phase 10 范围与统计口径

- History 提供 Recently Viewed、Recently Played、Most Viewed、Most Played、Most Liked；Today、Last 7 Days、Last 30 Days、All Time。按稳定媒体 ID 汇总一行，Recently 使用范围内最后事件时间，Most 使用范围内事件数量，排序并列以 ID 降序稳定裁决。无相应事件的媒体不进入列表。
- Today 从本地零点起，7/30 天包含今天及此前 6/29 个本地日历日；查询冻结 UTC 毫秒的包含起点/不包含终点，分页不滚动时间窗口。Play 按 qualified_at，View/Like 按 occurred_at；不以导入时间、播放进度或 started_at 冒充活动时间。
- 历史保留 missing/superseded 媒体的事件，明确标为不可打开，不把同路径新实例当作原记录。History 页面浏览和 Dashboard 统计本身不产生 View；打开查看器后复用 activation 服务，source=history。
- Continue Watching 仅显示 active 视频当前内容版本的 position>0 且未完成状态，按保存时间降序；显示已播/总时长和百分比。未知时长展示已播秒数及“时长未知”，不虚构百分比。完成状态由当前恢复位置和有效时长按既有公式计算，不由历史 Play 的 completed 推断；seek 保存可改变完成状态，不产生 Play。
- Dashboard 展示 Total Media、Images、Videos、Total Likes、Views Today，以及 Recently Added、Recently Viewed、Continue Watching、Most Liked、Most Viewed 各最多 6 项。媒体数包括 active/missing，排除 superseded；Total Likes 包含保留的全部历史事件，Views Today 统计本地当天 View 事件。Recently Added 仅展示 active 媒体，历史类预览保留不可用条目。
- 统计及排序由 SQLite 完成，History 每页默认 100、最大 200，keyset 游标绑定查询和数据代次；前端仅保留当前页、有界游标历史和可见行缩略图。不得将完整 Event Log 传到前端聚合。查看器在本页可用成员之间按打开时顺序导航，关闭后刷新报表。
- 本轮不扩展历史会话恢复、发布包或跨平台验收；新行为验证正常、失败、恢复及 100,000 媒体/事件规模，完成后停止。

固定技术栈：Tauri、React、TypeScript、Rust、SQLite、FFmpeg。具体依赖版本在工程初始化时锁定。

2026-11-02 用户进一步授权使用最新稳定 Rust 替代 1.90.0，并安装 typecheck/Tauri 工具：项目已改为 stable 通道（本轮 1.99.1），TypeScript 与 Tauri CLI 使用项目本地依赖。此项不扩展业务范围；安装或完整构建失败仍需如实保留未验收状态。

### 已有 Phase 1 基础

初始化 Tauri/React/TypeScript 与 Rust 分层；SQLite connection、migration 基础设施；安全创建 data/、cache/、cache/thumbnails/；统一错误与 get_app_status IPC。数据库 v1 仅保存迁移记录和应用元数据。媒体/标签/事件等业务表在后续 Phase 经正式 migration 创建，不能以本轮骨架为由改变其批准模型。

Phase 1 只建立基础设施，原生验证仍受环境限制。用户本轮已明确授权进入 Phase 2，不把此前未授权状态误当当前禁止指令；验证限制继续如实保留。

### 历史 Phase 2 范围（用户指定）

- 建立 media、system_tags、custom_tags、media_system_tags、media_custom_tags、like_events、view_events、play_events、quick_browse_sessions、app_settings 与必要 migration metadata。
- media 支持 id、fingerprint、file_path、file_name、media_type、file_size、created_time、modified_time、imported_at、last_seen_at、width、height、duration、thumbnail_path、like_count、view_count、play_count、last_viewed_at、last_played_at、playback_position、missing；未知媒体元数据可空，统计默认为零。
- System Tag 来源于真实物理目录，parent_id 或等价结构支持任意层级；不允许用户手工创建目录层级。数据层只保存可信观察，读取/验证磁盘仍是未来 scanner 职责。
- Custom Tag 明确无子标签，可创建、改名和删除，多对多关联媒体；禁止用逗号字符串保存 tags。当前名称 trim、非空且按 SQLite NOCASE 唯一；非 ASCII 不做额外 case-fold。
- 建立 Rust Repository；React 不接触 SQL，且不新增业务 Tauri command。事件仅实现表/约束和原子计数触发器，Repository 只读；不实现 Event Service。
- 为多对多、System Tag hierarchy、唯一性、FK、cascade、migration 等写测试。删除标签只清理关联，事件历史不得级联删除；普通数据操作不修改物理文件。
- 独立内容版本/位置历史/播放状态及快照成员表用于保留已批准架构边界；不因此实现播放器、随机排序或浏览业务。完成数据层与测试后停止，不自动推进 scanner 或其他 Phase。

### 当前 Phase 4 范围（2026-11-08 用户指定）

- 图片提取 width/height；视频以 ffprobe 提取 width/height/duration，架构允许时保存 codec 及必要基础播放信息。未知 duration 保持 NULL，不伪造时长。
- 图片生成缩略图；视频优先取约 10% duration 的帧，无法取帧可回退首帧。缩略图放应用 cache/thumbnails/，SQLite 只存路径、状态和元数据，不存 binary。
- 缓存关联稳定 media ID、内容版本和 fingerprint；内容证据变化失效，移动/重命名不因路径变化重建。重复打开 UI 复用 ready 缓存，文件丢失视为 miss。
- 后台生成，损坏图片/视频、unsupported codec、FFmpeg missing/failure、thumbnail generation failure 均逐文件隔离，不能使 scan 或 UI 崩溃。失败可显式重试，中断可恢复。
- 本轮完成全部测试、更新 PLANS.md 后停止。旧路线中的页面/播放事项仍是后续范围，不因阶段编号重用自动获得授权。

### Phase 5 范围（已完成）

- Sidebar 提供 All Media、只读树形 System Tags、平级 Custom Tags；点击父 System Tag 默认递归包含全部 descendants，返回媒体不可重复。
- MediaGrid 只加载 thumbnail，禁止一次读取全库原始高清图；SQLite keyset 增量查询、可见行 virtualization、lazy thumbnail loading，查询/DOM/选择集合与资源请求均有上限。
- Card 至少显示 thumbnail、file name、media type、Like Count、Custom Tags。DetailsPanel 显示 metadata 和只读文件位置；当前不打开原始媒体或播放器。
- Custom Tag 支持 create/rename/delete/add/remove/batch add/batch remove；只改数据库多对多关联，不改物理目录。System Tag 不提供手工更改目录关系的 command 或 UI。
- 必要类型化 Tauri 查询/编辑接口、失败与空状态、标签更新后的刷新、陈旧游标明确拒绝。运行 frontend/backend tests 后停止；Like Count 仅展示，不新增 Like/View/Play Event。

### 当前 Phase 6 范围（用户指定）

- ImageViewer：previous/next、zoom/pan、fit/100%、fullscreen、Details、Custom Tag。
- VideoPlayer：play/pause、timeline、volume/mute、playback speed、fullscreen、previous/next、Custom Tag、Details。
- 独立保存 playback_position：至少周期 checkpoint、pause、media switch、viewer close，以及 application shutdown when possible。状态关联 media ID + content version；旧 owner/旧序号/旧版本不得覆盖新进度，保存失败可重试。
- 播放器按既定 Direct → Remux → Proxy 回退，只读原件，派生资源放应用缓存；后台转换可取消，单文件失败不使 UI 崩溃。
- 不实现 Play Event/资格/completed 新业务，不增加 play_count；保存位置不属于 Play 生命周期。完整测试后停止，下一 Phase 才实现 Play Event。

## Phase 7 授权范围

- 每次主动 Like 新建 request_id 和事件，同一动作重试沿用 request_id；按钮与 L 调同一后端 add_like 服务。支持单项及全部 Like/View/Play 计数重建。
- View 仅在媒体成为查看器主媒体后记录，按 activation 去重，A→B→A 三次；hover/预取/重渲染不计。source 支持 grid、normal_viewer、quick_browser、search、history；本轮不新增 Quick Browser 或 History 页面。
- Play 沿用下文已批准的五秒/短片真实 ended 定义，打开播放器不计数；资格时插入，关闭/切换时受限终结同一事件。播放进度独立保存，不用位置更新代替观看证据。
- 事件插入、数据库触发器计数更新、活动重建增量及工作状态必须事务原子提交；失败回滚。rebuild_statistics 从日志修复三个计数及 last_viewed_at/last_played_at，后台分批执行、可恢复和取消。
- 测试必须覆盖 100 rapid Likes、事务回滚、计数重建、View 去重/A→B→A、有效/无效短播放、completed 和统一统计；失败修复后重跑完整测试，到此停止。

## Phase 8 查询范围

- 搜索可选文件名、System Tag、Custom Tag 或全部字段。字面子串匹配支持中文和 SQLite 的 ASCII 大小写折叠。System Tag 匹配包含后代目录。
- 媒体类型、System Tag、Custom Tag、Like/View/Play Count、导入日期和已查看状态可以组合。Custom Tag 多选支持 AND 与 OR。计数上下限及导入起止日均包含端点。已查看按 View Count 大于零判定。
- Like、View、Play、Imported、Created 和文件名支持升降序。另提供 Recently Viewed、Recently Played 和 Random。缺失时间按最早值排列，稳定媒体 ID 打破排序并列。
- Random 使用稳定 seed 和版本。分页、刷新、重新渲染和重新选择 Random 均复用当前 seed。用户点 Shuffle Again 时生成并保存新 seed。
- SQLite 执行查询和排序。前端按游标获取有界页面并虚拟渲染。验证 100,000 项媒体的普通和随机分页、组合筛选及索引使用情况。

## Phase 9 Quick Browse 范围

- 从当前 search/filter/sort 建立持久化浏览会话，保存 ID、started_at、ended_at、排序、完整 filter snapshot、random seed 和算法版本。会话内固定成员和 ordinal，点赞、标签编辑、扫描、分页及重渲染不重新查询成员；内容失效项保留位置并允许继续浏览。
- Left 上一项，Right/Space 下一项，L 调统一 Like Service，T 切换标签选择器，F 全屏，I 切换信息，R/Shuffle Again 生成新 seed 并建立新会话，Esc 退出。输入框编辑和快捷键设置内不触发媒体快捷键；键盘自动重复不重复执行动作。
- 数字 1–9 可绑定平级 Custom Tag，按键执行事务化 Toggle，并显示短暂的添加/移除反馈。同一次请求重试不能反向切换第二次；标签始终仅修改数据库关联。
- Quick Browse 的 View source 为 quick_browser，遵守 activation 语义；Like、独立播放进度、有效 Play 判断均复用既有服务。视频可配置 autoplay 和默认 mute，默认不自动播放、默认静音。
- 后台构建、可取消、失败可恢复；SQLite 负责筛选排序，前端只获取当前成员和当前原件。完成测试后停止，不新增历史会话恢复页面或会话保留期策略。

## 长期核心原则（已批准）

| ID | 约束 |
| --- | --- |
| R01 | media_id 表示逻辑媒体文件实例；path 和 fingerprint 都不是 identity。 |
| R02 | System Tag 仅由真实物理目录层级生成，数据库保存扫描投影；离线时标明上次观察状态，不冒充实时事实。 |
| R03 | Custom Tag 完全独立于文件系统，只关联媒体 ID，不改变文件名、目录或位置。 |
| R04 | Like、View、Play 使用 Event Log。 |
| R05 | like_events、view_events、play_events 是 Source of Truth。 |
| R06 | media.like_count、view_count、play_count 等仅为 Counter Cache。 |
| R07 | 每次有效事件插入与对应计数更新必须同一 SQLite transaction 提交或整体回滚。 |
| R08 | 必须支持从日志重建单媒体、单种及全部统计，包括零事件媒体。 |
| R09 | 普通启动不得重建整个数据库；优先打开已有数据库并使 UI 可用。 |
| R10 | 所有扫描支持 incremental scanning；启动后的增量扫描在后台，不等待全库扫描才开放 UI。 |
| R11 | 扫描、指纹、缩略图及播放代理生成不得阻塞 UI，必须使用有界任务队列。 |
| R12 | 单个损坏/不可读媒体及解码失败不得使整个应用崩溃。 |
| R13 | 用户原始媒体默认只读，不得删除实际媒体文件；清理只处理应用自己的缓存。 |
| R14 | Schema 修改必须经过可追踪 migration，不得临时改表绕过迁移。 |
| R15 | 合理的 Bug 修复应增加 regression test；不适用时说明理由。 |
| R16 | 不得为了完成某 Phase 破坏确认的数据模型和模块边界。 |
| R17 | 重大需求、架构及 Schema 修改先说明理由、影响与迁移策略，再实施。 |

## 媒体身份、重复内容与扫描（已批准）

- 两个不同路径的独立文件，即使 size、fingerprint、full hash 完全相同，默认仍为两个 media_id；禁止按摘要自动 deduplicate。fingerprint 值不得设置 UNIQUE，其用途仅为 reconciliation evidence、重复检测及未来重复内容 UI。
- 高置信度 rename/move 保留 media_id、imported_at、Custom Tags、三个 Event Log 和 playback state；更新 file_path、必要的 filename、System Tags、last_seen_at 及 filesystem metadata。file_path/filename 可由当前位置投影返回，无须重复存放在 media 主表。
- **False negative is preferable to false positive**。cross-volume、copy then delete、多份相同副本等无法可靠匹配时，对账结束后旧 media 标 missing，新文件创建 new media；不能无限 pending，也不能猜测性继承历史。临时文件读取/扫描尚未结束可 pending；离线或无权限不能伪装成“确认旧路径消失”。manual relink 不属于当前 Phase。
- 同一路径被确认替换为不同媒体：旧 media 标 superseded（历史保留、旧位置退役），新内容使用 new media_id、new imported_at，三个计数为零，不继承标签和进度。同路径、同 file ID 均不能单独证明可继承。
- new/existing/moved/renamed/modified/missing/restored 均需明确判定。modified 是内容变化观察；若确认已是不同媒体，必须走 replacement，而不是仅增加旧媒体版本。仅经高置信度确认仍属同一逻辑实例的变更可增加内容版本，证据不足保守分离。
- 移动/重命名后的身份确认应保留全部关联；不能自动匹配的情况下，保留旧历史但不保证将其自动挂到新文件上。

## Like（已批准并冻结）

每次用户主动 Like 都新增一条 like_event。Like 不是 Boolean，没有 Unlike、toggle、撤销或负向 delta。同一媒体可以累计任意多次 Like，受实现整数范围限制时须明确报错，不能溢出或丢弃。

例如五次主动 Like = 五条事件、like_count 增加五。每次主动动作有新的 request_id；同一动作的 IPC 重试沿用原 request_id，不能误当成下一次 Like，也不能把多次主动按键合并成一次。

同事务执行 BEGIN → INSERT like_events → UPDATE media.like_count → COMMIT；任一步失败整体 rollback。必须提供 rebuild_like_count(media_id)、rebuild_all_like_counts() 和统一 statistics rebuild。like_count 的重建公式为 COUNT(like_events)，不再使用 SUM(delta)。

## View = Media Activation（已批准并冻结）

媒体真正成为 Image Viewer、Video Player 或 Quick Browse 的当前主媒体时产生一次 View。A → B → A 产生 View(A)、View(B)、View(A)。

使用稳定 activation identity，每个 activation 只记一次；离开后再次成为当前主媒体必须获得新 activation。React rerender、StrictMode 重挂载、resize、state update、fullscreen update 不产生新 activation，不能以时间 debounce 定义业务。

Grid 点击并真正打开媒体、Quick Browse 切换当前媒体均算一次。thumbnail hover、lazy loading、scrolling into viewport、background preload 不算。仅预取或尚未完成主媒体切换不建立 activation；该业务不以停留时间或播放资格为前提。

view_events 是事实来源；view_count = activation 对应的有效事件数。播放器内部 Direct/Remux/Proxy 切换沿用 activation，不额外计 View。

## Qualified Play Sessions（已批准并冻结）

- 打开 Video Player 不算 Play；达到资格才插入一条 play_event，play_count 表示 Qualified Play Sessions 数量。
- 普通视频累计真实播放时间 >= 5 seconds 才 qualify；总时长 < 5 seconds 的视频，真实播放至结尾即可 qualify。恰好 5 秒按普通视频规则。
- watched_seconds 累计实际播放经过的时间，不把 seek 跳跃、缓冲、暂停、加载、恢复位置算观看时间。倍速时使用实际经过的播放时间，不用媒体时间跳变量冒充时长。
- 同一 player session 内 play/pause/resume/seek/重复播放仍最多一条事件。切换媒体、关闭播放器或明确结束该媒体 player session 后再次播放可以创建新 session 并形成新事件。重播按钮本身不自动划分新 session。
- 短视频资格依赖真实 ended，不依赖 completed 阈值；不能仅通过 seek 到尾或恢复近尾位置使其 qualify。未知/无效 duration 无法认定为短视频，先按 >=5 秒规则处理，取得有效时长后可按该 session 的真实 ended 证据核验。
- 自动播放若真实发生，也按同一资格与 session 规则，不以按钮点击作为判断。仅后台 preload 不算播放；播放失败前若已满足资格，已提交事件保留。
- play_events 至少支持 id、media_id、content_version_id、started_at、qualified_at、ended_at、start_position、end_position、watched_seconds、completed。资格时插入、结束时受限补全同一事件；不创建第二条 Play，不再次加计数。异常退出不能虚构未观察到的观看时间。

## Playback State 与 completed（已批准）

playback_states 独立关联 media_id 和内容版本，至少包含 position_seconds、duration_seconds、updated_at、completed。保存进度不得创建 Play Event，不要求先产生 qualified event 才可保存。

同内容移动/重命名保留状态；替换为新媒体不继承。原媒体的旧版本状态保留，内容版本变化不得隐式套用旧状态。多播放器并发用版本校验和明确 owner 接管，具体窗口 UX 可后续确定。

有效 duration > 0 时，位置先限制在 [0,duration]，completed =
progress >= 95% OR remaining_time <= min(10 seconds, duration * 5%)。
其中 progress = position / duration，remaining_time = duration - position；duration 未知/零/无效时 completed=false（待有效信息后重算）。禁止简单以 remaining < 10 seconds 判断短视频完成。completed 不是 Play qualification，seek 可改变完成状态但不能凭此生成 Play。

## 三层播放与派生缓存（已批准）

依序选择 Direct Play → FFmpeg Remux → FFmpeg Transcode Proxy：
WebView 能直接播放则 Direct；codec 可播放但 container 不兼容时优先 remux、不重新编码；codec 不兼容或 remux 仍不能播放时按需生成 proxy。探测与真实播放失败可触发下一层，但不得引入额外 activation 或 player session。

代理放 cache/playback/，例如 cache/playback/<media_id>/<content-version>.mp4。原文件永不修改；代理可安全删除和重新生成，版本改变后旧代理不能作为当前版本播放。使用 bounded/LRU 策略。优先 H.264 + AAC MP4，具体 profile、分辨率及 FFmpeg 构建选项按目标 WebView 验证并记录架构决策。

FFmpeg 任务有全局并发预算；播放 remux/proxy 默认低并发 1 job，后续可配置为 1–2，不允许无界转码。生成可取消、可报进度，失败需隔离。

## 100,000+ Media 与稳定浏览（已批准）

- 前端禁止一次加载全部媒体对象；Grid 必须 virtualization，thumbnail 必须 lazy loading，有界预取与缓存。
- SQLite 执行 sorting/filtering/search，增量分页默认约 100 条，使用 keyset/cursor，深分页不依赖巨大 OFFSET；React 不做全库排序过滤。
- 配置 foreign_keys、适用环境的 WAL、busy_timeout，并为高频查询建索引。
- Scanner、thumbnail 和 FFmpeg proxy 都有独立有界队列及共享资源上限；不得用异步函数名掩盖阻塞 UI 的实际工作。
- Counter rebuild 后台执行、资源有界，不能长时间使 UI 完全不可用；采用分批统计修复与并发事件补偿，详见架构。
- Random 保存稳定 seed 和算法版本；Quick Browse 固定成员/顺序，点赞、扫描、分页、重渲染不会隐式洗牌。主动更改过滤或 reshuffle 创建新会话。
- 性能未实测不是 Phase 1 阻塞项；后续 Phase 在明确硬件、库规模、事件量和预算后验收。

## 后续细节与阶段影响（均非 Phase 1 blocker）

| 项目 | 当前约束及决策时点 |
| --- | --- |
| 页面/MVP、搜索字段与排序组合（原 Q01） | 已冻结数据/API 边界，Phase 4 前细化页面验收及索引。 |
| System Tag 根名称与祖先筛选 UX（Q06） | 只能来自真实目录，展示政策 Phase 4 前定案。 |
| Custom Tag（Q07 已在本轮关闭） | 无子标签、可 CRUD、标准多对多；trim 后非空、SQLite NOCASE 名称唯一。 |
| 系统/格式矩阵与性能硬件（Q08） | Phase 1 初始化锁定当前构建目标；兼容矩阵和量化性能在 Phase 3/6 验证，不阻止数据基础。 |
| FFmpeg 许可、分发及 profile（Q09） | 三层架构已批准；实现选择于 Phase 3、发布前完成许可验证。 |
| 根移除、保留、备份 UI（Q10） | 不删除原文件或日志；正式升级须一致性备份，数据清理需另行设计。 |
| 网络盘/链接/多实例（Q11） | 当前设计默认单写实例、不跟随链接、拒绝重叠根；扩展支持另行验证。 |
| 深度核验频率（Q12） | 普通增量不保证发现同大小同时间戳改写；显式深度核验是后台任务。 |
| Quick Browse 生命周期（Q13） | 技术上可恢复固定顺序；保留期/恢复交互在 Phase 4 确认，不静默洗牌。 |
| 多窗口接管/同一实例编辑证据（Q14） | CAS 和 owner 防陈旧写入；证据不足按新媒体处理，不要求 Phase 1 猜测政策。 |

原 Q02–Q05 身份、Like、View、Play 已关闭；Q09 三层播放和 Q14 独立状态/替换隔离部分已关闭。manual relink、自动内容去重、原文件编辑删除均不属于本轮实施范围。

## 变更纪律

重大变更先说明原因、影响和 migration，再同步五份文档和决策历史。批准的产品语义不得被工具选型、性能捷径或某个 Phase 的便利性覆盖。当前仅授权 Phase 2 数据层；测试结果和环境限制如实记录，不把 SQL 测试或设计通过等同于 Rust 原生验收。
