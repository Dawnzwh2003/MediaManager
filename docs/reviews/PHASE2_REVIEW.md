# Phase 2 数据模型、数据库测试与审查

最新验证见 [2026-11-04 Phase 1/2 测试](../validation/phase12-2026-11-04.md)：当前授权的数据层在 Windows 上验收通过，7 项 Repository 测试及全部 SQL/完整 Rust 测试通过，真实桌面确认 schema=2。Phase 1 浏览器专项仍单独待补。下文保留原审查及后续历史记录。

## 范围与状态

按用户最新指令，本阶段只实现持久化模型、migration 和 Repository。scanner、Event Service、事件资格判断、随机顺序生成、媒体激活、播放器及 UI 业务均未实现；前端和 Tauri command 注册表没有新增业务入口，仍只有 get_app_status。

**数据层已编写，SQL 测试通过；Rust 原生验收仍受环境阻塞。** 不自动推进任何扫描或后续阶段。Phase 1 的未完成原生验证没有被本轮 SQL 结果抹去。

## Schema 与 Repository

- `0002_library_model.sql` 为新 migration，原 `0001` 未修改。运行时 schema=2，ledger 的版本/name/checksum/时间以及升级备份继续由现有 runner 管理。
- 包含用户指定的 media、system_tags、custom_tags、media_system_tags、media_custom_tags、like_events、view_events、play_events、quick_browse_sessions、app_settings。
- 辅助表：db_sequence、media_revisions、media_locations、playback_states、quick_browse_session_items；v2 共新增 15 张表。
- media 的全部指定字段均存在；额外的状态/版本/sequence 不把 path/hash 变成 identity。首次插入建立版本与位置记录，location 更新保留 ID/imported_at/标签/事件。fingerprint 非唯一。
- System Tag 为物理目录观察，树结构支持多层、同根 FK、防自引用/环；Rust Repository 检查绝对根及直接父路径，但不假装已扫描磁盘。无用户手工建目录入口。
- Custom Tag 为平面、多对多，trim 后非空、SQLite NOCASE 名称唯一，可改名/删除；无 parent_id，不以逗号字符串或 JSON 字段代替关系表。
- 六类 Repository：Media、CustomTag、SystemTag、只读 Event、QuickBrowse snapshot storage、Settings。SQL 参数化、分页 1–200；媒体分页使用 (imported_at,id) keyset。
- 标签删除级联关联；System 父目录有子节点时 RESTRICT；事件/快照引用媒体时 RESTRICT。未被引用的媒体数据库行可级联其所属数据，但 Repository 不提供硬删除，更不操作媒体文件。
- 事件 INSERT 与计数由同一 SQLite 触发器事务维护；未来服务不能重复 +1。Like/View 不可改删，Play 仅一次终结补全。没有事件写 Repository/资格服务，测试插入只是数据库 fixture。
- playback_states 与 Play 独立，media.playback_position 为当前版本投影；QuickBrowse 仅存储调用者给定的 seed/序号，ready 快照不可变。

## 本轮审查修复

1. v2 引入后，原 Rust migration 测试的“未来版本 2”和 schema=1 断言已过期：改为测试实际 v1→v2、未来 v3 拒绝和 CURRENT_SCHEMA_VERSION；修正伪造 gap fixture 的主键冲突。
2. SQLite 默认关闭 recursive_triggers 时，INSERT OR REPLACE 可能隐式删旧日志而绕过 DELETE trigger：增加 BEFORE INSERT guards，保护既有媒体/活动路径及事件 id/request/activation/player session，避免历史重写与双计数；已有执行通过的回归测试。
3. playback_states 的媒体/版本键禁止 UPDATE 改写，避免把一个媒体进度挪给另一媒体并留下旧缓存；切版本/删状态有缓存同步测试。
4. 明确 Superseded 时退役位置，System Tag 的直接物理父路径由可信观察 Repository 校验；只读事件仓储不承担新事实判定。
5. 快照发布验证连续 ordinal 和精确数量；标签/快照批量操作事务失败保留原数据。约束错误返回稳定 constraint_violation，不泄露 SQL/路径。

## 实际测试结果

| 命令 / 检查 | 结果 |
| --- | --- |
| `npm test` | **30/30 通过**；24 项新数据库测试 + 原 6 项契约/边界/SQL。 |
| `PRAGMA foreign_key_check` | 新数据库用例每项结尾均校验，无异常。 |
| `git diff --check` | 通过。 |
| `cargo check` | cargo 不存在，未进入编译。 |
| `cargo test` | cargo 不存在；22 项 Rust 测试均未执行，不能计为通过。 |
| `cargo test --no-default-features` | 同样被工具链缺失阻塞。 |
| `cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` | cargo 不存在，未完成。 |
| `npm run typecheck` / `npm run build` | 依赖尚未安装，tsc 不可用；未运行到编译。 |
| `npm run tauri -- info` / `npm run desktop:build` | Tauri CLI 不可用，未完成。 |

Node 测试执行的是本仓库实际 migration SQL，并使用 Node 自带 SQLite；没有用 mock 数据库替代约束。它们不能证明 Rust Repository 映射/借用/编译、bundled SQLite 或原生 migration runner 已通过。原生工具与依赖获取限制沿用 Phase 1，仍需在具备环境后执行并修复。

