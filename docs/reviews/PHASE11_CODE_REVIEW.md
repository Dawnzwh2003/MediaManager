# Phase 11 repository code review

日期：2026-11-12。先完成 robustness review 与复现修复；最终两种 Rust 各 100 项、Node 55 项、监听/build/fmt/两种 Clippy、两个真实规模 UI 各 13 项通过后，再进行本次全仓复审。工作区基线是 Phase 10，审查包含本轮差异及其依赖模块。

## 覆盖范围

| 区域 | 核对内容与结论 |
| --- | --- |
| 根规范、manifests、锁文件、Vite/Tauri 配置 | 技术栈、严格类型、构建入口、watch 忽略、feature 配置、CSP/capabilities；无依赖或锁文件变化，只有 local main 权限，无 SQL/fs/shell 插件 |
| `src/api`、`src/app` | IPC 只在类型化 API 层，耗时命令 spawn_blocking，错误脱敏及失败重试；无 SQL、任意文件/外部进程 API |
| `src/library`、查询 controller | 每页 100、换页释放旧项、游标 trail 上限、过期响应丢弃、选择上限；Grid 只 slice/map 可见行，缩略图只请求可见 ID，poll/timer 卸载清理 |
| `src/reports`、ReportsService/Repository | 本地日历窗口、五类日志统计、current-version CW、有限 Dashboard 预览；SQLite 聚合和 keyset，React 不载入事件表，限额和 stale cursor 拒绝明确 |
| `src/viewer`、`src/quick-browse` | 进度串行/重试 owner+CAS、activation 与 render 分离、固定会话成员和 seed、快捷键输入边界；无前端全库排序或隐式洗牌 |
| Rust commands/domain/services | 用例边界、DTO 参数上限、事务协调与仓储职责；事件资格留在活动服务，未绕过服务手工累加 Cache |
| DB lifecycle、v1–v11 migrations | 单实例锁、WAL/FULL/busy_timeout、校验历史先于持久 PRAGMA、备份、逐迁移回滚；v11 仅两个索引，v1–v10 字节内容未改 |
| Library/catalog/random/tag repositories | 参数化 SQL、封闭排序表达式、签名/代次 keyset、关系索引、磁盘临时排名；O(N) 首次 Random 发生在 SQLite/后台，热页复用 |
| Scanner/fingerprint/identity | 文件实例身份、保守移动/替换、根标签、覆盖目标退役；最多 100 准备项，文件 I/O/哈希在事务外，磁盘队列/观察；失败不整体 missing，取消/回滚可重新对账 |
| Metadata/Thumbnail/process | 单消费者、全局一个子进程、输出/像素/超时/取消限制、按项失败、版本 CAS；启动 recovery 后台重试且分批，无无界进程/全库媒体内存 |
| Viewer backend/playback cache/platform paths | 原件只读 handle、opaque token/range 限制、内容版本核对、Direct/Remux/Proxy；只清理应用拥有且命名受限的缓存，pin/配额保留，不删除媒体 |
| Events/statistics repositories | trigger 唯一计数入口、插入与 Cache 同事务、只允许一次 Play 终结补全；base/delta/high-water 分批、并发 Like、取消和恢复，实际大库零差异 |
| 自动测试、UI harness、benchmark/examples、脚本 | 临时库/合成原件；正常、失败、恢复、迁移、规模、异常子进程；benchmark 目录/规模限制与拒绝覆盖；UI 包装仅观察生产 API，无全库 mock 注入产品状态 |
| docs/archive/generated 内容 | 四份指定文档、验收和审查索引同步；归档脚手架不参加构建，忽略的依赖/产物/夹具不视为新增产品源码 |

审查采用跨模块调用链、资源生命周期、事务/SQL plan、权限路径、规模测试及本轮完整 diff 的结合；不将 grep 命中或测试通过单独当作正确性证明。

## 实际发现及处理

本轮初审发现并已复现修复的六组问题：扫描路径/unseen 查询退化；逐文件耐久提交开销；启动同步恢复阻塞；播放器 claim 写升级 Busy；根目录漏标签；高置信度覆盖移动的路径冲突。每项有查询计划、故障注入或合成文件回归，见 [robustness review](PHASE11_ROBUSTNESS_REVIEW.md) 与 [验收](../validation/phase11-validation.md)。

测试后复审没有发现需要追加修复的功能缺陷，也未发现本轮违反身份、事件不可变、进度隔离、只读原件或前端有界加载的变更。没有为“优化”改写已稳定的随机排序、播放器、标签或统计算法。

## 已知限制与后续边界

- 百万 View 下 History 每页重聚合约 0.5s，Dashboard 约 1s。搜索是字面子串匹配，稀疏/空结果可能遍历候选；结果有界不等于数据库计算恒定时间。
- 生产 counter rebuild 保持有界事务和步间让出，但当前 E: 上并发 Like 最大等待约 5.57s；只证明正确性和资源边界，没有证明严格交互写延迟 SLA。
- Random 首建 100k 约 0.94s；扫描仍需读取文件证据，10k 小文件增量约 158s。这些是实测成本，不能用“无 UI 卡死”替代吞吐说明。
- 本轮为 Windows Debug/合成库；没有新增 release 打包、跨平台、长期运行或真实 100k 视频解码验收。既有各阶段手工 UI harness 保留；本轮真实 UI 专项是 10k/100k 性能入口，两种 Rust 全套包含所有已有 backend 回归。

上述限制如需改变预算或算法，应另立具体证据和授权；本轮停止。最终测试窗口、进程/端口及截图内核清理记录在验收文档。
