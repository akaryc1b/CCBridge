# 构建、测试与发布

开发约束见 [CONTRIBUTING.md](../CONTRIBUTING.md)，UI 修改遵循 [UI_GUIDELINES.md](UI_GUIDELINES.md)。

## 本地构建与测试

需要 Rust 1.93.0、Node.js 22、pnpm 10、Git、Python 3；Mac 需 Xcode 命令行工具与 macOS SDK。

```sh
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui test
pnpm --dir ui build
cargo build --locked --release
CCBRIDGE_TEST_BIN="$PWD/target/release/ccbridge" cargo test --locked
cargo fmt --all -- --check
python3 tests/security-audit-test.py
python3 scripts/security-audit.py --output security-audit.json
pnpm --dir ui audit --prod --audit-level=low
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

默认 Cargo GUI 保留旧 egui 开发回退；正式 App 使用 `src-tauri` 宿主，与 CLI / MCP / Runner 共用 Rust 库。正式打包前将 Linux Runner 放入 `dist/ccbridge-linux-x86_64`，将已通过 Windows 原生验收的 Runner 放入 `dist/ccbridge-windows-x86_64.exe`，设置独立的更新签名密钥后运行 `scripts/package-tauri.sh`。签名密钥不进入源码。

服务器只构建 headless Runner：

```sh
cargo build --locked --release --no-default-features
cargo test --locked --no-default-features
sh tests/linux-smoke.sh "$PWD/target/release/ccbridge"
```

默认测试使用临时 SSH/Git/Claude fixture，不需要真实账号、模型或数据库。可选数据库集成脚本需要 Docker、Node、universal-db-mcp 2.14.0，只用于隔离测试；不在默认 CI 中连接外部数据库。

## GitHub 应用内更新与 CI / CD

首次从旧版迁移到 Tauri 需要替换一次 App；后续在“应用更新”检查、下载、验证签名后安装并重启。账号数据仍在 `~/Library/Application Support/CCBridge`，MCP 路径仍是 `/Applications/CCBridge.app/Contents/MacOS/ccbridge`。

安装前检查本机操作和 MCP 等待请求；这些活动未结束时显示具体操作及下一步，不能中断后强装。SSH 已连接时额外检查远端未结束任务，并列出任务标题与状态；查询失败则显示原因。未连接或连接已关闭时可离线安装，无须为更新登录 SSH：本机 App 重启不会停止脱离 SSH 会话运行的远端 Runner 任务，重连后可继续查询。旧版 MCP 等待窗口尚未结束时显示剩余等待秒数，不要求重启 Codex。

推送和 PR 执行 Rust 格式、Linux Runner 测试/烟测、Mac SSH/MCP 集成测试、前端测试与 Tauri 编译。标签 `vX.Y.Z` 与版本一致，Linux/数据库公共检查及对应架构检查通过后，该架构立即发布，不等待另一架构。ARM 与 Intel 分别构建、独立发布到同一个 Release；后完成的架构追加文件，并合并 `latest.json`，不会覆盖先完成的架构。尚未完成的架构不会被冒充为可更新；可以先下载已完成架构。发布包含：

- `CCBridge-macos-arm64.dmg` / `CCBridge-macos-x86_64.dmg`：按芯片首次安装或应急恢复；应用内更新自动按架构选择更新包
- `CCBridge.app.tar.gz`（ARM）、`CCBridge-macos-x86_64.app.tar.gz`（Intel）与各自 `.sig`：签名更新包
- `latest.json`：GitHub 更新清单
- `ccbridge-linux-x86_64`：配套 Runner

构建缓存分为 Linux、ARM、Intel 三组，固定 Rust 1.93.0；缓存 Cargo 下载及依赖编译产物（包含根目录和 Tauri 的 debug/release），不缓存签名密钥或最终安装包。主分支会完成 Tauri release 编译预热并保存缓存，后续标签可恢复主分支缓存；锁文件变化时尽量复用未变化依赖。pnpm 使用锁文件对应的下载缓存，Node 22 由 setup-node 提供，不再每轮通过 Homebrew 安装。首次运行、工具链或依赖发生变化仍需要编译，缓存命中不跳过测试、签名或发布验证。缓存是否精确命中可在各构建任务的 Summary 查看。

使用 Tauri 独立更新签名与 ad-hoc App 签名，**不需要 Developer ID 或 Apple 公证**。GitHub Secrets 中保存 `TAURI_SIGNING_PRIVATE_KEY` 与可选密码。公钥随 App 发布；下载、网络或签名验证失败不替换当前 App。新 App 完全无法启动时，从上一版本 ZIP 应急恢复，而不是依赖失效 App 自己回滚。

CI 不保存 SSH/数据库密码，不连接开发或生产服务器，不自动部署用户 Runner。GUI 登录与真实服务器验收在授权环境完成。维护说明见 [CONTRIBUTING.md](../CONTRIBUTING.md)。


安全检查范围、按目标与版本限定的公告复核策略见 [SECURITY.md](../SECURITY.md)。

## Windows 原生远端 Runner

这不是 Windows 桌面客户端。Windows 开发与实测范围见 [WINDOWS_REMOTE.md](WINDOWS_REMOTE.md)。在 Windows x64 上运行：

```powershell
cargo build --locked --release --no-default-features
rustc tests/windows-claude-fixture.rs -o "$env:TEMP\ccbridge-claude-fixture.exe"
python tests/windows-remote-smoke.py target/release/ccbridge.exe "$env:TEMP\ccbridge-claude-fixture.exe"
```

Windows 独立 CI 使用 Windows Server 2022，产出 `ccbridge-windows-x86_64.exe`。Mac 两架构复用同一份经过验收的 Runner；ARM 发布仍不等待 Intel App。该 exe 随 Mac App 打包，也作为 Release 独立附件。Windows 私有目录使用当前用户 SID / SYSTEM ACL，版本目录安装不覆盖正在使用的 exe。真实 OpenSSH 与真实 Claude 验收分开记录；fixture 不消耗模型额度。
