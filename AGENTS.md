# Repository Guidelines

## 文档与变更纪律

- `REQUIREMENTS.md` 保存业务需求；`ARCHITECTURE.md` 保存架构与 Schema；`PLANS.md` 保存阶段、验收与状态；`DECISIONS.md` 只记录重要决策及变更理由。
- 未确认的产品细节必须标明，已批准的规则不得反复标为待审核。重大需求、架构或 Schema 修改先说明理由、影响和迁移方案，再实施并同步文档。
- 不得为了完成某个 Phase 破坏已确认的数据模型或模块边界。设计就绪不等于实施授权；遵守用户当前的设计/实施范围。

## 固定技术栈与边界

- 使用 Tauri、React、TypeScript、Rust、SQLite、FFmpeg。前端通过类型化 Tauri API 调用后端，不直接操作数据库、文件系统或外部进程。
- Rust 负责业务规则、文件访问、后台任务和事务。数据库写入集中管理；耗时 I/O、扫描、哈希与缩略图生成不得阻塞 UI。
- 模块之间通过明确接口交互，不绕过服务层直接改变其他模块的数据。
- 服务按用例拆分；数据库写协调器只负责调度事务，不能成为包办扫描、播放、查询和标签的巨型服务。

## 不可违反的数据原则

- media_id 表示逻辑文件实例；path/fingerprint 都不是 identity。独立文件即使摘要相同也不自动去重，fingerprint 值禁止唯一约束。
- 高置信度移动保留 ID、imported_at、标签、事件与播放状态；不可靠匹配在对账结束后按旧 missing + 新媒体处理。确认同路径替换须新 ID、零历史继承。
- System Tag 仅反映物理目录层级；Custom Tag 永不改变物理文件位置。原件默认只读。
- Like、View、Play 使用 Event Log；`like_events`、`view_events`、`play_events` 是 Source of Truth，`media` 的统计字段仅为 Counter Cache。
- 事件插入与 Counter Cache 更新必须在同一个 SQLite transaction 中完成。必须支持从 Event Log 重建全部计数。
- Like 每主动动作一条事件，无 Boolean/toggle/撤销。View 按 activation 而非 render。Play 仅记录 qualified player session；准确资格及 completed 规则以 REQUIREMENTS.md 为准。
- 事件身份及资格事实不得改写/删除；Play 只允许一次受限终结补全，不得二次计数。进度保存不属于 Play 生命周期写入。
- 事件、自定义标签和播放状态关联稳定媒体 ID，不关联路径。播放进度与 Play Event 分开持久化；进度写入不得增加播放计数。
- Random 顺序必须绑定稳定 seed 与算法版本；Quick Browse Session 固定成员和顺序，分页、重渲染及后台扫描不能隐式洗牌。
- 普通启动不得重建整个数据库；所有扫描支持 incremental scanning。单个损坏媒体不得导致整个应用崩溃。
- 已有数据库和 UI 优先可用；后台扫描不作为 UI 开放条件。统计重建必须资源有界、短事务分批执行，不长期独占写入或阻塞 UI。
- 100,000+ 媒体采用 SQLite cursor/keyset 查询、虚拟 Grid、lazy thumbnails；禁止前端加载全库排序过滤。扫描、缩略图和 FFmpeg 均有界并发。
- 播放依次 Direct/Remux/Proxy；所有派生资源写应用缓存，内容版本隔离，可回收再生成，永不覆盖原媒体。
- 用户媒体默认只读；未明确新增相关功能前，不得删除实际媒体文件。缓存清理只能处理应用拥有的缓存目录。
- Schema 修改必须经可追踪 migration；禁止临时改库绕过 migration。

## 工程与测试规范

- Rust 使用 `rustfmt`、Clippy 和 `snake_case`；React 组件使用 `PascalCase`，TypeScript 函数与变量使用 `camelCase`，启用严格类型检查。具体工具版本在初始化时锁定。
- 测试覆盖身份匹配、增量扫描、事件幂等、事务回滚、计数重建、migration、损坏媒体隔离及权限边界。合理的 Bug 修复增加 regression test。
- 回归须覆盖移动/重命名后的关联保留、播放进度隔离、稳定浏览顺序；规模验收包含 100,000 媒体的分页查询和虚拟 Grid，不以设计推断代替实际测量。
- 测试使用临时数据库和合成媒体，不依赖真实私人媒体，不修改用户文件。新行为包含正常、失败和恢复场景。
- 提交保持单一目的；PR 说明行为、验证结果、关联问题及迁移影响，UI 变更附截图。命令仅在实际配置后记录，不宣称未执行的检查已通过。
