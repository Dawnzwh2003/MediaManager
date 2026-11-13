# Release Readiness — 2026-11-13

结论：**Ready for release: NO**。本次以第一次面向普通用户的 Windows 正式 release 为标准。工作区起始干净，基线为 Phase 11；不新增产品功能，不改 Schema、依赖或锁文件。仅增加发布边界测试、修正一个文件系统相关的测试假设，以及更新文档。

## Critical issues

1. **新用户无法导入媒体。** `src-tauri/src/lib.rs`、`build.rs`、commands、capabilities 和 `src/api` 没有选择目录/注册扫描根/启动扫描的产品入口；AppState 只启动派生资源、统计及 Quick Browse worker。生产空库界面只提示扫描后显示，没有实际操作。普通刷新只查库，重启也不自动扫描。Rust scanner 和测试 seed 不能代替用户导入流程。本轮遵守“不增加新功能”，未补 UI/IPC 或自动扫描。
2. **正式分发验收未闭合。** EXE、NSIS 均已成功构建，但未在干净 Windows 环境实际安装、升级、卸载并验证 WebView2 bootstrap；本机已有开发工具和 WebView2，不能替代该项。产物未签名。FFmpeg 外置工具的正式供应/分发与许可材料还未确认，仓库没有本轮可据以批准的完整分发清单。这些结论不意味着安装包构建失败。

## Known limitations

- 当前可验证范围为本机 Windows x64；最低系统版本、最低 Rust 版本、其他平台、长期运行与真实 100k 视频解码不在本轮通过范围。
- FFmpeg/ffprobe 不随 EXE/NSIS 分发。缺失时查询、状态和可直接播放的资源仍可用，处理/转换逐项失败；安装工具后，失败缩略图需要后端显式 retry，现有 UI 无独立 retry 入口。
- Windows 覆盖移动可能继承已删除目标的创建时间。创建时间属于稳定身份凭据的一部分；当凭据改变时采用旧 missing + 新 ID，旧历史/标签/进度仍在旧实例，当前新实例不继承。这种情况没有手工 relink；不能承诺所有移动都保留同一个当前实例。
- 缓存 JPEG 使用有界结构校验，不能等同完整熵数据解码校验；结构完整而熵数据损坏的 JPEG 可能留在 ready，前端显示读取失败占位。空文件、随机字节、截断/伪 JPEG 的重新排队与再生成已实测。
- SQLite NOCASE 仅提供 ASCII 折叠，中文按字面匹配，不提供完整 Unicode 大小写或规范化搜索。
- History 每页仍重新聚合 Event Log；首次 Random 排名也有全候选落盘成本。Phase 11 Debug 的百万 View 测量约 0.5s/History 页、1s/Dashboard，重建期间 Like 最大约 5.57s。这里引用历史性能证据，本轮没有把这些数字冒充 Release 性能 SLA。
- SQLite schema ledger 校验、FK 约束、事务和 Counter Cache 修复不等于每次启动做完整 integrity_check。损坏页面、已丢失事件、外部工具造成的外键错误不能用计数重建恢复，应保留文件并从可信备份恢复。
- 没有自动定期备份或自动降级；升级备份不包含媒体原件。Library Random seed 在 WebView 本地存储，完整偏好备份需要考虑该 profile。

## 检查矩阵

