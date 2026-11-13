# Phase 12 — Folder Import 验收

日期：2026-11-13；Windows x64，Rust/Cargo 1.99.1、Node 24.22.2、FFmpeg/ffprobe 9.0.2。先阅读五份规范并检查 Scanner、Fingerprint、Media Repository、Tag、Event Log、Storage，再更新 REQUIREMENTS/ARCHITECTURE/PLANS/D034 的设计和 v12 migration plan，随后实施。保留上一轮 Release Review 的未提交工作。

## 实现与边界

- 原生 Tauri folder picker → 后台预览 → 明确开始 → 文件级进度、取消、恢复/重试、完成统计。任务与目录/文件队列持久化在 SQLite；前端只接收聚合、最多 20 个错误和每页最多 100 个来源节点。
- 默认 app-local-data/data/media/<media_id>/<原文件名>。Settings 配置后续文件根，旧文件/数据库不隐式迁移。保留原始来源；所有存储根与来源互不包含；链接/reparse 不跟随。
- filename+size 候选分页 50；复用现有 Fingerprint 模块的完整 SHA-256 扩展确认内容。异名同内容独立，quick-v1 不足以合并。旧外部媒体可复制托管而保留原 ID、版本与历史；Scanner 默认独立实例规则不改。
- 64KiB 流式复制/Hash，单 worker；create_new part、sync、原子无覆盖完成，再单文件短事务发布全部媒体/来源/物理关联。空间预检及逐文件复检，单项失败继续。完成媒体不删除。
- Source Tags 为独立、只读、多来源树；System Tags 继续对应实际物理目录，Custom Tags 保持独立。Source Tag 包含子目录的筛选复用 SQLite keyset 与现有虚拟 Grid。

## 用户指定的 21 项

| 项目 | 实际测试 |
| --- | --- |
| 1 图片 | 真实 JPEG 导入，副本可读且原件保留 |
| 2 视频 | FFmpeg 合成 H.264 MP4，导入、metadata/thumbnail 和 Viewer 打开，零活动事件 |
| 3 多级目录 | 多分支及 24 层目录，来源树父子关系正确 |
| 4 同名同内容 | 同一 media_id，来源关系追加 |
| 5 同名不同内容 | 相同大小、不同完整 Hash，新 ID，无覆盖 |
| 6 异名同内容 | 两个独立媒体实例 |
| 7 重复导入 | 新媒体/复制/新标签关系为零，ID/imported_at 不变 |
| 8 多来源标签 | Animal/Cat 与 Travel/Japan 关联同一实例及各祖先 |
| 9–11 Like/View/Play | 通过活动服务创建真实资格事件，导入后各计数与日志保留 |
| 12 Custom Tag | 旧外部媒体复制托管后关系保留 |
| 13 Playback State | position/state_version/updated_at 和内容版本保留 |
| 14 中途取消 | 第二次复制时取消，已提交第一项保留；part 清理，恢复后无重复 |
| 15 复制失败 | 注入部分写入后的 PermissionDenied，隔离失败并成功处理其他文件；重试恢复 |
| 16 空间不足 | 注入 available=0，开始前拒绝，无媒体/事件写入 |
| 17 中文/Unicode | 中文层级、café、emoji、240 字符 basename |
| 18 重启持久化 | 数据库重开后媒体、来源、任务与存储设置保留 |
| 19 来源暂不可访问 | 预览后临时改名来源，拒绝开始；恢复路径后可继续 |
| 20 相互包含 | 相同根、目标内来源、来源包含目标，以及 Settings 与已登记来源重叠均拒绝 |
| 21 既有数据库升级 | v11 全事实逐列比较、外键检查、v11 一致性备份和重复打开；另覆盖 v1–v11 迁移链 |

另有：8MiB 文件 quick-v1 相同而完整 Hash 不同；预览后源内容变化；DB 发布回滚后所有关联回滚、保留预留目标、重启恢复；来源 SQL 分页和旧 Quick Browse filter JSON 兼容；系统时间跳变时恢复旧任务。

