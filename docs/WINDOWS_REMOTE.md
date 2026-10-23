# Windows 原生远端开发与验收方案

状态：开发中。基线为 main `4e3bdb0`（0.10.8）。本方案是 Mac 客户端连接 Windows 远端，不是 Windows 桌面客户端；后者见 WINDOWS_SUPPORT.md。

## 使用路径

Windows 启用系统 OpenSSH Server → 在 Mac App 保存 SSH 连接 → 检测 Windows / 当前用户 / Claude / Git → 安装原生 Runner → 浏览项目、管理文件、操作 Git。未安装 Claude 时连接和文件功能仍可用；创建 Claude 任务提示安装，不能将 SSH 标为失败。首次测试机不装 Claude。

首轮原生目标为 Windows x64。Windows 11 与 Windows Server 2022 分别验证；其他架构不冒充可安装。保持 Linux、macOS 现有协议和用户目录约束，不修改全自动授权、本机密码保存及其他会话。

## 1. 平台与 SSH 引导

- `remote_platform.rs` 集中描述平台、绝对路径、路径拼接、PowerShell 编码和固定引导脚本。Mac 不能用本机 Path 解析 Windows 路径。
- 保留现有 POSIX 探测；失败后尝试固定 PowerShell 探测。PowerShell 使用 NoProfile / NonInteractive / EncodedCommand，不更改 Windows OpenSSH 默认 shell。
- 协议输出使用 UTF-8 JSON，不依赖系统代码页。路径和业务参数通过 JSON stdin；凭据不放命令行。
- 平台探测成功但 Runner / Claude 缺失是独立状态。未知平台明确拒绝安装。
- Windows Runner 存储在当前用户 LOCALAPPDATA/CCBridge；任务状态与配置按账号隔离。首先支持用户目录内项目，与现有 Linux 合同一致；额外磁盘项目根目录须另有明确授权，不静默开放整盘。

## 2. 平台基础能力

- `platform.rs` 收敛 HOME、私有目录、原子替换、排他移动、注册表文件锁和进程树。Unix 原实现保留。
- Windows 使用当前进程令牌 SID 与 SYSTEM 的保护 DACL；拒绝重解析点及不属于当前用户的凭据目录。不能用 chmod 或普通隐藏属性代替 ACL。
- 用 Windows 原子替换保存 JSON；移动默认不覆盖，跨盘失败保留原文件。文件名拒绝设备名、冒号、尾随点/空格。
- 任务注册锁使用跨平台 File::lock；同一目录不并发写任务。

## 3. Runner 生命周期

- `runner.rs` 继续复用 JSON RPC、任务 UUID、模型解析、审批 MCP、续聊会话及数据库快照，不另建任务状态机。
- Windows Claude 子进程使用 process-wrap JobObject，挂起创建、分配 Job 后恢复，取消仅影响本任务进程树。
- CI 已发现父 Job 禁止 breakaway，普通 detached spawn 返回访问拒绝。因此 Windows Worker 通过本机 WMI Win32_Process 创建，保留当前用户环境，并在执行前校验账号 SID；不安装系统服务，不使用 SYSTEM 任务，不保存或索取额外密码。需要该账号具备本地 WMI 方法执行权限；受策略限制时明确报错。
- Worker 不属于 SSH 的子进程树；真实 OpenSSH 仍需验证断线后存活。Claude 子树单独属于 Worker 的 Job，取消不能影响其他任务。
- Claude 未安装不能派发假任务；测试 fixture 明确标记模拟执行，不能当真实模型验收。

## 4. 文件、Git 与认证

- 将 Windows 文件列表、预览、新建目录路由到 Runner 原生 RPC；保留 Linux SSH shell 入口，避免改变旧 Runner 行为。
- 路径需 canonicalize 后仍在账号范围；拒绝 junction / symlink 逃逸，文件管理保护运行配置、Git 元数据和凭据。
- 复用 workspace_ops 的重命名、移动、软删除、恢复与分支操作；增加 Windows 名称校验与排他移动。
- clone / bundle 导入 / fetch / ls-remote 使用原生 Git argv，不拼 cmd 脚本；Git 关闭交互并保留输出上限及真实进度。失败不覆盖已有目录。
- 保留本机 Git 认证复用。同步到 Windows 时按当前用户保存，先在 App 明确同意；认证检测不要求仓库地址，仓库访问检测才要求地址。用户令牌只走加密 SSH stdin，不进入日志、RPC 返回或 argv。
- 数据库映射与只读保护继续复用；Windows Node 与安装路径单独探测，不读取或迁移真实数据库凭据。

## 5. 安装与版本切换

Windows 正在执行的 exe 不能沿用 Unix 覆盖安装方式。按 App 版本选择 Runner 版本目录，上传到临时文件、校验 --version 后排他移动到版本路径；现有任务继续使用其启动版本，不删除正在使用的版本。新 App 首次连接旧服务器时可提示安装本版 Runner，安装完成后继续读取同一账号的任务状态。重复安装当前版返回“已是最新”。上传、校验、切换均提供真实阶段，失败保留旧版。

Mac 发布仍 ARM 先发、Intel 后追加。Windows Runner 是独立构建产物，不在本轮建立 Windows GUI 安装包，不改变现有发布权限或签名。

## 6. UI 与状态

复用服务器页、文件页、认证弹窗和操作进度，不新增同义入口。显示 Windows 与当前账号；缺 Git 只阻止 Git 操作，缺 Claude 只阻止模型任务。失败给具体原因和对应重试操作。不增加常驻教程或伪百分比。新布局若需要变更，先给亮暗预览；平台适配优先沿用已审核组件。

## 7. 验收顺序与完成条件

1. 本机共享核心测试、Mac bridge 编译；隔离 Linux Runner 构建及既有烟测，不能复用正在运行的用户任务。
2. Windows CI 原生编译、RPC probe、临时目录文件/分支/删除恢复、账号 ACL、重复写入、任务取消与两轮 fixture。
3. 用户局域网 Windows：原生 OpenSSH 登录、指纹确认、Runner 安装与重复安装、中文/空格路径、Git 更新、断线重连。该机不安装 Claude，界面应正确显示缺失。
4. 以后 Windows VPS 安装登录 Claude，再做真实全自动、审批、续聊、实际模型、模型列表与用量；未验收前记录 NOT_RUN，不套用 Linux 结果。
5. 审查完整差异、更新构建说明和测试记录后，才进行正式版本交付；现用 App 的替换需要无活动任务窗口。

当前依赖：Windows SSH 连接信息由用户在 App 输入。原生 Claude 验收依赖未来 VPS，不能阻塞无 Claude 的远端基础能力开发。
