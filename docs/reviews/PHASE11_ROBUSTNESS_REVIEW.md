# Phase 11 robustness review

日期：2026-11-12。开始时工作区干净，基线为 Phase 10。先审查，再以临时数据复现；只修复实际问题，不改变产品语义或优先增加视觉动画。

## 初始检查

| 检查点 | 初始发现与需补充的证据 |
| --- | --- |
| 1 Scanner 内存 | 目录迭代、磁盘队列、逐条观察和匹配，无全库媒体 Vec；补充 10k/100k 存储模拟与进程内存、真实小文件扫描 |
| 2 Fingerprint | 64KiB buffer、最多 3MiB 样本、哈希不在事务内；补充大小对照和增量跳过实测 |
| 3 SQLite plan | 实际 EXPLAIN 复现 scanner by_path 全表扫描、unseen 每步全根排序；需修复 |
| 4 indexes | queue/pending/identity 索引有效，路径及 unseen 缺少匹配索引；仅为实证问题追加 migration |
| 5 Grid virtualization | 只 slice/map 可见行，controller 默认 100 项；补充 10k/100k DOM、滚动和事件循环响应实测 |
| 6 lazy thumbnails | 只请求可见 ID，有取消/轮询清理；补充请求批次和脱离窗口后释放观察 |
| 7 thumbnail queue | 磁盘状态、单 worker、每次一项；审查启动 recover 的同步写入和故障重试 |
| 8 FFmpeg concurrency | probe/thumbnail/remux/proxy 共用 PROCESS_GATE；已有失败/超时/取消用例，补充并发实证 |
| 9 Random | 绑定 seed/version，磁盘临时索引，热分页复用；补充两种规模首建/热页和内存 |
| 10 Search | SQLite 字面子串、cursor 页；补充稀疏/无匹配/组合范围 benchmark，不假定子串可直接 B-tree seek |
| 11 Tag filters | 关系表和递归 System Tag，无逗号字符串；补充 AND/OR/后代过滤 plan/benchmark |
| 12 Event growth | 不可变日志与事务 trigger，无启动删历史；补充大事件量数据库/WAL体积与查询/重建 |
| 13 History | SQLite COUNT/MAX/GROUP BY，每页重新聚合；补充 10k/100k 及事件增长后的成本 |
| 14 Startup | 既有 DB 先验证/打开；发现 worker spawn 在返回状态前同步执行 recover 写入，需持写锁故障复现 |
| 15 Migration | 备份、checksum、gap/新版本拒绝、单迁移事务；补充本次升级和进程中止后的重开 |
| 16 missing | 保守匹配，失败扫描抑制 missing；完整旧回归及规模扫描复测 |
| 17 corrupted media | 单项失败，工具输出/像素/时间有上限；运行全部真实 FFmpeg 用例 |
| 18 DB errors | 稳定错误码、回滚、不替换坏库；重点复测短事务写竞争和恢复 |
| 19 unexpected exit | durable queue、统计 checkpoint、资格事实只补全最后证据；增加实际子进程异常退出/未提交事务回滚 |
| 20 counter rebuild | 每批1000日志/100媒体，base/delta；已有并发用例，补充更大日志量与每步前台等待 |

## 已复现的 scanner 性能问题

诊断脚本执行仓库实际 v1–v10 SQL，以临时数据库生成记录；每项重复 100 次，原始结果 `.tools/validation/phase11/baseline-plans.json`。

| 查询 | 10,000 媒体均值 | 100,000 媒体均值 | 计划 |
| --- | ---: | ---: | --- |
| by_path（包含 missing） | 0.367 ms | 3.776 ms | SCAN media + 临时排序 |
| unseen 下一项 | 4.044 ms | 47.379 ms | 按 root 扫描后逐项联接及临时排序 |
| 磁盘扫描队列下一项 | 0.0015 ms | 0.0016 ms | scan_queue_pending 覆盖索引 |
| 缩略图队列下一项 | 0.0027 ms | 0.0021 ms | derivatives_pending 覆盖索引 |

前两项在逐文件/逐 missing 发布循环中重复，会放大为平方级扫描成本。计划追加 v11：非 superseded 路径/missing 索引，以及 (root,media_id,generation) unseen 索引；unseen 以 media_id keyset 驱动。仅新增索引，保留历史 migration、身份/事件/进度；升级前由正式 runner 备份。先添加实际查询计划回归，再实施修复。

## 复现后的修复与结果

初审后完成查询计划、持锁、合成文件及真实扫描复现，修复范围如下：

1. v11 追加路径/missing 和 unseen keyset 索引；100k 下一项 unseen 从 47.379ms 降至约 0.00155ms，路径查找从 3.776ms 降至约 0.00201ms。
2. 真实 10k 文件暴露逐项 FULL 提交开销，改为最多 100 个已准备项事务；观察/队列 checkpoint 原子提交，I/O 不进入事务。首次 747.882→123.497s，增量 610.489→158.224s，增量 fingerprint=0。批量注入失败回滚、超限拒绝、取消后重扫恢复均通过。
3. 持写锁时同步启动恢复阻塞就绪，Thumbnail/Quick Browse recovery 移入各自后台线程并重试；Thumbnail 每批最多 1,000，旧 pending 版本不死循环。
4. 播放器 claim 在 150ms 竞争写锁下读后升级 Busy，使用 IMMEDIATE 后等待成功；不改变 owner/CAS/进度及 Play 资格语义。
5. 根目录本身漏 System Tag、高置信度移动覆盖已有目标 UNIQUE 冲突，均先以合成文件复现；修复后源 ID/导入时间/标签/进度保留，两份历史都保留。

测试和规模结果详见 [Phase 11 验收](../validation/phase11-validation.md)。两种 Rust 配置各 100/100、Node 55/55、监听/build/fmt/两种 Clippy 通过；两个真实 Tauri 规模各 13 项通过。测试后完成 [全仓复审](PHASE11_CODE_REVIEW.md)。本次未发现全库 React rendering 或前端统计事件表；保留 History 聚合和重建写入尾延迟的实测限制，无无理由模块重写。
