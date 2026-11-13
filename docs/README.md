# 文档索引

## 当前规范与设计

五份主文档继续放在仓库根目录：

- [AGENTS.md](../AGENTS.md)：长期工程规则。
- [REQUIREMENTS.md](../REQUIREMENTS.md)：批准的产品需求与当前范围。
- [ARCHITECTURE.md](../ARCHITECTURE.md)：系统与数据库设计。
- [PLANS.md](../PLANS.md)：阶段状态与最新验收结果。
- [DECISIONS.md](../DECISIONS.md)：重要决策及变更原因。

## 审查与验证记录

- [Phase 11 robustness review](reviews/PHASE11_ROBUSTNESS_REVIEW.md)
- [Phase 11 稳定性与性能验收](validation/phase11-validation.md)
- [Phase 11 全仓 code review](reviews/PHASE11_CODE_REVIEW.md)

- [Phase 10 History / Continue Watching / Dashboard 验证](validation/phase10-validation.md)

- [Phase 1 审查](reviews/PHASE1_REVIEW.md)
- [Phase 2 审查](reviews/PHASE2_REVIEW.md)
- [2026-11-02 历次验证](validation/2026-11-02.md)
- [目录整理与复验（历史）](validation/organization-2026-11-02.md)
- [Cargo 网络诊断与沙箱内外对照](validation/cargo-network-2026-11-02.md)
- [11-02 完整重新测试（历史）](validation/retest-2026-11-02.md)
- [Phase 1/2 完整测试与已通过的桌面证据](validation/phase12-2026-11-04.md)
- [前次浏览器预览专项重试与连接故障分析](validation/browser-preview-2026-11-04.md)
- [最新 Phase 1 再次检查：自动检查通过、浏览器 URL 自动审批终止](validation/phase1-recheck-2026-11-04.md)

审查记录中的原始文件清单和命令路径保留为历史快照。当前路径：`docs/PHASE*_REVIEW.md` 已移到 `docs/reviews/`，`docs/VALIDATION_2026-11-02.md` 已移到 `docs/validation/2026-11-02.md`。最新状态以 PLANS.md 和最近一次验证记录为准。

## 项目目录

根目录是唯一正式项目；前端在 `src/`，Rust/Tauri 在 `src-tauri/`。Node 测试按 `tests/frontend/`、`tests/architecture/`、`tests/database/` 分类；Cargo 集成测试仍在 `src-tauri/tests/`，单元测试跟随 Rust 模块。默认脚手架归档见 [archive](../archive/README.md)。依赖、构建输出与临时验证文件分别放在忽略的 `node_modules/`、`target/`、`dist/`、`.tools/`，不混入源码。
