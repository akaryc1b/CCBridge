# CCBridge 开发约束

## UI、状态与文案

所有页面、弹窗、通知及异步交互遵循 [docs/UI_GUIDELINES.md](docs/UI_GUIDELINES.md)。这是唯一正文；修改前阅读完整相关章节及直接调用链，不通过复制规则另建标准。

- 保留用户已授权的全自动默认和续聊模式，不静默降级、不代改安全开关。
- 状态由真实后端结果驱动；区分已保存、已连接和访问通过，禁止假百分比、静默失败和重复结果文案。
- 界面复用既有 shadcn/Radix、Lucide及统一亮暗token，不为新增功能自行造组件系统。
- 先提交关键页面/弹窗亮暗预览，用户确认后再实施功能。实现后按规范核对真实App主路径、异常、状态恢复及亮暗视觉，不以编译通过替代验收。
- 本轮Claude仅做分析；由Codex开发、编译、实机验收。禁止让Claude编译或实施代码。

## 发布与保护

构建发布遵循 CONTRIBUTING.md。现用App的活动任务、账号凭据与数据库配置不得因UI整改受损；不使用Developer ID。验收通过后更新现用App并发布GitHub：ARM先发布，x86随后追加，不等待Intel才发布ARM。

## 后续 Windows 支持

Windows 已纳入后续交付范围，具体平台计划见 [docs/WINDOWS_SUPPORT.md](docs/WINDOWS_SUPPORT.md)。当前 Mac 整改与发布继续推进，不以 Windows 为本轮发布前置条件。新增共享代码不得无条件依赖 macOS 路径、钥匙串或 Unix 命令；平台差异收敛到现有边界，共享核心与界面，不另建一套产品。
