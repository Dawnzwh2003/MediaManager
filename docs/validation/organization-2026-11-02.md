# 目录整理与 Phase 2 复验 — 2026-11-02

## 范围与文件位置

本轮仅检查、整理现有工程并执行验证。没有实现 Event Service、scanner、新业务 API 或 UI；没有修改 Rust 业务逻辑、数据库 Schema、已应用 migration 或依赖版本。

| 原位置 | 当前正式位置 |
| --- | --- |
| MediaManager/ | archive/scaffolds/media-manager-template/ |
| tests/contracts.test.ts | tests/frontend/contracts.test.ts |
| tests/boundaries.test.mjs | tests/architecture/boundaries.test.mjs |
| tests/schema.test.mjs | tests/database/schema.test.mjs |
| tests/database.test.mjs | tests/database/library.test.mjs |
| docs/PHASE1_REVIEW.md、docs/PHASE2_REVIEW.md | docs/reviews/ 下同名文件 |
| docs/VALIDATION_2026-11-02.md | docs/validation/2026-11-02.md |

默认模板完整保留 35 个文件，移动前后逐一比较 SHA-256 一致；未删除模板或覆盖正式项目。根目录保留唯一活动 package.json 与 Cargo workspace，五份主文档也保留在根目录。归档内的 manifest 仅作原始模板记录；Cargo 显式 exclude，Vite watch 排除 archive，TypeScript 和 Node 测试只包含正式目录。

配置调整：package.json 更新测试路径，tsconfig.json 纳入嵌套 TypeScript 测试，相关测试修正相对导入及 SQL 路径。新增 archive/README.md、docs/README.md，更新 README、ARCHITECTURE、PLANS 和 D025 的实际验证状态。REQUIREMENTS 与 AGENTS 的业务规则未变。

## 实际环境

Windows x64；Node 24.22.2、npm 11.19.1、rustc/cargo 1.99.1 stable。Tauri info 确认 MSVC Visual Studio Community 2026、WebView2 156.0.4314.58。npm 十个直接依赖已存在，包含 TypeScript 7.0.2 和 Tauri CLI 2.12.2；真实 package-lock.json 保留，未重新安装或伪造锁文件。Cargo.lock 尚未生成。

PowerShell 的 npm.ps1 被当前执行策略拒绝，因此实际使用 npm.cmd；没有修改系统执行策略。Cargo bin 仅补入当前进程 PATH；依赖下载使用工作区内 `.tools/cargo-validation`，CARGO_NET_RETRY=0、CARGO_HTTP_TIMEOUT=15、RUSTUP_AUTO_INSTALL=0，没有修改用户全局配置。

## Validation Summary

| 实际执行命令 | 结果 |
| --- | --- |
| npm.cmd ls --depth=0 | PASS，十个直接依赖均安装且版本匹配。 |
| npm.cmd test | PASS，30/30，无跳过。含真实 v1/v2 SQL、多对多、层级、唯一/FK/cascade、事件缓存原子性、替换隔离与 migration 回滚。 |
| npm.cmd run typecheck | PASS。 |
| npm.cmd run build | PASS，类型检查及 Vite production build；20 modules，产物在 dist/。 |
| cargo fmt --all -- --check | PASS。 |
| cargo metadata --no-deps --format-version 1 | PASS，仅正式 src-tauri 包，归档未成为 workspace 成员。 |
| cargo check | BLOCKED，exit 101，依赖下载失败，未进入编译。 |
| cargo test | BLOCKED，exit 101，依赖下载失败，完整测试未执行。 |
| cargo test --no-default-features | BLOCKED，exit 101，同上。 |
| cargo clippy --all-targets -- -D warnings | BLOCKED，exit 101，同上。 |
| rustc --edition=2021 --test src-tauri/src/platform/observation.rs -o .tools/validation/observation-tests.exe | PASS，随后实际执行该产物，4/4 测试成功。仅验证纯标准库模块，不替代 cargo test。 |
| npm.cmd run tauri -- info | PASS，工具与正式 React/Vite 配置识别成功。 |
| npm.cmd run desktop:build | BLOCKED，exit 1；beforeBuildCommand 的前端类型检查及构建成功，随后 Cargo 下载失败。 |
| npm.cmd run desktop:dev -- --no-watch --no-dev-server-wait --config .tools/validation/tauri-dev-check.json | BLOCKED，exit 101；已进入 cargo run，随后依赖下载失败。临时配置仅将 beforeDevCommand 置空，避免遗留开发服务器；没有更改正式 Tauri 配置或启动桌面窗口。 |
| git diff --check | PASS，文档更新后实际执行成功。 |
| Node 文件布局/摘要/Markdown 链接/锁文件校验 | PASS：35/35 归档文件摘要不变；12 份文档的 23 个本地链接有效；package-lock 直接依赖与 manifest 一致；原 MediaManager/ 位置已移走。 |

## 环境阻塞

Cargo check、两种 test、Clippy 以及 Tauri dev/build 都出现同一错误，首个无法取得的依赖为 fs2：

```text
error: failed to get `fs2` as a dependency of package `media-manager v0.1.0`
Caused by: download of config.json failed
[7] Could not connect to server
Failed to connect to index.crates.io:443
```

当前阻塞是 crates.io 网络访问，不是已确认缺少 MSVC/WebView2，也不是编译器 pin 缺失。没有为此修改应用代码或去掉依赖。待该网络可用后仍需执行完整原生编译、测试、Clippy 和桌面 IPC/重启/锁定 smoke；不能把格式、Node SQLite 或独立 Rust 模块通过冒充完整原生通过。

## 整理后代码审查

- 正式命令注册仍只有 get_app_status；services 仅 app_status，EventRepository 只有 counts/likes/views/plays 读取方法，没有事件写入或资格服务。
- React IPC 仍集中在 src/api/，前端没有 SQLite、文件系统或 shell 插件；现有架构测试实际通过。
- 媒体身份、非唯一 fingerprint、两类 Tag 边界、事件 Source of Truth 与数据库事务触发器均未改变；实际 SQL 回归测试通过。
- 测试移动后正常发现全部 30 项，类型检查包含嵌套测试；正式 workspace 只发现一个包。归档不参与正式构建，35 个原始文件完整保留。
- 未发现本次整理引入的代码问题；原生编译尚未完成，审查不替代编译和 Rust Repository 测试。

**Phase status: FAIL（环境阻塞，未验收）。Ready for next phase: NO。** 本轮到此停止，不进入后续 Phase。