## 实际发现并修复

1. Scanner 既有 UUID 含连字符，不能用 compact import job ID 格式验证。保留原身份，仅验证安全存储组件；旧媒体托管化后重新 Scanner 对账仍只有原实例。
2. 完成复制后 DB 发布失败，Scanner 原先会为该路径另建 ID。故障测试修复前稳定得到 new=1，修复后 new=0；新增目标预留索引，在原 Scanner 入队事务跳过未发布目标，恢复后使用原预留 ID。没有重写 Scanner。
3. 持久目标预留使数据库错误后的重试不覆盖、不重复复制；状态变更在事务中重新校验。重启恢复按任务 rowid 边界，避免依赖系统时钟。
4. 同一预览多个来源命中一个旧外部实例时，空间按目标 ID 去重。小文件大小使用 B/KiB/MiB，避免几 KiB 误显示 0 MiB。

## 桌面与数据安全证据

隔离目录 `.tools/validation/folder-import/`，真实 native picker 选择三份合成文件（两份同名 JPEG + 一份视频）。首次预览 3 文件/2 新媒体/1 重复/4 新标签；完成 2 新媒体、1 复用、4 标签关系、0 失败。重复导入 0 新媒体、3 复用、0 新标签关系、0 失败。来源 Animal 父级筛选包含 Cat 中的图片，只返回 1 项。Settings 原生选择新的 custom-media 根，旧媒体仍可查询。

退出后只读检查：2 media、3 来源路径、4 来源标签/关系、2 managed_media、2 completed jobs；Like/View/Play 各 0，foreign_key_check 空，全部托管副本与来源完整 Hash 相同，来源图片仍与原始合成 fixture 相同。窗口与 1420 监听已退出，没有使用私人媒体或正式用户数据库。

- [预览截图](folder-import-preview.png)
- [完成截图](folder-import-complete.png)
- [存储设置截图](folder-import-settings.png)

100,000 合成媒体、1,000 匹配项、1,000 来源节点实测分页；最终两种配置查询部分分别 41.64ms / 41.57ms（开发构建样本，不含数据构造，不代表 SLA）。Library 每页 ≤200，来源每页 ≤100；来源子树使用 parent 索引。既有十万 Grid 虚拟化、百万事件查询与其他全套回归一并运行，不创建 100,000 个真实视频。

## 检查记录

最终两种 Rust 配置各 122/122（全部 include-ignored，含真实工具），前端 57/57、开发监听 1/1；typecheck、两种严格 Clippy、fmt、生产构建和 diff-check 全通过。产物 `target/release/media-manager.exe`，Vite 主 JS 325.21kB / gzip 98.77kB。完整命令日志保存在忽略目录 `.tools/validation/folder-import/`；日志不提交生成目录。

```powershell
cargo test --locked --offline --no-default-features -- --include-ignored --nocapture
cargo test --locked --offline -- --include-ignored --nocapture
cargo clippy --locked --offline --no-default-features --all-targets -- -D warnings
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt --all -- --check
npm.cmd test
npm.cmd run test:dev
npm.cmd run typecheck
npm.cmd run desktop:build -- -- --locked --offline
git diff --check
```

## 限制

仅本机 Windows x64 验证；未重复干净系统安装/卸载、签名或 FFmpeg 分发验收。Settings 不搬迁既有媒体/数据库，旧根需保持可访问。完整 Hash 会读取全部内容；来源/目标同时被其他进程修改会逐项拒绝并要求重新预览。链接/reparse 不跟随，层级和文件名受文件系统限制。

UI 展示最近任务和前 20 个错误，暂无任务历史管理/自动清理策略。取消保留已完成媒体；DB 发布失败后已完成但尚未关联的文件保留预留，不删除，应用也不将其作为普通媒体呈现，使用 Resume / Retry 恢复。不将进程取消/故障注入等同于硬件断电耐久性认证。

本阶段不自动开始其他 Phase。
