# 开发与发布

## 开发

使用 Rust 1.93、Node.js 22、pnpm 10。先阅读 README 构建步骤。Rust 核心在 `src/`，正式 GUI 在 `src-tauri/` 和 `ui/`；旧 egui 仅保留为开发回退。

默认测试使用临时 SSH / Git / Claude fixture，不能连接真实模型、账号或数据库。完整 SSH/MCP 验收先构建 release，并将 `CCBRIDGE_TEST_BIN` 指向同一源码版本的程序。

不要提交 `dist*`、`target`、运行日志、用户配置、私钥或真实会话。数据库功能暂缓。

## 发布

1. 同步根 Cargo.toml、Cargo.lock、src-tauri/Cargo.toml、src-tauri/Cargo.lock、tauri.conf.json 与 ui/package.json 版本。
2. 本机执行对应测试，推送代码并等待 CI。
3. 在通过检查的提交打 `vX.Y.Z` 标签。标签构建需要仓库 Secret `TAURI_SIGNING_PRIVATE_KEY`，密码可留空；公钥在 Tauri 配置中。
4. CI 自动发布 ZIP、签名 tar.gz、sig、latest.json 与 Linux Runner。检查清单版本、平台 `darwin-aarch64`、HTTPS 下载 URL 与签名存在。
5. Mac 实测从上一签名版本检查 → 下载并验签 → 安全排空 → 安装重启 → SSH/MCP 恢复。

不配置 Developer ID 或 Apple 公证。发布签名密钥保存在仓库之外的受限位置；不要在日志、issue、PR 或命令输出中展示。更新私钥丢失将影响旧 App 的更新信任链，必须安全保管。

发布软件不等于部署 Runner。远端更新由 App 在当前账号执行，保留 `.previous`；不自动降级较新 Runner。应急恢复使用上一个已验证 ZIP，保留用户数据目录。
