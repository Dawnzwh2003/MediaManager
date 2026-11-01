# 分阶段开发计划

## 当前状态与门槛

本轮已将用户批准的产品规则写入设计，并完成新 15 项 architecture review：**Ready for Phase 1: YES**。没有未解决的 Phase 1 业务语义 blocker。上轮 NO 的身份、Like/View/Play、播放状态及兼容架构问题已关闭；该历史不再作为当前门槛。

**Phase 1 未开始。** 用户明确本轮只更新设计，不创建代码/脚手架/数据库/migration、不安装依赖。设计就绪与开始实施是两件事，等待后续明确开始指令。未执行应用构建、功能测试或性能测量；文档核验不代替功能验收。

状态区分：设计完成、未开始、进行中、待验收、已验收、阻塞。完整页面清单、后续平台矩阵和性能实测不阻止基础工程阶段；应在它们所属 Phase 细化并验收。重大变化先说明原因并维护决策历史。

## Phase 0 — 规范、批准语义与架构复审

- **状态：设计完成；本轮复审通过。**
- 交付：五份文档职责、目录/模块、总体 Schema、Tauri 边界、增量扫描、指纹、缓存、会话事件与阶段路线。
- 验收结果：逻辑实例身份、重复内容分离、保守移动、替换隔离、累加 Like、Activation View、Qualified Play、独立进度、completed 和三层播放已正式批准；新 15 项检查无 Phase 1 blocker。
- 剩余：后续功能 UX 和量化预算按下列阶段推进，不再要求关闭所有未来问题才初始化骨架。本轮到此停止。

## Phase 1 — 工程骨架与数据库基础

- **状态：未开始**；依赖：Phase 0 设计就绪及后续实施指令。
- 目标：锁定 Tauri/React/TypeScript/Rust/SQLite/FFmpeg 工具版本，建立模块骨架、类型化 IPC、最小权限、数据库生命周期与 migration runner；确定当前构建平台。
- 验收：骨架可构建启动；README 记录实际命令；已有 DB 普通重启不重建；foreign_keys/WAL/busy_timeout 正确；空库/旧库/迁移失败与 checksum 测试通过；不兼容库拒绝写入，不静默新建空库；React 无 SQL/任意文件权限。
- Schema 验收：稳定 media/内容版本、非唯一摘要、事件唯一性、零计数默认值、superseded 及关联外键成立。无 Unlike/delta 模型；为后续终结补全和有界重建预留正式表/接口，不实现临时捷径。
- 不要求本阶段完成真实播放、全量 UI、100,000 性能实测或最终平台编码 profile；这些不属于 Phase 1 基础验收。

## Phase 2 — Identity 与增量扫描

- **状态：未开始**；依赖：Phase 1。
- 目标：根/目录投影、增量观察、版本指纹、保守 reconciliation、有界任务及恢复。
- 验收：覆盖 new/existing/moved/renamed/modified/missing/restored、跨根先后顺序、文件 ID 复用、相同副本与 pending 收尾；独立同 hash 文件不合并。
- 高置信度移动/重命名保留 imported_at、标签、三个日志与 playback state；模糊移动最终 missing+new；同路径替换旧 superseded、新 imported_at/零计数/无标签进度继承。
- 根离线/权限错误不批量误标 missing；取消恢复仍增量；未变项不重复昂贵处理；大视频采样读取有固定预算，完整摘要流式限流；扫描期间 UI 可用且原文件不变。manual relink 不在本阶段。

## Phase 3 — 元数据、Thumbnail 与三层播放缓存

- **状态：未开始**；依赖：Phase 2。
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

当前设计已具备 Phase 1 条件，**但本轮未获开始 Phase 1 的指令**。性能未实测和后续 UX 未定都不单独构成 NO；只有会改变 Phase 1 基础模型或边界的真实 blocker 才重新降级，并说明具体原因。
