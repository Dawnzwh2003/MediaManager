# Rust 测试失败与播放状态迁移验证（2026-11-07）

## 范围与根因

保留进入本轮时所有未提交修改（含 v3 migration 与扫描代码），不推进扫描或其他阶段。本轮代码修改仅在 `src-tauri/src/db/migrations/tests.rs`。

上轮执行会话已不可恢复；检查现有 `.tools/validation` 日志未找到该次失败记录。因此本轮重新执行无桌面 lib 测试，实际复现 10 passed / 1 failed，再精确运行同一失败测试。

失败测试：`db::migrations::tests::changed_checksum_or_history_gap_is_rejected_without_new_ddl`。

失败位置：修复前 `src-tauri/src/db/migrations/tests.rs:123` 的 `.unwrap()`，错误 `SqliteFailure(ConstraintViolation, extended_code: 1555, UNIQUE constraint failed: schema_migrations.version)`。这不是缓存值比较断言失败：测试尚未执行到第二个 `assert!(matches!(migrate(...), Err(AppError::Migration(_))))`。

夹具删除 v2 后，执行无 WHERE 的 `UPDATE schema_migrations SET checksum=?1,version=2`；现在已有 v3，因此 v1 与 v3 同时被改为主键 2，导致夹具自身失败。修复为只恢复 v1 校验和，保留 v1/v3，删除中间 v2 来构造真实的历史缺口。原拒绝断言保留，并新增 ledger 版本及拒绝前后 schema 不变断言；未删除、跳过或降低测试要求。

## 播放状态与迁移验证

未改写 `0001`、`0002` 或现有 `0003` SQL，也未修改 migration runner、Trigger 或播放逻辑。新增 Rust 回归测试 `upgrade_preserves_playback_states_and_versioned_cache_without_play_events`，使用临时数据库和合成记录，分别在 recursive_triggers 开/关下验证：

- 含播放进度的 v2 数据库升级到当前 v3 后保留进度，升级前备份保持 schema=2 与相同播放状态；`playback_cache_version` SQL 不变。
- 缓存始终等于当前版本的 `playback_states.position_seconds`，没有当前版本状态则为 0。
- 新旧版本进度隔离，切回旧版本恢复原进度；非当前版本更新/删除不污染缓存。
- 当前版本状态 INSERT/UPDATE/DELETE 正确刷新缓存；事务回滚及无效版本的延迟 FK 提交失败恢复缓存与版本。
- 重开后状态一致，`PRAGMA foreign_key_check` 无错误；全过程 `play_events` 与 `play_count` 始终为 0。

目前没有证据表明 `playback_cache_version` 存在导致此次失败的缺陷，不因测试夹具错误修改已应用 migration。

## 实际验证

使用已有工作区 Cargo cache，均为 locked/offline；未修改依赖或锁文件。完整命令输出在 `.tools/validation/test-repair-2026-11-07/`。

- 精确失败用例，修复前：0 passed / 1 failed（failed-test-before.log）。
- 精确失败用例，修复后：1 passed / 0 failed（failed-test-after.log）。
- `cargo test --locked --offline --no-default-features`：25/25（lib 12、foundation 6、repositories 7），0 ignored。
- `cargo fmt --all -- --check`：通过。
- `npm.cmd test`：30/30，包含实际 v1/v2 SQLite/播放缓存测试。
- `cargo test --locked --offline`（默认 desktop）：25/25（lib 12、foundation 6、repositories 7），0 ignored；与无默认特性的 25 项为同一批测试的两种构建配置，不合计为 50 个独立用例。
- `cargo clippy --locked --offline --all-targets -- -D warnings`：失败，exit 101；报告两项进入本轮前已有的扫描代码问题：`src-tauri/src/db/repositories/scanning.rs:118` 的 `persist` 参数过多（8/7），以及 `src-tauri/src/scanner/mod.rs:116` 的 `drop(meta)` 触发 `drop_non_drop`。未添加 allow、降低警告等级或修改扫描实现；按本轮范围保留为未解决项。
- `git diff --check`：通过。

Windows 本轮实际运行 25 个 Rust 用例；Unix 专用用例未在此平台执行。已有扫描模块仍是上一轮未完成工作，本轮未运行扫描、未将其验收或推进其状态；PLANS.md 不改变阶段状态。
