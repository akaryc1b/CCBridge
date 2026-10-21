# CCBridge

通过自己的 SSH 账号，把 Mac 上的 Codex MCP 接到服务器上的 Claude Code。Rust 核心 + Tauri 2 原生宿主，界面采用 React 与公共 shadcn/ui、Radix 控件，不依赖 Electron。

## 下载与安装

在 [Releases](https://github.com/akaryc1b/CCBridge/releases) 下载 `CCBridge-macos-arm64.zip`，解压后把 `CCBridge.app` 放到 `/Applications`。

- Mac App：Apple Silicon，macOS 12+；Intel Mac 暂无验收版本。
- 配套服务器 Runner：Linux x86_64，CI 在 Ubuntu 22.04 构建；Mac arm64 可使用 App 内的同架构程序。
- App 当前为 ad-hoc 签名，未做 Developer ID 签名或 Apple 公证。更新可能触发 macOS 钥匙串重新授权，授权必须由用户在系统中完成。
- 服务器必须已有可运行、已认证的 Claude Code。App 不安装 Claude，也不复制模型凭据。

## 连上服务器并打开项目

1. 在“服务器”输入实际主机、端口、用户名和密码。22 只是 SSH 标准默认值，可改成实际端口。首次连接请向管理员独立核对服务器指纹。
2. 可选择把 SSH 密码保存在 macOS 钥匙串。其他配置按服务器、端口、账号独立保存；程序从远端获取真实 HOME。
3. 首次使用点击“安装当前账号 Runner”。程序只上传到当前账号 HOME，不使用 sudo。已安装时显示版本；同版本禁用重复安装，后台也跳过上传。版本不同才提供更新入口，旧程序备份为 `.previous`。
4. 在“项目与文件”浏览目录、创建项目文件夹、克隆 HTTPS 仓库，或打开已有项目。蓝色高亮表示正在浏览的项目，“新任务目录”标签表示新会话工作目录；点击“设为新任务工作目录”才切换后者。浏览与切换项目不会改变旧会话目录。地址输入尚未加载成功时，不允许克隆、建文件夹或注册项目。

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

“任务与对话”按项目分组会话，组内按最近更新时间排列，可折叠项目；项目子目录归入最具体的已注册项目，未注册目录单独分组。消息展示 Markdown；工具过程和原始事件折叠显示。追问保持同一 Claude 会话和远端目录，切换 App 工作区不会改变旧会话目录。⌘ Enter 发送，Enter 换行。

MCP 新任务默认审批模式；App 输入框会明确显示下一轮模式。用户可在 App 开启“允许选择全自动模式”，再为下一轮选择全自动。MCP 新任务必须显式传入对应模式；MCP 追问沿用上一轮模式，不允许静默提升权限。运行中的一轮权限固定。

**全自动会跳过 Claude 工具审批，可以使用该 SSH 账号的文件、命令和网络权限。工作目录不是 OS 沙箱。** 请用独立、低权限的 SSH 账号；关闭开关不会追溯降低已运行任务的权限，必要时取消任务。

## 每轮选择模型

在“服务器”设置默认模型；留空沿用远端 Claude 配置。在对话输入框可选择：

- **下一轮模型名称**：输入框直接显示已配置的具体名称。新对话使用服务器默认，续聊使用该会话上一轮的选择，不受之后修改服务器默认值影响。
- **远端配置**：不传 `--model`，由远端 Claude 决定。尚未确认时显示“模型待确认”；已有上报值时同时标注远端默认来源。
- **指定模型**：输入别名（如 `sonnet`、`opus`）或远端服务支持的完整模型 ID。

每轮分别保存请求模型与 Claude 主会话 init 事件上报的实际模型。未上报时显示未知，不用请求别名或子任务用量推断实际模型。上报结果不等于独立验证模型供应商。

MCP `cc_start` / `cc_reply` 支持可选 `model` 和 `model_policy`（`inherit`、`remote_default`、`explicit`）。省略保持兼容；传 `model` 默认按明确指定处理。同一任务 UUID 不允许换模型重放。

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
| `cc_result` | 等待最终结果，单次最多 900 秒；`include_events=false` 返回紧凑结果 |
| `cc_reply` | 原会话续聊，保持项目、会话和模式 |
| `cc_list` | 最近任务 |
| `cc_cancel` | 取消指定任务 |

多项目时用 `project_list` 的准确路径作为 `remote_project`，不要为了给某个会话路由而修改 App 全局选择。`remote_project` 与 `local_repository` 不能同时传。未指定时沿用 App 当前远端工作区与旧版兼容回退。

本地未提交文件不会自动同步到远端。任务响应不确定时先查询，或用原 UUID 重试，不要换新 ID 重复派发。只续接本会话记录的任务，不取全局最新任务冒充当前会话。

## 多项目、多数据库 MCP

在“数据库”页面，先点“探测远端 MCP 环境”，再新增数据库连接并保存。探测会检查当前账号的 Claude MCP 注册、包装器、`/opt` 等标准目录及 nvm，复用服务器已安装的 universal-db-mcp，不会重新安装，也不会复制或覆盖 `/etc` 共享配置。

每条连接填写名称、MySQL/PostgreSQL 类型、地址、端口、用户名、密码和数据库名。运行环境路径可从探测结果自动带入，也可在高级区域手工修正。当前启动适配器已验证 universal-db-mcp **2.14.0**；其他版本会显示未兼容，而不是误报未安装。

三个状态分别验证：
- **运行文件探测**：找到 Node、包目录及版本；不启动数据库连接。
- **检查 MCP 握手**：完成 initialize 和 tools/list，核对命名连接的工具过滤；即使数据库不可达也可以通过，不能证明认证成功。
- **测试数据库连接**：实际认证和连接；不读业务表。生产数据库请使用本身仅有 SELECT 权限的账号。

在“项目数据库映射”选择准确项目目录，为每条连接填写用途别名，例如 `business_dev`、`workflow`、`reporting`。同一项目最多映射 8 条连接；不同项目可复用连接、使用相同别名而互不串库。Claude 工具按 `db_<用途别名>` 命名，`project_list` 返回脱敏映射供 Codex 选择项目。

连接固定 `safe` 与 `allowWrite=false`，禁止模型动态切换连接。全自动模式不会改变数据库权限。不会直接继承共享包装器的潜在写权限，也不会把密码放进 MCP argv 或提示词。连接和会话快照含凭据，保存在当前 SSH 账号的私有 HOME 状态目录，文件从创建时就是 0600；同账号及服务器管理员仍可读取。

映射或连接修改只影响新会话；续聊保留原快照，界面会提示配置已变更。需要切换数据库或更新凭据时请新建对话。编辑密码留空保留已保存密码。删除被项目引用的连接会被阻止，须先移除映射。

## 本地构建与测试

需要 Rust 1.93.0、Node.js 22、pnpm 10、Git、Python 3；Mac 需 Xcode 命令行工具与 macOS SDK。

```sh
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui test
pnpm --dir ui build
cargo build --locked --release
CCBRIDGE_TEST_BIN="$PWD/target/release/ccbridge" cargo test --locked
cargo fmt --all -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

默认 Cargo GUI 保留旧 egui 开发回退；正式 App 使用 `src-tauri` 宿主，与 CLI / MCP / Runner 共用 Rust 库。正式打包前将 Linux Runner 放入 `dist/ccbridge-linux-x86_64`，设置独立的更新签名密钥后运行 `scripts/package-tauri.sh`。签名密钥不进入源码。

服务器只构建 headless Runner：

```sh
cargo build --locked --release --no-default-features
cargo test --locked --no-default-features
sh tests/linux-smoke.sh "$PWD/target/release/ccbridge"
```

默认测试使用临时 SSH/Git/Claude fixture，不需要真实账号、模型或数据库。可选数据库集成脚本需要 Docker、Node、universal-db-mcp 2.14.0，只用于隔离测试；不在默认 CI 中连接外部数据库。

## GitHub 应用内更新与 CI / CD

首次从旧版迁移到 Tauri 需要替换一次 App；后续在“应用更新”检查、下载、验证签名后安装并重启。账号数据仍在 `~/Library/Application Support/CCBridge`，MCP 路径仍是 `/Applications/CCBridge.app/Contents/MacOS/ccbridge`。

更新前会排空本地写入、检查远端任务，并等待 MCP 长调用租约结束。执行中的任务和审批不会被强制中断。旧版 MCP 没有租约协议，旧等待调用会保守地阻止更新至其最大等待窗口结束；重新加载 MCP 后的新调用使用租约。无法确认远端状态时不安装。

推送和 PR 执行 Rust 格式、Linux Runner 测试/烟测、Mac SSH/MCP 集成测试、前端测试与 Tauri 编译。标签 `vX.Y.Z` 与版本一致且所有检查通过后，发布：

- `CCBridge-macos-arm64.zip`：首次安装或应急恢复
- `CCBridge.app.tar.gz` 与 `.sig`：签名更新包
- `latest.json`：GitHub 更新清单
- `ccbridge-linux-x86_64`：配套 Runner

使用 Tauri 独立更新签名与 ad-hoc App 签名，**不需要 Developer ID 或 Apple 公证**。GitHub Secrets 中保存 `TAURI_SIGNING_PRIVATE_KEY` 与可选密码。公钥随 App 发布；下载、网络或签名验证失败不替换当前 App。新 App 完全无法启动时，从上一版本 ZIP 应急恢复，而不是依赖失效 App 自己回滚。

CI 不保存 SSH/数据库密码，不连接开发或生产服务器，不自动部署用户 Runner。GUI 登录与真实服务器验收在授权环境完成。维护说明见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证与安全

项目代码采用 [MIT](LICENSE)。第三方许可证保留在 `SHADCN-EGUI-LICENSE` 与 `licenses/` 中；shadcn/ui 组件来源及许可证见 `ui/THIRD-PARTY.md`。安全报告与凭据边界见 [SECURITY.md](SECURITY.md)。
