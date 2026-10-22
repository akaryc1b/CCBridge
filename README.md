# CCBridge

[![CI](https://github.com/akaryc1b/CCBridge/actions/workflows/ci.yml/badge.svg)](https://github.com/akaryc1b/CCBridge/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/akaryc1b/CCBridge)](https://github.com/akaryc1b/CCBridge/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**在 Mac 上管理 SSH 项目，让 Codex 通过 MCP 调用远端 Claude Code。**

CCBridge 将 SSH 连接、远端文件、Git 操作和 Claude 会话放在同一个桌面应用中。采用 Rust + Tauri 2，界面基于 React、shadcn/ui 与 Radix，不依赖 Electron。

```text
Codex / MCP 客户端 ── stdio ── CCBridge Mac App
                                  │ SSH
                                  ▼
                           当前账号的 Runner
                                  │
                           Claude Code / 项目
```

## 功能

- **SSH 连接**：账号密码登录、服务器指纹核对、本机保存密码、启动自动连接。
- **项目与文件**：多项目、多仓库；目录浏览、文本预览、新建、重命名、移动、回收与恢复。
- **Git 与 Codeup**：克隆进度、访问检测、认证保存、可选 VPS 认证同步、分支管理、只快进更新。
- **Claude 会话**：按项目分组、Markdown 对话、多轮续聊、任务状态、审批或全自动执行。
- **模型与额度**：从远端 Claude 获取可选模型；展示每轮实际模型，以及最近上报的五小时和每周额度。
- **数据库 MCP**：项目可映射多个 MySQL / PostgreSQL 连接，按用途隔离，固定只读模式。
- **桌面体验**：亮色 / 暗色 / 跟随系统；GitHub 签名应用内更新，保护运行中的任务。

## 下载与安装

从 [最新 Release](https://github.com/akaryc1b/CCBridge/releases/latest) 下载对应安装包：

| Mac 芯片 | 安装包 | 系统要求 |
|---|---|---|
| Apple Silicon（M 系列） | `CCBridge-macos-arm64.dmg` | macOS 12 或更高 |
| Intel | `CCBridge-macos-x86_64.dmg` | macOS 12 或更高 |

打开 DMG，将 **CCBridge.app 拖入 Applications**，再从“应用程序”启动。后续在 App 的“应用更新”中升级，无需反复安装 DMG。ZIP 用于备用恢复。

应用未使用 Developer ID 签名或 Apple 公证，首次打开可能需要按 macOS 提示允许。应用内更新使用独立签名验证；验证失败不会替换现用 App。

Windows 尚未发布，计划见 [Windows 支持范围](docs/WINDOWS_SUPPORT.md)。

## 快速开始

### 1. 连接服务器

在“服务器”填写实际主机、SSH 端口、用户名和密码，首次连接核对服务器指纹。密码只在 App 输入，不要发送给模型或写入 MCP 配置。

远端需要已安装并完成认证的 **Claude Code**。CCBridge 不代装 Claude，也不复制 Claude 的登录凭据。

### 2. 安装 Runner 并选择项目

连接后，在服务器页安装当前账号的 Runner；App 会显示版本和安装状态，同版本不会重复上传。正式包提供 Linux x86_64 Runner，不需要 sudo。

在“项目与文件”打开已有目录，或创建项目并克隆仓库。一个项目可以包含后端、Web、移动端等多个仓库。浏览目录与 MCP 默认项目分开管理，续聊保留原会话目录。

### 3. 接入 Codex

在“Codex / MCP”点击 **自动写入 MCP 配置**，然后重新加载该 MCP，或新开 Codex 会话。不要在其他任务运行时强行重载。

手动配置时，将以下片段合并到 Codex 的 `config.toml`，不要覆盖整个文件：

```toml
[mcp_servers.ccbridge]
command = "/Applications/CCBridge.app/Contents/MacOS/ccbridge"
args = ["mcp"]
tool_timeout_sec = 960
```

无需设置项目 `cwd`。其他 stdio MCP 客户端使用相同命令和参数。

将 [调用规则示例](integration/AGENTS-snippet.md) 配置到项目约束后，可以直接说：

> 使用 Claude 分析这个项目的登录流程，只分析，不修改代码。

模型执行前需确认连接和准确项目路径；多轮追问使用原会话，不能将“已派发”当作任务完成。

## 需要了解的边界

- **全自动**：App 新任务默认选择全自动，但需用户先开启对应账号的授权。MCP 调用应显式指定模式；省略时协议默认审批，续聊沿用原模式。全自动可使用该 SSH 账号的文件、命令和网络权限，工作目录不是系统沙箱。
- **代码同步**：远端 Claude 读取服务器上的代码；本机未提交内容不会自动上传。Git 更新只获取当前跟踪分支并快进，拒绝脏工作区、分叉等情况，不自动 stash、reset 或推送。
- **认证作用域**：Mac 认证与 VPS 持久认证不同。若要在 VPS 普通终端或 Claude 中直接 `git pull`，需在 App 明确确认后同步到该 VPS 账号；克隆代码不会自动同步凭据。
- **密码保存**：使用本机私有文件保存，不是加密保险库。同一系统账号的程序及管理员可能读取；首次保存会提示风险。VPS 保存认证同样需要明确确认。
- **模型与用量**：模型列表来自远端，可刷新，无需为模型增减更新 App；远端 Claude Code 本身须支持相应模型。额度是当前账号 CCBridge 任务的最近上报，不是全部 Claude 客户端的实时账单。
- **数据库权限**：固定只读保护不能代替数据库本身的 SELECT 权限；建议使用专用低权限账号。

完整操作、MCP 工具和状态含义见 [使用指南](docs/USER_GUIDE.md)。

## 开发

需要 Rust 1.93.0、Node.js 22、pnpm 10、Git、Python 3；Mac 还需 Xcode 命令行工具和 macOS SDK。

```sh
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui test
pnpm --dir ui build
cargo test --locked --no-default-features
```

默认测试使用隔离的 SSH / Git / Claude fixture，不需要真实账号。正式构建、完整集成测试和签名打包见 [构建与发布指南](docs/BUILDING.md)。

| 目录 | 内容 |
|---|---|
| `src/` | Rust 核心、SSH、Runner、MCP 与状态管理 |
| `src-tauri/` | Tauri 原生宿主与应用配置 |
| `ui/` | React 界面与公共组件 |
| `integration/` | MCP 接入与调用规则 |
| `tests/` | 协议、Git、数据库等隔离测试 |
| `docs/` | 使用、构建、UI 规范与版本说明 |

## 发布与贡献

GitHub Actions 分别构建 ARM 和 Intel，共用 Linux / 数据库检查，独立缓存和发布。**ARM 先发布，Intel 通过后追加到同一 Release**；更新清单合并保留两个平台。

欢迎通过 [Issues](https://github.com/akaryc1b/CCBridge/issues) 报告问题或提出建议。提交代码前阅读 [贡献指南](CONTRIBUTING.md)；涉及界面的改动遵循 [UI、状态与文案规范](docs/UI_GUIDELINES.md)。安全问题按 [SECURITY.md](SECURITY.md) 报告，不要公开凭据或私有会话。

## 许可证

项目采用 [MIT License](LICENSE)。第三方许可证保留在 [licenses/](licenses/)、[SHADCN-EGUI-LICENSE](SHADCN-EGUI-LICENSE) 和 [UI 第三方说明](ui/THIRD-PARTY.md) 中。
