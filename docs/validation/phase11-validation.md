# Phase 11 稳定性与性能验收

日期：2026-11-12，Windows x64/MSVC。按既有阶段纪律先读历史验收记录、完成 [robustness review](../reviews/PHASE11_ROBUSTNESS_REVIEW.md)，再复现、修复、跑全套测试，最后做 [全仓复审](../reviews/PHASE11_CODE_REVIEW.md)。无新产品功能、视觉动画、依赖升级或原件操作。

## 条件与可复现夹具

- Node 24.22.2、Rust stable 1.99.1、FFmpeg/ffprobe 9.0.2；Cargo locked/offline，Debug 构建。沿用既有工具链和锁文件。
- `src-tauri/examples/phase11_benchmark.rs` 使用真实 migration 和生产 Repository/Service。仅允许本 checkout `.tools/validation/phase11/scale-10000` / `scale-100000`，seed 拒绝覆盖已有库。媒体 80% image/20% video，有层级目录、稀疏文件名和 AND/OR 标签；合成事件使用真实表与计数 trigger。
- 两个规模各执行 catalog、history、grow、rebuild、worker-rebuild、startup；fingerprint 比较 1/4/64 MiB 文件。另以 10k 个真实合成小文件跑完整 scanner 两遍；100k scanner 使用磁盘 SQLite 存储模拟，没有创建 100k 个真实视频。
- 独立子进程约每 10ms 采样峰值工作集。RSS 是整个进程值，并非 Rust 分配器精确值；测试有编译和其他 I/O 并行，单次测量不构成 SLA。原始 JSON、进程输出和检查日志保存在忽略的 `.tools/validation/phase11/`。
- 可在全新夹具上执行 `cargo run --locked --offline --no-default-features --example phase11_benchmark -- 10000 seed`，将 10000 换为 100000 验证另一规模；之后 mode 可选 `catalog`、`history`、`grow`、`rebuild`、`worker-rebuild`、`startup`、`fingerprint`、`scan-batched`。grow 是追加日志，worker-rebuild 拒绝复用任务 ID，scan-batched 拒绝复用扫描库；重跑应使用新 checkout/保留原测量，不能把旧库当新基线。

## 20 项检查结论

| 检查 | 实测/故障证据与结论 |
| --- | --- |
| 1 Scanner 内存 | 10k/100k 磁盘记录逐项读取；完整扫描最多 100 个准备项，规模内存见下表。无全库媒体 Vec |
| 2 Fingerprint | 64KiB buffer、最多 3MiB 样本；4/64MiB 耗时接近。真实未变更重扫 fingerprint=0 |
| 3 SQLite plan | 实际 EXPLAIN 复现 by_path SCAN、unseen 重复排序；修复后均 SEARCH，无根级临时排序 |
| 4 indexes | v11 仅增加两个实证缺失索引；现有 queue/derivative pending、排序及关系索引继续复用，有 plan 回归 |
| 5 Grid virtualization | 两个真实 Tauri 规模各 13 项通过，DOM 最大 16 张卡片，分页不累积全库 |
| 6 Lazy thumbnails | 可见 ID 请求最大批次 16，翻页/滚动只请求有限项；合成记录无原件时显示失败占位 |
| 7 Thumbnail queue | 单消费者、持久状态；2,501 running 项分三批恢复，旧 pending 生成器不会无限恢复，故障可重试 |
| 8 FFmpeg concurrency | 四个并发调用以独占 marker 子进程证明无重叠；probe/thumbnail/remux/proxy 共用同一 gate。真实工具、超时、取消测试全部执行 |
| 9 Random | 首建磁盘排名、热分页复用；相同 seed 重载稳定，显式 Shuffle 才变化，见下表 |
| 10 Search | 稀疏/无匹配文件名、标签搜索均执行真实 SQLite；结果有界。子串搜索可能检查全部候选，未声称 B-tree 子串 seek |
| 11 Tag filters | AND/OR、System Tag 后代/叶子及组合条件 benchmark；EXISTS 关联索引，排序 keyset seek，无前端全库过滤 |
| 12 Event growth | 两规模 View 增至 100k/1m，事件/Cache 事务一致；记录 DB/WAL 体积，不删除历史 |
| 13 History | 全部五种历史、CW、Dashboard 实测；500 页遍历 100k 媒体的自动测试检查无重复遗漏。百万事件聚合成本明确保留 |
| 14 Startup | 已有 DB 状态打开和真实桌面首屏计时；持写锁时后台恢复不挡就绪，普通启动库字节数不变 |
| 15 Migration | v10→11 在 10k/100k 含备份实测；媒体 ID/事件/进度/FK/重开幂等保留，既有失败回滚/checksum/gap/较新版本拒绝全部回归 |
| 16 Missing | 移动/重命名/恢复/同路径替换及失败扫描不批量 missing；根目录标签和覆盖移动新增回归 |
| 17 Corruption | 损坏视频/图片逐项隔离，支持格式均真实生成缩略图；不会写原件或导致应用崩溃 |
| 18 DB errors | 坏库不覆盖、错误脱敏、事务失败回滚、150ms 写锁等待、批量 stage/publication 注入故障后重试 |
| 19 Unexpected exit | 真实子进程 exit(73)，已提交 Like/进度保留，未提交日志/Cache 回滚；重开恢复 running 队列，FK/quick_check 正常 |
| 20 Counter rebuild | 人为破坏全部缓存，生产 20ms cadence worker 并发 Like；计数及最后事件时间与日志零差异，记录总时间及前台尾延迟 |