| 项目 | 本轮证据 / 结论 |
| --- | --- |
| Build | 正式配置 TypeScript/Vite + Tauri optimized Release 成功；独立配置验证生产窗口；临时 bundle 配置构建 NSIS 成功 |
| Tests | Node 55/55、监听 1/1、Rust desktop/no-default 各 105/105；所有真实 FFmpeg 用例执行，零忽略；两种 Clippy、fmt 通过 |
| Database migrations | 正式 runner 测试 v1–v10→v11、备份保留/恢复后再迁移；既有失败 DDL/ledger 回滚、gap/checksum/较新版本拒绝回归 |
| Fresh install | 生产 EXE 在独立空应用目录启动成功、实际 tauri.localhost 内嵌前端；干净操作系统安装仍未验证 |
| Existing database upgrade | 生产窗口将合成 v10 库升级为 v11，112 个媒体 ID/导入时间/路径零差异；自动备份仍为 v10，sentinel/事件/进度保留 |
| Empty data folder | 新目录/已存在空 data/ 自动初始化、无重复备份、不删无关 sentinel；生产 Dashboard/History/CW 为零，空 Quick Browse 可退出 |
| Large media library | 自动测试完整分页 100k 常规/Random、100k History 与固定快照；另启动包含 100k 媒体/百万 View 的生产库，首屏/滚动有界 |
| Corrupted media | 真实坏视频/图片、无效 metadata、输出限制、超时/取消逐项隔离，原件指纹不变 |
| Missing media | 现有 missing/恢复、离线源读取失败、坏扫描不整体 missing 的实际回归全部执行 |
| Moved / renamed media | 普通移动/重命名和中文路径保持已确认关联；覆盖移动同时检查凭据不变/变化的批准分支，见下文 |
| Unicode / 中文 / very long filenames | 实际中文、空格、组合字符/emoji/特殊字符路径；244 UTF-16 单位文件名、304 单位完整路径扫描/资源读取成功；293 单位中文长路径经真实 FFmpeg 生成缩略图 |
| Unsupported codecs | 修改 AVI FourCC 的真实夹具返回 unsupported_codec，其他媒体继续；真实 Remux/Proxy 正常与失败测试执行 |
| FFmpeg unavailable | 缺工具的 metadata/thumbnail/转换失败持久化且可后端 retry；生产空库用显式不存在的工具目录仍正常进入 UI |
| Thumbnail cache corruption | 空文件、随机字节、伪 JPEG 请求时变 pending，再生成 ready；缓存权限故障、丢失及版本切换原回归执行 |
| Database inconsistency | 坏库不清空、迁移历史拒绝、事务/COMMIT 回滚、FK 边界、计数/最近时间损坏后的重建回归；退出后独立 SQL 再核对 |
| Counter rebuild | 100k 媒体分批发布、并发 Like、失败/取消/恢复全套执行；日志和 Cache 事务一致，未由前端统计 |
| Quick Browse | 全筛选/排序/seed 冻结、100,001 成员、取消/配额/超时/重启恢复测试；生产 112 项会话实际 1→2，原件正常显示，Esc 退出 |
| History | 五类/四时间范围、500 页精确遍历、missing 保留、稳定游标、Dashboard 聚合自动回归；生产空库实际访问正常 |
| Continue Watching | current-version/completed 排除和进度测试；生产升级库显示 7.5/30s、25%，打开后真实解码到烧录的 00:00:07.500 |
| Search / Filter / Sort | 字面查询、参数注入字符串、中文名、AND/OR、层级目录和全部常规排序的真实 SQLite 回归；前端过期响应/边界测试 |
| Random seed | seed/算法版本、分页/过滤/重连后幸存顺序及前端 localStorage 重载测试；普通刷新不生成新 seed |
| App restart | 生产空库实际关闭重开，既有空 Quick Browse 会话仍保留；旧 owner、持久 checkpoint 和实际 exit(73) 崩溃恢复全套执行 |
| Playback restore | 后端真实重开恢复、owner/CAS/sequence 隔离；生产升级后实解码并保持 7.5 秒，关闭后独立 SQL 确认 |

## Tests

环境：Windows x64/MSVC，Node 24.22.2、Rust stable 1.99.1、FFmpeg/ffprobe 9.0.2；Cargo 使用既有缓存、真实 lock 与 `--offline`。日志位于忽略的 `.tools/validation/release-review-2026-11-13/`。