### 新数据库测试覆盖

- 指定字段、未知元数据、默认零值；重复 fingerprint 独立实例；活动路径/ID 唯一。
- 一个媒体多 Custom Tag、一个 Tag 多媒体；平面名称唯一、改名保关联、删除不删媒体。
- 缺失 FK、原子替换关联回滚；128 层目录、防环/跨根/孤儿；父节点 RESTRICT、叶节点关联 CASCADE。
- 无历史媒体所属数据级联，有历史媒体禁止删除；路径变化/同路径替换不错误继承。
- Like 事务回滚、累计五次、请求唯一、不可改删；计数溢出失败回滚事件。
- activation 唯一与 last_viewed_at；Play 版本 FK/唯一 session/终结不二次计数。
- 播放状态不生成 Play、旧版本隔离、状态键不可挪用；SQLite 递归触发器开/关均正确。
- 稳定 seed、快照序号/成员唯一、ready 不可变、发布拒绝 gap、删除会话 CASCADE；结构化设置 JSON。
- 真实 v1→v2 SQL 保留应用元数据；失败迁移回滚所有新 Schema；REPLACE 不能绕过事实保护。

## 本轮文件变更（相对 Phase 2 开始时）

新增：

```text
src-tauri/migrations/0002_library_model.sql
src-tauri/src/domain/library.rs
src-tauri/src/db/repositories/mod.rs
src-tauri/src/db/repositories/media.rs
src-tauri/src/db/repositories/tags.rs
src-tauri/src/db/repositories/events.rs
src-tauri/src/db/repositories/sessions.rs
src-tauri/src/db/repositories/settings.rs
src-tauri/tests/repositories.rs
tests/database.test.mjs
docs/PHASE2_REVIEW.md
```

修改：五份主设计文档、README.md、package.json、src-tauri/Cargo.toml、db/mod.rs、db/migrations.rs、db/migrations/tests.rs、domain/mod.rs、error.rs、tests/foundation.rs。

前端 src/ 与 get_app_status command 未修改；未增业务 IPC 或 service。完整依赖锁文件仍未生成，不把单独固定直接版本误称为全部依赖已锁定。

## 剩余风险与停止点

Rust 22 项测试（20 项跨平台、2 项 Unix 专用）须在补齐工具链后运行；格式/lint/类型/原生构建均须验收。路径规范化与真实目录来源由未来 scanner 提供；当前数据层不验证磁盘存在。后续 activation/player session 工作表及 rebuild delta hook 需正式 migration 后才可启用 Event Service，不允许新服务重复增加计数。

本轮到数据库层停止，不执行 Phase 2b scanner 或其他阶段。

## 2026-11-02 再次检查补充

再次确认没有 Event Service，事件 Repository 仍只读，注册命令仍只有 get_app_status。发现并修复目录观察词法路径可含 .. 的缺口：新增 platform/observation.rs，拒绝观察根与目录中的 ParentDir，SystemTagRepository 使用该校验，并补 Repository 回归断言。此修复不扫描文件系统、不修改 SQL 或已应用 migration，也没有修改依赖/工具链版本绕过环境。

npm test 再次 30/30 通过；stable rustfmt 检查通过。实际使用 rustc +stable 直接编译新增的纯标准库测试模块，并执行 .tools/observation-tests.exe，4/4 通过，证明该模块与本机原生链接可工作。当前共有 26 项 Cargo 测试，但上述结果只覆盖新模块的 4 项，不能记为完整 cargo test 已通过。

固定 1.90.0 工具链仍缺失；npm install 再次 EACCES、crates.io 仍连接失败，导致完整 Cargo、前端和 Tauri 检查仍受阻。验证详情见 [第二轮检查记录](../validation/2026-11-02.md)。原有 Phase 2 表格保留为历史记录；整体状态仍为未验收，不进入后续阶段。

## 用户授权后的工具链更新

按用户最新指示已从 1.90.0 pin 切到 stable（本轮 1.99.1），D025 记录策略与原因；普通 cargo fmt 已通过，旧 pin 不再是当前 blocker。当时尝试安装 TypeScript/Tauri CLI 仍返回 EACCES，Cargo 检查/测试/Clippy 仍因 crates.io 连接失败未完成。npm 测试 30/30、独立 Rust 模块测试 4/4 成功，不能替代全库编译。该轮结果见 [验证记录末节](../validation/2026-11-02.md)。无新业务服务或数据库变更。

## 目录整理后的复验

默认模板已归档，Node 测试按 frontend/architecture/database 分类，审查与验证记录分别放入 docs/reviews/ 和 docs/validation/。所有导入、测试脚本和文档链接随之更新；原始文件清单保留历史路径。本轮未修改 Rust 业务逻辑、数据库 Schema 或 migration，也没有实现 Event Service。

现有 npm 十个直接依赖齐全；整理后 npm 测试 30/30、typecheck、生产构建、cargo fmt、Tauri info 和独立路径测试 4/4 实际通过。完整 Cargo 与 Tauri 原生构建仍在 crates.io 下载 config.json 时失败。详细命令、代码审查和停止点见 [最新复验记录](../validation/organization-2026-11-02.md)。
