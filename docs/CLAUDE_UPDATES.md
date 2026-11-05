# Claude Code 检查与更新

服务器页的 Claude Code 行提供“检查更新”。检查仅查询已安装版本和官方通道版本，不下载或安装新程序，也不发送模型提示。

检测到新版后，当前账号的独立 native/npm 安装可点击“更新”，确认后按现有 `claude update` 更新通道执行。活动任务（含分析、排队、待审批）未结束时，App 后台显示等待；等待期间不取消任务，不持有 App 通用写锁。等待上限15分钟，连接变化取消尚未派发的更新。已开始的更新不因切页停止。

共享安装、Homebrew和未识别安装只提供检查与明确提示，不使用 sudo，不转装或改动其他账号的程序。个人 native 安装要求目标在账号 HOME 的 `.local/share/claude/versions` 下；npm 要求真实路径为账号 HOME 下的 `node_modules/@anthropic-ai/claude-code` 且属于该账号。

## 结果与恢复

- 更新前核对检测版本；版本已变化时要求重新检测。
- Runner复用账号调度器申请覆盖 HOME 的独占维护段，并再次检查活动任务，避免检查与启动之间抢占。CLI自己的更新器负责原子替换；CCBridge不自行覆盖程序或认证。
- 完成后运行同一路径的 `--version`，展示实际版本，并刷新模型/思考能力。版本未变化不冒称已升级。
- 网络断线或超时显示结果待确认，应先重新检测，不能盲目重发。
- 更新输出最多保留64KiB并持续排空；超时清理本次维护子进程组。错误不回显原始命令输出或设置内容。
- 新能力使用 `claude_update_v1`；旧Runner不会影响原会话，只禁用新更新入口。

## 来源与实现依据

官方说明：`https://code.claude.com/docs/en/setup`（通过官方文档检索核对）。`claude update` 沿用 `autoUpdatesChannel`；Homebrew应交由原包管理器升级。更新通道元信息来自官方安装器使用的 `https://downloads.claude.ai/claude-code-releases/{latest|stable}`。只提取用户设置与系统托管设置中的通道字段，不读取认证文件或写回配置。

## 本轮验收边界

用户要求“先不要更新”：本次只在私有假安装与独立QA App验收。不能把测试版本变化表述为真实服务器已更新；不部署Runner、不替换现用App、不重载MCP。
