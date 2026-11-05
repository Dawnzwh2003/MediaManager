# Cargo 网络诊断（2026-11-02）

## 结论

本轮对照实验确认：当前 `index.crates.io:443` 连接失败由执行沙箱的网络限制导致。同一 HTTPS 请求、同一 Cargo 下载命令在沙箱内失败，在经批准的沙箱外执行中成功。此次诊断没有改变代理、镜像、TLS 设置、正式项目依赖或 Schema。

先前目录整理记录中的连接失败确实发生，但不能据此认定本机网络或 crates.io 服务不可用。本次没有重新运行正式项目的完整编译或验收，Phase 状态仍为未验收。

## 实际检查与结果

| 检查 | 结果 |
| --- | --- |
| `Resolve-DnsName index.crates.io` | 成功解析 Fastly CNAME、IPv4 与 IPv6 地址。 |
| 代理与 Cargo 配置检查 | 当前进程未设置 HTTP_PROXY、HTTPS_PROXY、ALL_PROXY、CARGO_HTTP_PROXY 等所查网络覆盖变量；默认用户 Cargo 目录、项目及盘根所查 config/config.toml 未发现代理或镜像配置。工作区验证 Cargo 缓存也无 config/config.toml。 |
| 沙箱内 `curl.exe -I --connect-timeout 10 --max-time 15 https://index.crates.io/config.json` | exit 7，连接失败。 |
| 沙箱内相同参数访问 `https://registry.npmjs.org/` | exit 7，连接失败。 |
| 经批准在沙箱外执行上述两个 curl 命令 | 两者均 HTTP 200，命令 exit 0。 |
| 沙箱内隔离 Cargo 探针 | exit 1，更新 index 时下载 config.json 失败，连接在 6 ms 后失败。 |
| 经批准在沙箱外执行相同 Cargo 探针 | exit 0，成功解析并下载 fs2 0.4.3、libc 0.2.191、winapi 0.3.9 及两个 winapi 平台包。 |

Cargo/rustc 实际版本均为 1.99.1。探针位于忽略目录 `.tools/diagnostics/cargo-network/`，使用独立 `[workspace]`、固定 `fs2 = "=0.4.3"`，避免解析或改变正式项目依赖。探针命令如下，沙箱内外保持相同：

```powershell
$env:CARGO_HOME = 'E:\MediaManager\.tools\cargo-validation'
$env:CARGO_NET_RETRY = '0'
$env:CARGO_HTTP_TIMEOUT = '15'
$env:RUSTUP_AUTO_INSTALL = '0'
cargo fetch --manifest-path .tools/diagnostics/cargo-network/Cargo.toml
```

环境变量只作用于执行该命令的进程。下载缓存和探针生成的真实锁文件均在 `.tools/` 下；正式项目根目录和 `src-tauri/` 仍无 Cargo.lock。

## 后续执行方式与验收边界

后续需要联网的正式 Cargo 命令应通过经批准的沙箱外执行获取依赖。无需为此次连接问题更换镜像、关闭证书验证或修改业务代码。

本轮仅证明 Cargo 能在沙箱外访问 registry 并下载最初报错的 fs2 及其依赖；不代表正式项目的全部依赖解析、Cargo check/test/Clippy、Tauri dev/build 或桌面 smoke 已通过。完整原生验证及阶段验收仍待执行，不进入后续 Phase。
