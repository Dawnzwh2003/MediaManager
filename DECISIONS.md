# 重要技术决策记录

## 记录规则与批准来源

仅收录影响数据模型、架构边界、持久化语义和重要选型的决策，不记录普通实现细节。状态使用：已批准（Approved）、提案、已取代。当前 D001–D017 均为本轮批准规则下的设计基线；实现参数和后续 UX 不能把这些规则重新标成 pending。

日期：2026-11-01。本轮用户正式批准身份、保守匹配、事件语义、播放状态/兼容和规模规则，并要求将一致的 D009–D014 转为 approved，不一致的修改并记录原因。下面保留旧提案/旧审查结论的变更摘要，不把旧语义继续作为现行约束。当前无真实 DB，仅改逻辑设计，无数据迁移执行。

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

- 状态：**已批准（Approved）**；来源：用户本轮“不要开始 Phase 1”。
- 决策：本轮仅维护五份文档并复审；Ready=YES 也不创建代码、脚手架、数据库或迁移文件。
- 变更：旧“完整页面/所有未来问题未定即 NO”的门槛已收窄；仅真正改变 Phase 1 基础架构的业务 blocker 可阻止放行。性能实测属于后续验收。
- 影响：当前 Phase 1 未开始，等待后续开始指令，不把设计批准等同于实现批准。

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
- 当前结论：**Ready for Phase 1: YES**。性能/兼容实测留作对应 Phase 验收，不把未测量误当业务 blocker；本轮仍不开始实现。

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

## 后续事项（非当前 Phase 1 blocker）

版本/驱动选型在 Phase 1 锁定；平台播放 profile/许可在 Phase 3 验证；页面、标签展示、Quick Browse 保留 UX 在 Phase 4 细化；性能预算和实测在相关 Phase/最终规模验收完成。未来 manual relink、原文件编辑/删除或内容聚合必须另立决策和迁移，不是当前功能。当前无待冻结的身份或 Like/View/Play 核心业务语义。
