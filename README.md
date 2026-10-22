# CCBridge

通过自己的 SSH 账号，把 Mac 上的 Codex MCP 接到服务器上的 Claude Code。Rust 核心 + Tauri 2 原生宿主，界面采用 React 与公共 shadcn/ui、Radix 控件，不依赖 Electron。

## 下载与安装

在 [Releases](https://github.com/akaryc1b/CCBridge/releases) 按 Mac 芯片下载 `CCBridge-macos-arm64.dmg`（Apple Silicon）或 `CCBridge-macos-x86_64.dmg`（Intel），打开后将 `CCBridge.app` 拖入 Applications，再从应用程序启动。ZIP仅作恢复备用；ARM先发，Intel随后追加到同一版本。

- Mac App：Apple Silicon / Intel x86_64，macOS 12+。Intel 包已通过 Rosetta CLI 与 SSH/MCP 协议验收；实际 Intel 设备的图形界面仍需在设备上确认。
- 配套服务器 Runner：Linux x86_64，CI 在 Ubuntu 22.04 构建；Mac arm64 可使用 App 内的同架构程序。
- App 当前为 ad-hoc 签名，未做 Developer ID 签名或 Apple 公证。正常更新和连接只读取本机私有文件；仅显式迁移旧钥匙串凭据时可能要求系统授权。
- 服务器必须已有可运行、已认证的 Claude Code。App 不安装 Claude，也不复制模型凭据。

## 外观、密码保存与启动连接

左下角可选择亮色、暗色或跟随系统。工作台可直接打开项目文件或新建任务；普通浏览不会更改MCP默认项目。

“设置 → 启动App时自动连接SSH”默认关闭。开启前需保存密码并确认服务器指纹；每次GUI启动只尝试一次。失败可在服务器页手动重试，主动断开后不重连，也不重放任务。

勾选“记住密码”后只使用本机保存，不再提供钥匙串保存选项。**本机保存是明文私有文件，不等同于钥匙串保护**：目录0700、文件0600，同一Mac账号的程序仍可能读取。风险首次确认后持久化，更新和重启不重复提示。保存前排除Time Machine；其他备份工具需用户自行排除。密码不写入profile或MCP配置。旧版凭据可在App点击“迁移此账号已保存的密码”，仅显式读取此账号的钥匙串并保留原副本，不自动批量导出。Codeup本机和远端作用域分别保存，并提供旧凭据显式迁移入口；正常读取和保存不访问钥匙串。

应用内更新继续下载并验证独立签名包，不需要每次重新安装DMG。非标准位置启动时提示移到应用程序；已有旧版时保留它，提示Finder替换或使用签名更新，不强行覆盖运行中的副本。

## Codeup、分支与文件管理

在“项目与文件”点击 **Codeup 认证 / 检测**，选择“本机获取”或“服务器当前 SSH 账号直接获取”，输入普通 HTTPS 仓库地址即可检测已有 Git 凭据。缺少认证时，在同一弹窗输入 Codeup HTTPS 克隆用户名及密码 / 访问令牌，点击“保存认证”，然后检测访问。留空密码不会覆盖已保存的认证；不接受把令牌嵌进仓库 URL。

**App 认证与 VPS 认证是两份独立状态。** 本机档用于在 Mac 获取代码再传输；服务器档保存在 Mac、按 SSH 主机/端口/账号隔离，仅在 App 内执行 Git 时临时经 SSH 提供。未保存 App 认证时沿用对应机器已有 Git 凭据助手。克隆只传代码，不自动复制认证。

要让 VPS 终端或远端 Claude 后续直接 `git pull`，可随时在此窗口 **同步到当前 VPS**，不需要重新克隆。选择本机档或服务器档，填写专用令牌，或留空复用已有认证（本机档可用 Git 凭据助手）；勾选针对当前 VPS 账号的风险确认，再点同步。凭据只写入当前账号私有目录（目录0700、文件0600），通过单独 Git include 配置限定 Codeup HTTPS，不改 SSH 登录、仓库 URL、其他账号或其他域名。该 VPS 账号下的程序（包括 Claude）及管理员可读取；建议服务器专用的最小权限令牌。VPS 托管认证启用后，服务器克隆与 fetch 优先使用它，不再被 Mac 上的旧服务器档覆盖；认证失败不会自动换身份重试。**检测 VPS 自身认证**不注入 Mac 凭据，检测成功才代表 VPS 具备仓库访问权限。移除操作仅撤掉本 App 管理的配置和凭据，Mac 认证与其他 Git 配置保留。

在项目或仓库根目录点击工具栏 **Git 更新**：支持单仓库和项目目录的直接子仓库，先列出分支/跟踪关系，再获取各自跟踪远端并只做快进。脏工作区、分叉、无跟踪分支、活动任务或确认后分支变化均明确拒绝；多仓库分别报告结果。不自动 stash/reset、不切分支、不推送。Codex 可调用 `git_update(path="远端绝对路径", dry_run=true)` 查看，再调用 `git_update(path=..., expected=上次返回的repositories)` 更新；写入需要 App 已开启全自动授权。MCP 需重新加载或新开 Codex 会话才能发现新增工具。本地代码仍由 Codex 的本机 Git 工具处理；本机修改必须先提交并推送到 Codeup，VPS 更新才取得到，未提交文件不会自动同步。

打开具体 Git 仓库根目录，点击 **Git 分支**：查看当前分支与未提交状态、切换本地分支、新建分支、从远端引用建立跟踪分支；“获取远端分支”只 fetch、不合并。不会自动 stash、reset 或推送，有未提交改动或相关运行任务时拒绝切换。

每个文件或文件夹右侧的 **…** 提供重命名、移动和移入回收区。移动可浏览目标文件夹；所有同名目标均不覆盖。回收区可恢复到原路径，同名冲突时保留回收项；不提供永久删除。支持新建空文件，已有新建文件夹入口保持不变。

文件操作限当前 SSH 账号 HOME；保护 Git 元数据、凭据目录、CCBridge 运行配置、已注册项目根目录、当前工作区及相关运行任务。跨磁盘移动暂不支持；注册项目根目录不能直接重命名或移走，以免破坏项目/数据库映射。新功能需要在服务器页安装与 App 同版本的 Runner。

## 连上服务器并打开项目

1. 在“服务器”输入实际主机、端口、用户名和密码。22 只是 SSH 标准默认值，可改成实际端口。首次连接请向管理员独立核对服务器指纹。
2. 可选择在本机记住SSH密码。其他配置按服务器、端口、账号独立保存；程序从远端获取真实 HOME。
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

“会话”按项目分组会话，组内按最近更新时间排列，可折叠项目；项目子目录归入最具体的已注册项目，未注册目录单独分组。消息展示 Markdown；工具过程和原始事件折叠显示。追问保持同一 Claude 会话和远端目录，切换 App 工作区不会改变旧会话目录。⌘ Enter 发送，Enter 换行。

MCP 新任务默认审批模式；App 输入框会明确显示下一轮模式。用户可在 App 开启“允许选择全自动模式”，再为下一轮选择全自动。MCP 新任务必须显式传入对应模式；MCP 追问沿用上一轮模式，不允许静默提升权限。运行中的一轮权限固定。

**全自动会跳过 Claude 工具审批，可以使用该 SSH 账号的文件、命令和网络权限。工作目录不是 OS 沙箱。** 请用独立、低权限的 SSH 账号；关闭开关不会追溯降低已运行任务的权限，必要时取消任务。

## 模型与用量

连接 SSH 并更新 Runner 后，App 自动从当前账号的远端 Claude Code 获取模型清单。服务器默认模型与对话下一轮模型均从下拉框选择，不提供手填模型 ID；可刷新列表，获取失败时就近重试。别名及其解析值来自远端，不把历史会话模型当作可选清单。探测只做控制协议初始化，不发送用户提示或启动模型推理。

新会话使用账号默认选择；续聊保留原会话的选择，改模型只作用于下一轮。每轮实际模型取自 Claude 主会话上报，不以请求别名代替。已保存的旧模型若不在远端清单中，需重新选择。

服务器页的“Claude 用量”显示当前 SSH 账号下 CCBridge 任务最近上报的五小时和每周额度、恢复时间及上报时间。读取仅查看任务事件，不发起模型请求、不读取模型凭据；它不是独立实时查询全部 Claude 客户端的额度。未上报时不显示0%，额度窗口到期后等待新上报。其他终端产生的使用量要等 Claude 再次上报后才能反映。

MCP `cc_start` / `cc_reply` 的 `model` 和 `model_policy`（`inherit`、`remote_default`、`explicit`）保持兼容。旧调用方式仍可用，不因 App 改为下拉框增加前置条件。同一任务 UUID 不允许换模型重放。

## 接入 Codex

在 App 的“Codex / MCP”点击“自动写入MCP配置”，再重新加载 MCP 或新开会话。只更新 `mcp_servers.ccbridge`，保留其他服务与配置；修改前创建私有备份，无改动不重复备份。

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

推送和 PR 执行 Rust 格式、Linux Runner 测试/烟测、Mac SSH/MCP 集成测试、前端测试与 Tauri 编译。标签 `vX.Y.Z` 与版本一致，Linux/数据库公共检查及对应架构检查通过后，该架构立即发布，不等待另一架构。ARM 与 Intel 分别构建、独立发布到同一个 Release；后完成的架构追加文件，并合并 `latest.json`，不会覆盖先完成的架构。尚未完成的架构不会被冒充为可更新；可以先下载已完成架构。发布包含：

- `CCBridge-macos-arm64.dmg` / `CCBridge-macos-x86_64.dmg`：按芯片首次安装或应急恢复；应用内更新自动按架构选择更新包
- `CCBridge.app.tar.gz`（ARM）、`CCBridge-macos-x86_64.app.tar.gz`（Intel）与各自 `.sig`：签名更新包
- `latest.json`：GitHub 更新清单
- `ccbridge-linux-x86_64`：配套 Runner

构建缓存分为 Linux、ARM、Intel 三组，固定 Rust 1.93.0；缓存 Cargo 下载及依赖编译产物（包含根目录和 Tauri 的 debug/release），不缓存签名密钥或最终安装包。主分支会完成 Tauri release 编译预热并保存缓存，后续标签可恢复主分支缓存；锁文件变化时尽量复用未变化依赖。pnpm 使用锁文件对应的下载缓存，Node 22 由 setup-node 提供，不再每轮通过 Homebrew 安装。首次运行、工具链或依赖发生变化仍需要编译，缓存命中不跳过测试、签名或发布验证。缓存是否精确命中可在各构建任务的 Summary 查看。

使用 Tauri 独立更新签名与 ad-hoc App 签名，**不需要 Developer ID 或 Apple 公证**。GitHub Secrets 中保存 `TAURI_SIGNING_PRIVATE_KEY` 与可选密码。公钥随 App 发布；下载、网络或签名验证失败不替换当前 App。新 App 完全无法启动时，从上一版本 ZIP 应急恢复，而不是依赖失效 App 自己回滚。

CI 不保存 SSH/数据库密码，不连接开发或生产服务器，不自动部署用户 Runner。GUI 登录与真实服务器验收在授权环境完成。维护说明见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证与安全

项目代码采用 [MIT](LICENSE)。第三方许可证保留在 `SHADCN-EGUI-LICENSE` 与 `licenses/` 中；shadcn/ui 组件来源及许可证见 `ui/THIRD-PARTY.md`。安全报告与凭据边界见 [SECURITY.md](SECURITY.md)。
