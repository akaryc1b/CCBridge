# CCBridge

通过自己的 SSH 账号，把 Mac 上的 Codex MCP 接到服务器上的 Claude Code。使用 Rust 构建，界面采用公共 shadcn-egui 和 egui_commonmark，不依赖 Electron。

## 下载与安装

在 [Releases](https://github.com/akaryc1b/CCBridge/releases) 下载 `CCBridge-macos-arm64.zip`，解压后把 `CCBridge.app` 放到 `/Applications`。

- Mac App：Apple Silicon，macOS 12+；Intel Mac 暂无验收版本。
- 配套服务器 Runner：Linux x86_64，CI 在 Ubuntu 22.04 构建；Mac arm64 可使用 App 内的同架构程序。
- App 当前为 ad-hoc 签名，未做 Developer ID 签名或 Apple 公证。更新可能触发 macOS 钥匙串重新授权，授权必须由用户在系统中完成。
- 服务器必须已有可运行、已认证的 Claude Code。App 不安装 Claude，也不复制模型凭据。

## 连上服务器并打开项目

1. 在“连接与权限”输入实际主机、端口、用户名和密码。22 只是 SSH 标准默认值，可改成实际端口。首次连接请向管理员独立核对服务器指纹。
2. 可选择把 SSH 密码保存在 macOS 钥匙串。其他配置按服务器、端口、账号独立保存；程序从远端获取真实 HOME。
3. 首次使用点击“安装当前账号 Runner”。程序只上传到当前账号 HOME，不使用 sudo。已安装时显示版本；同版本禁用重复安装，后台也跳过上传。版本不同才提供更新入口，旧程序备份为 `.previous`。
4. 在“远端工作区”浏览目录、创建项目文件夹、克隆 HTTPS 仓库，或打开已有项目。将所选目录设为 CC 工作目录。

一个项目可以包含多个独立仓库，无需给父目录再初始化 Git：

```text
<账号 HOME>/projects/my-project/
├── backend/
├── web/
└── mobile/
```

选择父目录用于跨仓库任务，选择具体仓库用于单仓库任务。已有目标仓库不自动覆盖、拉取或切分支。

克隆支持两种认证来源：使用服务器当前账号的 Git 认证，或使用本机 Git 认证取得镜像后通过 SSH 传输 Git bundle。后者不上传本机凭据、Git 配置或未提交改动，也不让服务器自动获得以后 fetch 的认证。

资源管理器只浏览当前账号 HOME 内的路径，不沿符号链接越界。文本预览为只读 UTF-8，限 128 KiB，不加载密钥、隐藏文件或二进制内容。

## 对话与权限

“任务与对话”按会话展示 Markdown 消息；工具过程和原始事件折叠显示。追问保持同一 Claude 会话和远端目录，切换 App 工作区不会改变旧会话目录。⌘ Enter 发送，Enter 换行。

默认审批模式。用户可在 App 开启“允许选择全自动模式”，再为下一轮选择全自动。MCP 新任务必须显式传入对应模式；MCP 追问沿用上一轮模式，不允许静默提升权限。运行中的一轮权限固定。

**全自动会跳过 Claude 工具审批，可以使用该 SSH 账号的文件、命令和网络权限。工作目录不是 OS 沙箱。** 请用独立、低权限的 SSH 账号；关闭开关不会追溯降低已运行任务的权限，必要时取消任务。

## 接入 Codex

在 App 的“Codex 接入”点击“一键配置 Codex”，再重新加载 MCP 或新开会话。只更新 `mcp_servers.ccbridge`，保留其他服务与配置；修改前创建私有备份，无改动不重复备份。

也可以合并以下片段到 Codex 的 `config.toml`：

```toml
[mcp_servers.ccbridge]
command = "/Applications/CCBridge.app/Contents/MacOS/ccbridge"
args = ["mcp"]
tool_timeout_sec = 960
```

不需要 `cwd`。其他 stdio MCP 客户端使用同样命令和参数。示例调用规则见 [integration/AGENTS-snippet.md](integration/AGENTS-snippet.md)。配置存在不代表当前会话已加载工具。

| MCP 工具 | 用途 |
|---|---|
| `connection_status` | 身份、连接、工作区、Runner/Claude 状态与全自动授权 |
| `project_list` | 已注册项目的名称、路径与数据库用途映射，不返回密码 |
| `repository_match` | 可选：按 Git remote 身份匹配明确指定的本地仓库 |
| `cc_start` | 新任务；支持 UUID 幂等请求与显式 `remote_project` |
| `cc_status` | 状态、审批、分页增量事件 |
| `cc_result` | 等待最终结果，单次最多 900 秒 |
| `cc_reply` | 原会话续聊，保持项目、会话和模式 |
| `cc_list` | 最近任务 |
| `cc_cancel` | 取消指定任务 |

多项目时用 `project_list` 的准确路径作为 `remote_project`，不要为了给某个会话路由而修改 App 全局选择。`remote_project` 与 `local_repository` 不能同时传。未指定时沿用 App 当前远端工作区与旧版兼容回退。

本地未提交文件不会自动同步到远端。任务响应不确定时先查询，或用原 UUID 重试，不要换新 ID 重复派发。只续接本会话记录的任务，不取全局最新任务冒充当前会话。

## 数据库功能：实验性，暂缓开发

源码保留多项目、多数据库映射和 MySQL/PostgreSQL 只读实例实现，目前不作为稳定交付承诺。已知问题：安装探测只覆盖 PATH，会漏掉 `/opt` 等位置的 universal-db-mcp；本轮不修复、不连接真实数据库。

实验功能要求已有 Node 与 universal-db-mcp 2.14.0。连接和任务快照保存在当前远端账号的私有文件中（目录 700、文件 600），不是操作系统对同一用户的凭据沙箱。固定 safe 模式不能替代数据库自身 SELECT 权限。不会读取共享管理员配置；查询内容可能进入模型上下文。

## 本地构建与测试

需要 Rust 1.93.0、Git、Python 3；Mac GUI 需 macOS SDK。

```sh
cargo build --locked --release
CCBRIDGE_TEST_BIN="$PWD/target/release/ccbridge" cargo test --locked
cargo fmt --all -- --check
./scripts/package-macos.sh --no-build
```

打包 Mac App 前，将配套 Linux Runner 放在 `dist/ccbridge-linux-x86_64`，否则 Linux 服务器不能从 App 自动安装。CI 自动完成该依赖。

服务器只构建 headless Runner：

```sh
cargo build --locked --release --no-default-features
cargo test --locked --no-default-features
sh tests/linux-smoke.sh "$PWD/target/release/ccbridge"
```

默认测试使用临时 SSH/Git/Claude fixture，不需要真实账号、模型或数据库。可选数据库集成脚本需要 Docker、Node、universal-db-mcp 2.14.0，只用于隔离测试；不在默认 CI 中连接外部数据库。

## CI / CD

推送和 PR 自动执行格式检查、Linux Runner 测试/烟测、macOS 完整 SSH/MCP 集成测试与 App 打包。构建产物保存在 Actions。推送与 Cargo 版本一致的 `vX.Y.Z` 标签后，只有上述任务全部成功，才自动发布 GitHub Release 和两个产物：

- `CCBridge-macos-arm64.zip`（含配套 Linux Runner）
- `ccbridge-linux-x86_64`

CI 不保存 SSH/数据库密码，不连接开发或生产服务器，不自动部署或重启用户的 App/Runner。发布使用 GitHub 自带临时令牌，不需要个人令牌 Secret。维护说明见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证与安全

项目代码采用 [MIT](LICENSE)。第三方许可证保留在 `SHADCN-EGUI-LICENSE` 与 `licenses/` 中。安全报告与凭据边界见 [SECURITY.md](SECURITY.md)。
