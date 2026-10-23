# Windows 原生远端：暂停交接

2026-10-23，用户决定暂不安装 Windows，保留独立分支，后续再决定是否继续。

## 保存位置与边界

- 分支：`codex/windows-native-remote`，GitHub 仓库 `akaryc1b/CCBridge`。
- 开发基线：main `4e3bdb006413d6e034c742fd498b50cd0b98a79a`（0.10.8）；暂停前代码提交 `13fcb5f`。
- 不合并 main、不创建正式版本、不替换现用 App。没有连接用户 Windows、安装 Runner 或安装 Claude；现有 Mac/Linux 服务不变。
- 开发方案：`docs/WINDOWS_REMOTE.md`。本分支不是已完成验收的发布版本。

## 已有证据

- Mac 共享核心 33 项单元测试、前端 29 项测试与构建、隔离 SSH/MCP 回归通过。
- 独立 Windows CI 原生编译、平台/文件/名称保护/删除恢复/Git 分支、PowerShell 引导与重复安装测试通过；Mac/Linux 回归通过。
- Windows 独立 Worker、全自动、续聊、模型事件和取消进程树通过的是确定性 fixture，不是真实 Claude。
- Windows HTTPS 引用访问通过，克隆失败，整体 Windows CI 未通过。最新记录：[39900731247](https://github.com/akaryc1b/CCBridge/actions/runs/39900731247)。

## 未完成项与下一步

1. Windows HTTPS clone：Git 报 `could not create work tree dir` / `Invalid argument`，路径含 Windows 扩展路径前缀 `\\?\`。需继续核实 Git 参数的路径表示并修复，不能把引用访问成功当克隆成功。
2. 克隆后的 Windows 凭据保存/检测/移除测试尚未执行；只读 ACL 校验已编译，仍需运行验证。
3. 用户 Windows OpenSSH 实机连接、Runner 安装、断线存活未验收。
4. Windows 真实 Claude 全自动、续聊、模型与用量未验收；未来有 Windows VPS 并安装登录 Claude 后才可完成。

仅在用户决定恢复后继续；先处理上述 CI 克隆失败，再安排实机验收，不自动部署或发布。

## 隔离测试 App

本机曾打开 `CCBridge Windows Remote Test.app`，使用独立数据目录，暂停时正常退出。没有替换 `/Applications/CCBridge.app`。

开发候选包位于工作树 `target/debug/bundle/macos/CCBridge Windows Remote Test.app`；其内 Windows 候选 Runner 来自 `80ecb81`，早于暂停前代码，不是最终交付包，不用于正式安装。
