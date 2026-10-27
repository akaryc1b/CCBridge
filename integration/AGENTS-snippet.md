# CCBridge 调用规则示例

本片段只规定已获用户授权的远端任务如何执行，不自行扩大到发布、数据删除、生产操作或其他项目。

1. 先 `connection_status` 核对连接、账号、目录与授权。全自动要求 `profile.allow_auto=true`；关闭时请用户在 App 选择，不替用户修改开关。
2. 多项目先 `project_list`，用准确路径 `remote_project` 路由任务；未指定项目时使用 App 当前工作区。只有明确需要匹配本地仓库时才用 `repository_match`。不同时传 remote_project 与 local_repository。
3. 获得全自动授权后，`cc_start` 显式传 `mode="automatic"`、新 UUID task_id 和完整任务范围。只读分析显式传 `access="analysis"`，开发传 `access="write"`；改代码前核对仓库、分支和未提交状态。账号已禁止AI修改时不能请求write，也不能通过MCP关闭保护。
4. 保存本会话 task_id、session_id、workspace、模式与目的。追问用 `cc_reply` 和新 UUID 续接本会话最近一轮，保持原目录、模式和分析范围，不取全局最新任务；同一session上一轮未结束时不再续发。
5. `cc_result(wait_seconds=900)` 等结果；未完成继续等待，不快速轮询、不把派发当完成。遇到审批与 automatic 模式不符时报告，不代点审批。
6. 响应不确定时查状态或用原 UUID 重试，不重复派发。不接管其他会话任务。支持analysis的Runner允许同一工作区多个独立分析session并行，沿用原Claude登录；分析读取实时文件，不保证快照一致。开发/开发冲突排队，Git及人工文件修改需等待活动任务释放，不绕锁。旧Runner不支持时先升级，不降级成可写来模拟分析。
7. 使用已注册 MCP。未加载时重新加载或新开会话；不通过手动启动 CC 或读取 Runner 状态文件冒充 MCP。
8. 用户要求更新远端代码时，优先 `git_update`，显式传项目/仓库远端路径，不用 Claude 自行拉取来代替 App 认证。可先 dry_run 检查并将返回的 repositories 作为 expected 传回；仅快进，脏仓库/分叉/活动任务先报告。没有认证时请用户在 App 配置或明确同步到 VPS，不通过 MCP 传令牌。保护开启时MCP实际更新会被拒绝，请用户在App手动更新，不绕过保护。更新本地代码用本机 Git；未提交改动不会同步，VPS 拉取的是已推送提交。