## Scanner 与指纹

路径诊断每项重复 100 次。v10 基线 by_path 在 10k/100k 平均 0.367/3.776ms，unseen 下一项 4.044/47.379ms；v11 分别降至 0.001424/0.002009ms、0.001574/0.001549ms。pending 队列已有覆盖索引约 0.002ms，未重写。

| 测量 | 10k | 100k |
| --- | ---: | ---: |
| Scanner 存储模拟进程峰值 | 11.2 MiB | 12.5 MiB |
| 遍历 active unseen（9k/90k 项） | 180ms | 1,782ms |
| 1,000 次路径查找 | 21.7ms | 30.6ms |

真实 10k 小文件基线已有 v11 索引，尚未合并逐文件提交：首次 747.882s、增量 610.489s。最多 100 项事务后：首次 123.497s、增量 158.224s，约 6.1/3.9 倍改善；峰值工作集 12.38 MiB（基线 12.23 MiB）。两遍分别新建 10,000 / 未变更 10,000，第二遍零指纹。此夹具主要度量文件访问与数据库耐久写入，不包含解码，不能外推真实视频总耗时。保留 WAL/FULL，无一个覆盖整个扫描的事务。

指纹 10 次：1MiB 文件 p50 15.70ms；4MiB/64MiB 各只读取 3MiB，p50 46.29/46.46ms；进程峰值约 6.77 MiB。

## Library 查询（毫秒）

生产查询每页 100 项，最多 30 页；深游标直接 seek，未用 OFFSET。稀疏或无匹配少于 30 个样本，p95 仅作描述。

| 查询 | 10k | 100k |
| --- | ---: | ---: |
| Imported 分页 p95 | 1.015 | 1.039 |
| 深 keyset 页 | 0.616 | 0.776 |
| Random 首建 | 69.110 | 941.835 |
| Random 热页 p95 | 1.266 | 2.487 |
| 稀疏文件名 | 3.983 | 56.632 |
| 无匹配文件名 | 3.230 | 53.862 |
| Custom Tag AND p95 | 1.821 | 2.060 |
| Custom Tag OR p95 | 1.019 | 1.287 |
| System Tag 含后代 p95 | 1.260 | 1.231 |
| System Tag 叶子 p95 | 1.405 | 1.638 |
| 标签名子串 | 3.006 | 38.008 |
| 进程峰值工作集 | 12.16 MiB | 14.78 MiB |

## 事件增长、History 与重建

| 测量 | 10k 媒体 | 100k 媒体 |
| --- | ---: | ---: |
| 初始数据库 | 17,915,904 B | 179,011,584 B |
| View 数量增长后 | 100,000 | 1,000,000 |
| 增长后数据库 | 36,655,104 B | 374,394,880 B |
| 追加 9 倍 View 耗时 | 27.846s | 268.086s |
| 观测 WAL 最大值 | 4,915,192 B | 4,927,552 B |
| Most Viewed 页 p95 | 37.900ms | 499.858ms |
| Recently Viewed 页 p95 | 40.933ms | 483.732ms |
| Dashboard 一致快照 | 71.530ms | 995.187ms |
| Continue Watching 页 p95 | 0.595ms | 0.732ms |
| 生产 worker 重建总时间 | 23.496s | 201.200s |
| 重建期间 Like p50 / p95 | 42.37 / 121.99ms | 46.76 / 139.80ms |
| 重建期间 Like 最大值 | 3,650.77ms | 5,572.19ms |

增长每批 1,000 日志，trigger 原子维护 Cache；checkpoint(TRUNCATE) 后 WAL 为零。History 单独进程约 17 MiB 峰值，前端仅接收一页。五种 History 都与 SQL 聚合核对，All Time 仍读取 Event Log，不依赖正在重建的 Cache。

