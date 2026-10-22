# GitHub 自动同步到 Codeup

GitHub 是主仓库和构建发布源，Codeup 是第二远端。独立工作流 `Sync Codeup` 在分支或标签推送后同步全部当前分支和标签，也可在 Actions 中手动运行。现有 `CI and Release` 不变，无需发布新版 App。

## 配置一次凭据

在 GitHub 仓库 Settings → Secrets and variables → Actions 添加 Repository secrets：

- `CODEUP_USERNAME`：Codeup HTTPS 克隆用户名。
- `CODEUP_TOKEN`：拥有 CCBridge 仓库推送权限的克隆密码或专用令牌。

建议使用仓库范围最小权限的专用凭据。Mac 上的 Git 凭据助手只供本机使用，GitHub 托管 Runner 无法访问本机钥匙串。不要将凭据写入源码、远端 URL 或日志。

## 同步规则

- 串行执行，每次重新获取 GitHub 当前引用，避免较旧事件覆盖较新状态。
- 原子推送分支与标签；不推送 PR、跟踪分支等内部引用。
- 不强推、不删除 Codeup 引用。分支改写、标签重打或 Codeup 上出现独立提交时，同步失败并保留远端内容，需要人工决定如何处理。
- 删除 GitHub 分支或标签不会删除 Codeup 对应引用。
- 缺少凭据或推送失败会在 Actions 显示失败，不影响独立构建发布工作流。

## 验证

保存 secrets 后，打开 Actions → Sync Codeup → Run workflow。确认任务成功，并比较双方分支和标签引用。后续正常推送会自动触发。