```powershell
$env:CARGO_HOME='E:/MediaManager/.tools/cargo-validation'
$env:RUSTUP_AUTO_INSTALL='0'
$env:MEDIA_MANAGER_FFMPEG_DIR='E:/MediaManager/.tools/ffmpeg/ffmpeg-9.0.2-essentials_build/bin'
npm.cmd test
npm.cmd run test:dev
cargo test --locked --offline -- --include-ignored --nocapture
cargo test --locked --offline --no-default-features -- --include-ignored --nocapture
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --all-targets -- -D warnings
cargo fmt --all -- --check
```

两种 Rust 各 105：33 unit + 12 activity + 7 derivatives + 6 foundation + 8 library + 8 quick_browse + 3 release_review + 6 reports + 7 repositories + 7 robustness + 8 viewer。完整编译和测试通过不是首次运行全部通过：

- 新增空库测试最初使用 i64::MAX 作为报表 end，触发已有 JS safe integer 校验；改为合法窗口，保留初始失败日志，未改产品校验。
- 首轮完整测试复现原覆盖移动测试失败。将两个文件创建间隔分开 75ms 后稳定复现：源 File ID 不变，移入后 created 变为原目标 created，因此完整 file_key 改变。原测试未经核对就假设高置信度，造成依赖时间分辨率的偶发失败。
- 修正真实文件测试，明确检查高置信度保持 / 低置信度 missing+new 两个结果，强制保留两份 Like 历史和源进度/标签；另增仓储测试直接验证已确认移动先退役占用目标，再保留源 ID 发布，仍覆盖 Phase 11 UNIQUE 修复。**没有删除失败断言、忽略测试或放宽产品身份规则。** 修正后重新跑两种完整套件各 105/105。

新增用例全部使用临时数据库与合成文件。生产 smoke 使用隔离 identifier/应用目录，访问复制的合成数据库；源文件仅来自既有 `.tools/validation/phase10/Synthetic Collection`。v10 smoke 夹具在全新目录执行真实 v1–v10 SQL、匹配实际 name/checksum ledger，之后由正式生产 runner 升级；未修改正式用户数据库或篡改已应用迁移。

## Build

正式命令：`npm.cmd run desktop:build -- -- --locked --offline`。脚本实际先执行 `npm run build`（tsc --noEmit + Vite）再编译 optimized Tauri EXE。没有运行 dev server 来代替 Release；生产窗口的可访问性 URL 为 `http://tauri.localhost/`。

NSIS 验证：临时 JSON 仅开启 bundle.active/targets，不改仓库默认配置，执行 `npm.cmd run tauri -- build --ci --bundles nsis --config .tools/validation/release-review-2026-11-13/bundle-check.json -- --locked --offline`。经批准下载 NSIS/相关工具及校验 hash，makensis 成功。Cargo offline 不覆盖打包器工具下载。

| 产物 | 结果 |
| --- | --- |
| `target/release/media-manager.exe` | 正式配置构建成功，最终已恢复正式 identifier/默认数据目录配置；未用它访问用户正式目录 |
| `target/release/bundle/nsis/Media Manager_0.1.0_x64-setup.exe` | 3,254,120 B；构建成功，未在当前用户系统安装或更改卸载注册信息 |
| 正式最终 EXE SHA-256 | `5FD7585BBFA177CB4ADEA4F55F5D0974C0F2100CCB353EB8A49F84CA282BC78D` |
| NSIS SHA-256 | `B28179379CFB09ADC23E57E9BFE97913D64D16543379CD13A19F1907513401EF` |
| 签名 | Windows Authenticode 检查为 NotSigned |

独立 fresh/upgrade/large 生产构建仅覆盖 identifier、应用数据目录和窗口参数；前端仍为产品 dist，没有打入测试 harness。测量/安装包副本与 hash 保留在 `.tools`，没有发布到远程或建立新的产品入口。

## Data safety