生产重建每步最多 1,000 日志/100 媒体，步间 20ms，10k/100k 并发成功 Like 345/3,106 次，最终计数及时间戳差异均为零。另有不间歇调用 step 的压力测试：总时间 23.926/105.946s，前台 p95 3,153.99/1,432.83ms；不能将该模式当作生产调度。

**保留的性能限制**：百万事件每页重新 GROUP BY，View History 约 0.5s、Dashboard 约 1s；耐久写入重建期间仍有秒级前台尾延迟。本轮有界分批及异步 UI 已验证，但没有证明严格写延迟上限。未用放松 synchronous、删除事件或新增未经证明的缓存算法掩盖成本；发布版和更慢存储须另测。

## 启动、迁移与真实界面

已有库 AppState 首次 status 10k/100k 为 13.017/12.669ms，20 次热 status 约 0.041ms；重开前后数据库字节数一致。v10→11 含备份的最终测试为 44.117/357.774ms（另一配置结果见原始日志）。这是单次测量，迁移成本含索引创建，普通重启不会重建。

真实 WebView2 使用 `tests/ui/performance.html?run=1&size=...`，直接导入产品 App，包装 API 仅观察调用，不 mock 后端或把夹具装入 React。两种规模分别 13 项；包括 10 次 next、3 次 previous、深滚动、稀疏/无匹配搜索、Random 刷新、AND/OR、事件导致 cursor stale 后刷新恢复。

| UI 指标 | 10k | 100k |
| --- | ---: | ---: |
| 最大媒体卡片 DOM | 16 | 16 |
| 最大页 / 缩略图请求批次 | 100 / 16 | 100 / 16 |
| 累计请求不同缩略图 ID | 236 | 242 |
| 最大页 payload（JSON 字符） | 23,018 | 23,121 |
| 本次查询次数 | 27 | 27 |
| 文档开始到首 Grid | 725.5ms | 515.6ms |
| OS 进程创建到首 Grid | 1,449.2ms | 1,227.4ms |
| 最大 16ms heartbeat 间隔 | 71.0ms | 69.2ms |

上述为最后一轮开发构建、已有 WebView profile/OS 缓存；不代表发布版冷机启动。JS heap 观测为约 14.8/14.5 MiB，非整个 WebView RSS。测试固定尺寸 1400×920，不宣称每种窗口尺寸都固定 16 卡片。之前并发 I/O 运行的最大 heartbeat 间隔为 10k 428.5ms / 100k 353.8ms，仍通过一秒检查；没有删除较慢测量或只声称一次最好结果代表全部条件。

测试修正记录：100k 稀疏查询恰好返回满页，原 harness 只等“100 项”导致先检查旧页面而误报；增加等待实际稀疏名称，Random 刷新及标签还等待新 IPC 完成。没有为通过测试修改产品查询逻辑。

截图：[10k](phase11-grid-10000.png)、[100k](phase11-grid-100000.png)。

## 完整自动检查

在仓库根目录，设置实际缓存与工具路径：

```powershell
$env:CARGO_HOME='E:/MediaManager/.tools/cargo-validation'
$env:RUSTUP_AUTO_INSTALL='0'
$env:MEDIA_MANAGER_FFMPEG_DIR='E:/MediaManager/.tools/ffmpeg/ffmpeg-9.0.2-essentials_build/bin'
npm.cmd test
npm.cmd run test:dev
npm.cmd run build
cargo test --locked --offline -- --include-ignored --nocapture
cargo test --no-default-features --locked --offline -- --include-ignored --nocapture
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --all-targets -- -D warnings
```

结果：Node 55/55，监听 1/1，严格 TypeScript/Vite build 通过；Rust 两种配置各 100/100，零忽略（实际执行真实 FFmpeg 用例）；fmt、两种配置 Clippy 均通过。Rust 分组为 31 unit + 12 activity + 7 derivatives + 6 foundation + 8 library + 8 quick_browse + 6 reports + 7 repositories + 7 robustness + 8 viewer。

最初一轮 Rust 构建曾因正在运行 target 内 benchmark 的 Windows 文件锁出现 LNK1104；改为运行复制到隔离 `.tools` 的测量二进制后完整重跑通过，原失败日志保留。没有修改系统执行策略或绕过失败检查。

最终范围：只修复实证问题，旧 migration、锁文件与用户媒体未改。全仓复审结论、后续限制和最终清理见关联复审文档及 PLANS.md。未执行本轮 release 打包或跨平台测试，不将历史结果冒充本轮验证。

最终退出后只读检查两个规模：全部计数/最近事件时间与日志零差异，foreign_key_check 零错误，quick_check=ok。测试应用和测量子进程已退出，原生窗口列表为空，1420 无监听，截图 Node 内核已重置；保留隔离夹具及日志供复查。四份指定文档及审查/验收的本地链接存在性检查、最终 fmt/diff-check 通过。
