# 开发与发布

## 开发

使用 Rust 1.93.0。先阅读 README 的构建步骤。修改后运行格式检查与对应测试。完整 SSH/MCP 验收要求先构建 release，并将 `CCBRIDGE_TEST_BIN` 指向同一源码版本的程序。

不要提交 `dist*`、`target`、运行日志、用户配置、凭据或真实会话。新 UI 使用既有公共组件。数据库功能暂缓，不在此阶段新增数据库能力。

## 发布

1. 更新 Cargo.toml 的版本及 Cargo.lock 对应包版本。打包脚本从 Cargo.toml 读取 App 版本，不再维护另一份常量。
2. 推送代码，等待 CI 通过。
3. 在同一已验证提交创建并推送标签，例如 `git tag v0.4.2 && git push origin v0.4.2`。
4. 标签 CI 校验版本，构建/测试 Linux 与 Mac，生成 GitHub Release。
5. 检查 Release 的两个产物和 Actions 结果。GUI 登录和真实服务器验收仍需在授权的环境中进行。

发布只分发软件，不自动替换任何人的运行环境。回退时使用上一版本 Release；远端手动更新会备份原 Runner 为 `.previous`。
