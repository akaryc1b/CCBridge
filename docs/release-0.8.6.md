# 0.8.6：Git 更新与可补同步的 VPS Codeup 认证

- 项目与文件工具栏新增 Git 更新：检查单仓库或直接子仓库的分支/上游，fetch 后仅快进，分别显示成功、已最新、领先或失败；脏工作区、分叉、活动任务、无跟踪分支和确认后分支变化都会阻止对应操作。不 stash/reset、不切分支、不推送。
- Codex MCP 新增 git_update：显式远端路径、只读 dry_run 和可选 expected 快照，复用 App / VPS Git 认证。旧 MCP 调用保持兼容；重新加载 MCP 或新建会话可发现新工具。
- Codeup 认证页分别显示 Mac 与 VPS 状态。可在克隆之后随时补同步到当前 VPS，不必重新克隆。凭据来源为当前输入、App 已保存认证或本机 Git 凭据助手；每次写入 VPS 都需要人在 App 确认。
- VPS 使用 Git 原生 credential-store + 独立 include，文件0600/目录0700，仅当前账号及 Codeup HTTPS；不修改 SSH 登录或仓库 URL。检测 VPS 自身认证不注入 Mac 凭据。可更新或移除本 App 管理的 VPS 认证，保留其他 Git 配置。
- 应用升级只保留侧栏“应用更新”一个入口，移除设置页重复按钮；Git 更新只针对仓库代码。

同一 VPS 账号程序及管理员可读取其持久凭据，建议最小权限的服务器专用令牌。本机未提交代码不会自动同步到 VPS；需先提交并推送。DMG 首装、独立签名应用内更新，不使用 Developer ID；ARM 先发布，Intel 随后追加。