| 问题 | 审查结论 |
| --- | --- |
| 用户数据丢失风险 | 本轮场景未发现原件或已提交历史被删除/清空。迁移前备份、故障事务回滚、拒绝坏/较新/未知库均通过。断电前未提交进度不能保证，备份/恢复和不可靠身份匹配限制如上 |
| 是否有实际删除媒体文件的代码 | 产品没有删除用户原件的服务/IPC。确有 `fs::remove_file`：Thumbnail 只替换其 hash JPEG/part，Playback 只清理命名受限的应用缓存和 part，Quick Browse 只清理 hash spool。测试中会删除自建合成源来模拟 missing/覆盖，不是产品原件删除入口 |
| Path traversal | 原件协议仅解析 opaque token + direct/remux/proxy，不把 URL 当文件路径；非法 ../ 和编码 traversal 用例拒绝。扫描拒绝链接根/祖先，缓存使用 hash 名和祖先/reparse 检查；观察目录拒绝 ParentDir。未发现可由 React 参数逃逸到任意路径的入口 |
| React 任意文件系统操作 | 没有 fs/shell 插件或任意路径命令；只有 local main capability。convertFileSrc 的 asset scope 限应用 JPEG 缩略图，原件通过受限只读 media 协议 |
| React 任意 SQL | 没有 SQL 插件或执行 SQL 的 IPC，前端权限测试通过；仓储绑定参数，动态表/排序来自封闭枚举。包含引号与 SQL 片段的搜索作为字面值处理 |
| 外部进程 | 生产只使用 Rust Command 参数调用受配置的 FFmpeg/ffprobe，不经 shell 执行文件名；全局进程预算、超时/输出/像素限制和回收测试通过 |

退出后的独立只读核对：升级库 schema=11，唯一备份 schema=10；112 媒体、5 Like、119 View、4 Play；媒体 ID/路径/导入时间差异 0，计数及最近时间相对日志差异 0，foreign_key_check 无错误、quick_check=ok。112 个合成原件重算 quick-v1 指纹全部一致；恢复位置 7.5/30 秒。新增 View 来自本轮实际续播（source=history）及 Quick Browse，未凭进度增加 Play。

生产 100k 库实际首屏每页 100 项，可访问性树中媒体选择项初始 16 个、一次原生滚动后 20 个；窗口保持响应，代码路径仍为有限页 + 虚拟行。该观察不宣称所有窗口尺寸都有相同卡片数，也没有把可访问性计数冒充新的精确内存或延迟 benchmark。10k/100k 既有规模库再次独立核对 Cache/最近时间零差异、FK 零错误、quick_check=ok。

清理记录：升级实例正常退出。fresh/large 最小化后，窗口工具先后报告 user input detected 与 window is minimized，无法完成恢复/关闭；停止 UI 重试后，按实际 EXE 完整路径核实两个测试 PID，经批准结束这两个隔离进程。未处理正式应用或其他 Node/WebView 实例。随后原生验证窗口清单为空，1420 无监听，截图 Node 内核已重置；升级/空库检查再次通过，空库保留其已建立的空 Quick Browse 会话。未安装 NSIS 或更改正式 app data。

最终三处 instance.lock 均可独占打开，确认测试持有者已释放。结束后的大库仍有 100,000 媒体/1,000,000 View，quick_check=ok、FK 零错误；48 个相关本地文档链接及 git diff --check 通过。正式最终 EXE hash 与记录一致。

截图：[生产首次空库](../validation/release-review-fresh.png)、[生产实际续播到 7.5 秒](../validation/release-review-playback.png)。

## Ready for release

**NO**。构建和当前测试通过，首次用户导入流程与正式分发验收仍缺失。本轮不通过新增功能、修改身份语义、临时改正式库或跳过失败测试来改变结论。README 已重写 Installation / Development / Production Build / FFmpeg / data、cache、database location / backup / troubleshooting。
