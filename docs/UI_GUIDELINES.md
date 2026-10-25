# CCBridge UI、状态与文案开发规范

**生效日期：2026年10月21日 · 现状分析基线：0.8.6 / `0404d3c`**

本文件是全项目 UI、交互状态与文案的唯一开发规范。用户已确认规范及v1亮暗预览，当前按此实施；实际验收状态见UI_RETROFIT.md。后文现状行号属于分析基线，实施时需复核；代码片段为设计，不表示能力已经实现。

## 适用范围与实施边界

推荐先修全局样式与状态基础，再逐页收紧布局，而不是继续逐个弹窗堆补丁。优先处理：边框和亮暗 token、长操作真实进度、认证状态与按钮层级。

- **全自动授权保留**：当前用户已明确选择全自动，不将新任务默认改回审批。界面清楚显示实际模式；账号未授权时仍按现有安全合同阻止全自动，不静默降级或替用户改开关。
- **易读优先**：正文14px、主要内容14–15px、辅助信息13px、非关键信息最低12px；通过删冗文、缩短行和调整布局获得紧凑感，不通过小字塞内容。
- **状态就近且连续**：连接断开影响当前操作时必须在当前上下文可见；对象错误归对象，不能切页后到处重复。无需为普通保存、MCP写配置和分支切换机械新增确认。
- **关闭不丢现场**：只有后端操作可被可靠识别、持续查询且结果可恢复后，才开放长操作后台关闭。未知结果必须先查询，不能盲目重发。
- **进度有真实依据**：整体显示“已处理 N/M”（不等于成功N项），单项显示阶段和耗时；对象进度必须标为“对象”，不冒充字节百分比或整项完成度。
- **仍保留必要入口**：克隆前可主动配置认证，失败时也能就近修复。保留侧栏亮暗快捷选择，设置中的同一选项共享状态；不新增第二个应用更新入口。
- **规范按含义检查**：不以字数、颜色字面量或checkbox数量机械判错；长路径、模型标识、准确错误及真实业务选项可合理例外。

覆盖工作台、文件、会话、数据库、服务器、Codex/MCP、更新、设置和所有共用弹窗。规范改变不扩大账号权限或任务授权。

---

## 0. 现状与关键原因

1. **「粗黑大框」的主要原因在全局样式。**
   - 项目使用 Tailwind 4.3.3。Tailwind v4 中，`border` 工具类不写颜色时，默认颜色是 `currentColor`，也就是文字颜色。
   - `index.css` 只有 `* { box-sizing }`（第 37 行），没有给 `*` 设置 `border-color: var(--color-border)` 的基础规则。
   - 结果，下面这些凡是只写了 `border`、没写颜色的地方，在亮色模式下都会画成黑框：
     - `GitUpdate.tsx:89,108`、`CodeupAuth.tsx:264,355`、`WorkspaceTools.tsx:239,384` 的 `rounded border`
     - `DialogContent`（`ui/dialog.tsx:62`）
     - `Button` 的 `outline` 变体（`ui/button.tsx:14–15`，`border` 后面只在 dark 下有 `dark:border-input`）
     - `SelectContent`
   - 只有 Card 例外：它被 `[data-slot="card"] { border-color: var(--line) }`（约第 1025 行）单独修正过。
   - **所以先补一条全局规则，比逐个组件改样式更重要。**
2. **「请勿重复提交」不是唯一的防重机制。** 实际上有两层：
   - 前端 `request()` 对同名命令正在执行时会直接丢弃新请求（`App.tsx:330–334`）。
   - 后端所有写操作共用一把 `writes.try_lock` 锁（`backend.rs` 中的 writing 列表，包含 `git_update`、`clone_into_directory`、`git_vps_auth_install` 等），拿不到锁会返回「另一项写入操作仍在执行…」。
   - 真正的问题是：前端丢弃请求时**什么都不显示**；而且 GitUpdate 和 CodeupAuth 直接调用 `invoke`，没有走 `request()`，界面上看不到「谁正在进行」。
3. **Git 更新当前拿不到逐仓库进度。** `git_update` 是一个同步 RPC（`backend.rs` 中 `git_update_plan`/`git_update` 分支），在一个循环里依次执行 preflight → `git_fetch_remote` → apply，全部结束后才返回 `{ok, updated, failed, results}`。另外：
   - `git_fetch_remote`（`ssh.rs:459`）执行的是 `git fetch -- <remote>`，没有 `--progress`。
   - `execute_reader` 把 stderr 全部收集完才返回，所以没有字节或对象数。
   - 唯一已有的进度通道是克隆用的 `clone_progress`：一个全局 Mutex 存 `{stage, completed, total}`，前端每 1 秒轮询一次。
   - **结论**：要显示「当前仓库 / 阶段 / N/M / 耗时」，后端必须改；要显示「接收对象 / 字节」，还必须给 fetch 加 `--progress` 并流式读取 stderr。现在不能假装有这些数据。
4. **VPS 凭据和 App 服务器档之间有优先级。** `remote_git_credential`（`ssh.rs:436–452`）的逻辑是：VPS 上已经配置凭据时返回 `None`，服务器端 Git 直接使用 VPS 上的凭据，App 保存的「服务器档」会被忽略。这一点直接影响用户判断「改哪份凭据才有用」，**必须显示出来**。现在弹窗里没有任何地方提示。
5. **VPS 写入的确认在后端是硬性要求。** `vps_auth.rs:92`（`server_storage_ack != true` 时拒绝）和 `backend.rs` 中的 `vps_account` 一致性校验都会拒绝未确认或账号不一致的请求。
   - 改版时**只改确认的形式**：把常驻 checkbox 换成「点击后弹出确认框，在确认框里按下确认」。确认仍然只对这一次操作有效，不记住。
6. **新任务当前默认是全自动。** 这是本用户已经明确选择的使用方式；本次保持该行为，并使新建、续聊和账号授权的模式展示一致。账号未授权的全自动请求仍必须被拒绝。
7. **MCP 兼容约束**：
   - `mcp.rs:119–123` 的 `git_update` 工具，`dry_run` 时调用 plan，否则调用 update；返回结构 `{ok, updated, failed, results[{path, branch, state, message, before, after}]}` 必须保持不变，只允许新增字段。
   - 非人工调用时，必须 `allow_auto` 才能执行（`backend.rs` 中 `!human && op=="git_update"` 的判断）。
   - ff-only 的语义由 `git_update.rs` 保证（dirty、无上游、分叉都拒绝；`merge --ff-only`），不得改动。

---

## 1. 项目级规范

### 1.1 信息必要性判定

一段文字或一个元素，**只有满足下面至少一条时**才允许出现在界面上：

1. **状态**：告诉用户「现在是什么」。比如已保存、未检测、2/3、失败原因。
2. **决策依据**：用户如果不知道这条信息，可能会做错选择。比如服务器 Git 当前使用的是 VPS 凭据，或者某个操作会写入哪个账号的哪个位置。
3. **授权点的后果**：在破坏性操作或写入凭据的确认框里，写清楚具体对象和后果。
4. **下一步动作**：错误信息里附带「去哪里修」的按钮。

以下内容**禁止出现**：

- 产品自我保证清单（「不会…不会…不会…」）。这些保证应该由按钮禁用、确认框、错误码来体现。
- 同一事实重复两次（标题和描述、徽标和正文、页面内和页面外）。
- 把解释性的话收进 `<details>` 或 tooltip 里。折叠只允许用来放**原始数据**，比如日志、JSON、诊断信息。
- 「请勿重复提交」「请稍候」这类提示。应该用禁用按钮加进度状态来表达。
- 「已就绪」「可用」这类笼统说法，用来描述其实只是「配置存在」的情况。

### 1.2 Token（单一来源）

`index.css` 整合为语义token的单一来源；迁移前核对现有组件引用，不直接整块删除仍使用的映射。最终只保留**一组** `:root` 和 `:root.dark` 变量，以当前已确认的 CC Switch 风格中性色板为准（约第 905–935 行）。删除前面旧的蓝色系 `@theme` 硬编码 hex（第 2–24 行），以及旧规则里的硬编码颜色（目前全文件有 126 处 hex）。

| Token | 亮色 | 暗色 | 用途 |
|---|---|---|---|
| `--background` | `#fff` | `#1c1c1e` | 页面、弹窗底色 |
| `--side` / `--soft` | `#f5f5f7` | `#161618` / `#252528` | 侧栏、hover、分段控件底色 |
| `--selection` | `#e8e8ed` | `#313136` | 选中行 |
| `--line` | `#e6e6eb` | `#34343a` | 普通边框与分隔线；输入框、焦点和错误使用对应语义token |
| `--strong` | `#d2d2da` | `#4a4a52` | 输入框边框、强分隔 |
| `--muted-text` | `#73737c` | `#9696a1` | 次要文字、路径、时间 |
| `--primary` | `#c24b0b` | `#e39962` | 主按钮、焦点、进度条 |
| `--success` | `#1f8a5b` | `#5fbf8f` | 新增：通过、完成 |
| `--warning` | `#9a6200`，底色 `#fff6e0` | `#e0b45a`，底色 `#33290f` | 新增：跳过、已过期、待检测 |
| `--destructive` | `#c73845` | `#ef6b74` | 失败、破坏性操作（需要在 dark 下补一个值） |

- **字号**：正文14、主要内容14–15、辅助信息13、非关键信息最低12、分区标题15–16、页面标题20。长文和对话不得压缩为11px。
- **间距**：采用 4 的倍数，常用 4 / 8 / 12 / 16 / 20。页面内边距 20，section 之间 16，行内间距 8。
- **圆角**：控件 6，面板 10，对话框 14。
- **控件高度**：按钮、输入框 32，紧凑型 28，列表行 36（单行）或 48（两行）。
- **边框**：一律 1px `var(--line)`，**不使用粗边框，不给内容卡片加阴影**。阴影只用于浮层（菜单、Dialog）。
- **必须补充的全局规则**：
  ```css
  @layer base { *, ::before, ::after { border-color: var(--color-border); } }
  ```
  `--color-border` 由 `@theme inline` 映射到 `var(--line)`。另外 `Button` 的 outline 变体要改为 `border-border`。

### 1.3 布局与层级

- **页面结构**：52px 页头（标题 + 右侧操作）→ 内容区。内容区由 `Section` 组成：15px 标题，下面是若干 `Row`，用 1px 分隔线隔开，**不再一个 Panel 一张大卡片**。
- **列表**：统一使用行式列表（36/48px），次要操作在 hover 或聚焦时显示 `⋯`，状态放在行尾。
- **Dialog 尺寸**（只用这几档）：
  - `sm` 420px：确认、单字段输入
  - `md` 560px：表单。例如 Codeup 认证、SSH 服务器、克隆、文件操作
  - `lg` 720px：多对象列表。例如 Git 更新、回收区、分支
  - `xl` 880px：文件预览
  - 所有档位 `max-h: 80vh`，内容区单独滚动，Header 和 Footer 固定。
- **Dialog 结构**：Header（标题 + 一行对象标识，可省略）→ Body → Footer（右对齐，**只放一个主按钮**，放在最右）。破坏性按钮使用 destructive 变体，按钮文字里写出对象。
- **按钮层级**：每个页面 section 或每个弹窗最多 1 个 `default`（primary）；次要操作用 `outline`；低频操作用 `ghost` 或放进 `⋯` 菜单；破坏性操作用 `destructive`，而且只出现在确认步骤里。

### 1.4 组件（复用 shadcn / radix-ui，不引入新平台）

现有组件：`button`、`badge`、`card`、`dialog`、`input`、`label`、`progress`、`select`、`switch`、`textarea`，以及 radix 的 `DropdownMenu`、`ContextMenu`。

需要补充的组件（`radix-ui` 依赖里已经包含相应原语）：

| 组件 | 说明 |
|---|---|
| `ui/alert-dialog.tsx` | 用于破坏性操作和授权确认 |
| `ui/toggle-group.tsx` | 分段控件。用于 Codeup 的 本机/服务器 切换、数据库的 连接/映射 切换、主题切换 |
| `ui/tooltip.tsx` | 图标按钮的名称、禁用原因 |
| `common/StatusRow.tsx` | `label | 状态 chip + 次要说明 | 操作`，用来表达「配置 / 检测 / 访问」等状态 |
| `common/OperationBar.tsx` | 长操作的统一进度条与摘要 |
| `common/InlineAlert.tsx` | 就近显示的提示：info / warning / destructive，可带一个操作按钮 |
| `common/EmptyState.tsx` | 一句标题 + 一个操作 |

### 1.5 状态模型

**A. 三类状态必须分开显示，不能互相推导：**

| 状态类型 | 允许的取值 |
|---|---|
| 配置 | 未配置 / 已保存 · 用户名 / 读取失败 |
| 连接 | 未连接 / 连接中 / 已连接 / 已断开 |
| 访问检测 | 未检测 / 检测中 / 通过 · 时间 / 失败 · 原因 |

- 「已保存」**不能**显示成「可用」。
- 「检测通过」只针对**那一个仓库地址**、**那一个时间点**；换一个仓库地址后，结果自动变回「未检测」。

**B. 异步操作统一使用一个状态机：**

```
idle → checking → ready(可执行 n / 跳过 m)
     → running(total, completed, current{name, phase, since, objects?, bytes?})
     → done | partial | failed | interrupted | unknown
```

- `partial`：至少有一项成功、至少有一项失败。
- `interrupted`：SSH 断线或 App 重启导致中断。
- `unknown`：请求已经发出，但结果无法确认（例如 SSH 超时）。显示为「结果未确认 · 刷新状态」，**不提供盲目重试**。

**C. 陈旧数据**：

- 检查结果带 `checked_at`。相关文件、分支、凭据、账号或连接世代变化后，按对象失效并显示「需要重新检测」。不以统一两分钟阈值制造无意义的过期提示；每次写入前仍由后端核实实际前置条件。
- 后端返回「已变化，请刷新」这类错误时，统一显示为 warning 提示，并附带「刷新」按钮。

### 1.6 进度与耗时

- **任何可感知的等待都立即有反馈**：短操作使用按钮忙碌态；长操作显示当前对象、阶段和耗时，批量操作再显示已处理 N/M。不要给瞬间保存额外挂一排计时、阶段和百分比。
- **百分比**只在分子分母都来自真实数据时才显示：
  - 下载更新：`downloaded/total`，且 `total` 存在
  - 上传 bundle：已读字节数 / 文件大小
  - Git 报告的 `Receiving objects (x/y)`
  - 不满足条件时，只显示已收字节数，或者显示不确定进度条加阶段文字。**禁止**用 `animate-pulse` 的假进度条来冒充百分比（现在 `App.tsx:2518–2524` 就是这样）。
- 批量操作的整体进度条只按「已完成项数 / 总项数」计算。

### 1.7 关闭、取消、防重

- **关闭 ≠ 取消。**
  - 如果后端操作会继续执行（Git 更新、克隆、VPS 写入），弹窗应当**允许关闭**。关闭后，页头显示一个后台状态 chip，例如「Git 更新 2/3」，点击可以重新打开。
  - 只有后端真正支持取消时，才显示「取消」或「停止」按钮。目前只有 `task_cancel` 支持取消。
- **短操作**（预计 5 秒以内，比如保存）：弹窗保持模态，主按钮内显示转圈，其他按钮禁用。
- **防重**：「是否正在执行」以**后端状态快照**为准（见 §4），主按钮显示为「进行中…」并禁用。
  - 前端 `request()` 静默丢弃请求的行为要改：当同一命令正在执行时，返回 `{inflight:true}`，调用方据此显示状态。
  - 后端写锁报错时，显示「正在执行：<操作名>」，并提供「查看」入口。

### 1.8 确认与风险

- **确认的触发**：只在授权点出现，通过点击后弹出的 AlertDialog 完成；**不使用常驻 checkbox**。每次打开确认框，状态都重新开始，**禁止**默认勾选或记住上次的同意。
- **确认文案的结构**：对象（账号或路径）+ 后果（写入什么、谁能读、能否恢复）+ 一个动词按钮，按钮文字比如「写入 VPS」「移入回收区」。
- **「本机明文保存」的一次性同意**（后端 `secret_store::confirm_consent`）：在第一次保存时弹出确认框；之后不再询问，但要在 设置 › 凭据 中显示「存储：本机文件」。

### 1.9 文案规则

- **表达**：标题说对象或动作；说明只补充会改变选择的信息。错误给原因和恢复动作，长标识保持完整可复制。不设置硬性字数上限。
- **标准词表**：
  - 状态：未检测、检测中、通过、失败、已保存、未保存、已写入、未写入、等待、进行中、已完成、部分完成、已中断、结果未确认、跳过、已是最新、已快进、本地领先
  - 动作：检测、保存、写入 VPS、移除、刷新、重试失败项、快进更新 N 个仓库
- **禁用词**：「请勿重复提交」「请稍候」「一目了然」「开源版」「无需…」「不会…（作为常驻说明）」「已就绪」（用来描述配置存在时）。
- **时间**：同一天内显示 `HH:mm`，其他日期显示 `M月D日 HH:mm`；耗时显示 `m:ss`。
- **路径**：主视图显示有辨识度的名称和父级；完整路径可复制，歧义时可查看。高风险确认框必须直接显示目标路径，不能只藏在 tooltip。

### 1.10 错误、空、加载与 toast

- **错误**：就近显示（行内、弹窗内）。全局 error-box 只用于连接级别的错误。同一条错误不同时以 toast 和行内两种形式出现。
  - 现在 `backend-event` 写入的 `errors.backend` 没有 `page` 字段，会在所有页面显示（`App.tsx:664–666` 和 `:798–808`）。要把它归属到「连接」类错误。
- **空状态**：一句话 + 一个主操作，不放插画和口号。
- **加载**：首次加载显示骨架行；刷新时保留旧数据，只在刷新按钮上转圈。
- **toast**：只用于「后台完成的结果」和「可撤销的操作」（比如移入回收区后的撤销）。成功 toast 一行，不重复面板里已经显示的状态。

### 1.11 MCP 与安全兼容（不可破坏）

- `git_update` 返回结构只能新增字段（`code`、`elapsed_ms`、`skipped`），`state` 取值保持 `updated / unchanged / ahead / failed`。被阻止的仓库**继续**使用 `state:"failed"`，另外加 `code:"blocked_*"`，UI 显示为「跳过」。
- 不新增 MCP 写凭据、改分支等能力。「需要人工操作」的分支（`!human` 拒绝）保持原样。
- ff-only、dirty 检查、`expected` 快照校验、`protected_running` 都不改。

---

## 2. 逐页、逐弹窗整改清单

> **P0**：影响正确性、安全或可用性，或者全局视觉 bug。**P1**：降噪和一致性。
> 每条格式为「文件 → 问题 → 改法」。

### 2.0 全局（`ui/src/index.css`、`components/ui/*`、`App.tsx`）

- **P0** `index.css` 缺少基础 border-color → 补 `@layer base` 规则；`ui/button.tsx:14` 的 outline 变体改为 `border-border bg-background`。
- **P0** `index.css` 中两套 token 并存（第 2–24 行是旧蓝色系 hex，约第 905 行起是新色板）→ 删除旧值，补齐 `--success`、`--warning`，以及 dark 下的 `--destructive`。
- **P0** `App.tsx:330–334` 同名请求静默丢弃 → 返回 inflight 标记，由调用方显示「进行中」。
- **P0** `App.tsx:664–666` `errors.backend` 没有 page 归属，所有页面都显示 → 归入连接错误，侧栏持续表达连接状态，在当前受影响操作旁提供一次恢复入口，服务器页保留详细原因。
- **P1** `App.tsx:854–876`：保留侧栏亮暗快捷入口，设置中的外观选项复用同一分段组件和持久状态；不复制说明，不发生两处不同步。
- **P1** `App.tsx:1000–1019` 未连接提示「先连接服务器…」加「前往连接」→ 改为 EmptyState，按钮直接执行连接（例如「连接 cyb@host」），而不是跳转页面。
- **P1** `App.tsx:984–998` 安装位置横幅的文字和按钮用了中英混排（「Applications」）→ 统一用中文（见 §3）。

### 2.1 工作台（`components/Workbench.tsx`）

- **P1** 「快捷操作 · 克隆仓库」（第 170–173 行）只是跳转到项目页，名不副实 → 改为直接打开克隆弹窗，或者删除。
- **P1** 「运行环境」中 Runner 只显示版本号，没有和 App 版本比较；Claude 只要有版本号就显示「已就绪」（第 146–150 行）→ Runner 显示版本与「需更新」或「已安装」；Claude 显示版本号，或者「未检测到 · 原因」。
- **P1** 「最近会话」的时间只有日期，没有时刻（第 125–127 行）→ 按 §1.9 的时间格式。
- **P1** 新增「需要处理」一节：待审批、失败、Runner 版本不一致、后台操作进行中（例如 Git 更新）。没有内容时整节隐藏。

### 2.2 项目与文件（`App.tsx:889–977, 1254–1413`）

- **P1** 页头一共 5 个按钮（Git 更新、分支、新建、克隆仓库、⋯），其中「克隆仓库」是 primary → 保留「克隆仓库」为主按钮；「Git 更新」在后台运行时显示进度 chip「Git 更新 2/3」。
- **P1** 文件状态栏「N项 · 服务器文件」（第 1406–1410 行）中的「服务器文件」是无效信息 → 删除，只保留项数和截断提示。
- **P1** 空列表「正在打开…」→ 改为骨架行；「选择项目或输入目录」保留。
- **P1** 「设为MCP默认项目」（第 961 行）执行后没有任何反馈 → 成功后 toast「已设为 MCP 默认项目」，并在项目行显示徽标（项目行已有）。

### 2.3 文件预览（`App.tsx:2570–2577`）

- **P1** 标题只有「文件预览」，看不出是哪个文件；返回值不是文本时会显示原始 JSON（`:536–539`）→ 标题用文件名，元数据可得时显示大小；二进制文件显示「无法预览二进制文件」；使用 `xl` 尺寸。

### 2.4 新建、重命名、移动、移入回收区（`WorkspaceTools.tsx:113–309`）

- **P1** 第 278–280 行「进行中的任务、已注册项目根目录和当前工作区受到保护。」在每种操作里都显示 → 删除；被保护时由后端报错就近显示。
- **P1** 第 269 行「不会覆盖同名目标；不能移动到自身或子目录。」→ 删除；目录选择器里禁用自身和子目录。
- **P1** 第 264 行 `📁` emoji → 换成 `Folder` 图标；目录选择器使用 `lg` 列表行样式。
- **P0** 第 272–276 行移入回收区的确认文字太长 → 按 §3 改短，但必须保留对象名，文件夹还要说明「包含全部内容」。
- **P1** 第 171–172 行刷新失败时的 toast 文案 → 按 §3 改写。

### 2.5 回收区（`WorkspaceTools.tsx:310–400`）

- **P1** 打开时已经自动加载，又放了一个「加载 / 刷新」大按钮 → 改为页头的图标按钮。
- **P1** 列表行用了 `rounded border p-3` 小卡片 → 改为行式列表，增加「删除时间」（后端已返回 `deleted_at`）。
- **P1** 描述「恢复到原路径，不覆盖已有同名文件。这里不提供永久删除。」→ 删除；遇到冲突时在行内报错。

### 2.6 分支（`WorkspaceTools.tsx:401–614`）

- **P1** 描述「{path} · 请先打开一个 Git 仓库根目录」在仓库内也会显示 → 只显示仓库名。
- **P1** 「读取 / 刷新分支」「获取远端分支（fetch）」两个大按钮 → 一个刷新图标，加一个「获取远端」的 ghost 按钮；获取远端时也接入 OperationBar（同样用 fetch 进度）。
- **P1** 第 558–561 行的常驻保证说明 → 删除；需要说明时，写在确认那一行：「将 main 快进到 origin/main」。
- **P0** `confirm` 状态目前只在点击「快进同步到…」时设置，主按钮会直接执行 `change()` → 确认流程统一：快进、新建、切换都在按钮文字里写清对象；保留现有快进确认，不因普通切分支机械增加确认；dirty/活动任务仍由后端阻止。

### 2.7 克隆（`App.tsx:2470–2569`）

- **P0** 假进度条（`:2518–2524`，`w-1/3 animate-pulse`）→ 换成 OperationBar：整体按「仓库 N/M」，单个仓库显示阶段文字和耗时；本机获取时，上传 bundle 显示真实字节和百分比（§4.3）。
- **P0** 「正在执行，请勿重复提交；…」（`:2541–2543`）→ 删除；钥匙串提示只在后端确实返回等待系统授权时显示，不能把所有凭据助手耗时猜成钥匙串弹窗。
- **P1** 克隆前保留紧凑的「认证」次要入口，显示当前有效凭据来源，不强迫先失败。认证失败时显示行内「修复认证」，预填该失败仓库的 URL 和获取方式。已有保存后重试机制保留，但写入成功不直接等同访问成功。
- **P1** 「获取方式」的 Select → 改为分段控件「本机获取 / 服务器获取」。
- **P1** 逐仓库结果（`:2547–2552`）→ 合并进 OperationBar 下方的行列表：已克隆 / 已存在 / 失败（原因）。
- **P1** 克隆进行中允许关闭弹窗（后端会继续），页头显示 chip。

### 2.8 Codeup 认证（`components/CodeupAuth.tsx`）

- 本机与服务器页签互斥展示。本机不读取或展示服务器区；服务器页显示当前SSH账号。
- 默认只显示认证来源/配置状态和仓库访问；已有App认证或Git助手时不常驻输入框。“更换认证”或本机缺少配置时才出现用户名与令牌。
- 打开时自动检查配置，状态行的刷新按钮不需要仓库URL；仓库访问输入与检测放在一起，有URL才能检测。助手已配置、App已保存、VPS已同步均不等于仓库访问通过。
- 保存与同步目标明确：本机表单“保存到 App”，服务器表单“同步到服务器”。服务器未同步时提供“同步本机认证”；已同步后的更换、重新同步和移除放入⋯菜单。
- 不静默改变认证身份；同步确认必须显示明确来源。原有App服务器档的保存、迁移、移除继续提供；VPS已同步且存在旧档时指出服务器认证优先，仅有旧档时如实显示。
- VPS同步每次点击后确认目标账号、来源、私有文件路径和可读范围；`server_storage_ack:true`只能由当次确认发出。本机保存遵循一次性确认。
- 检测结果绑定scope、URL、账号、连接世代与已知配置身份；切换对象、修改输入或写入认证后失效。配置刷新不清除未保存输入，实际配置变化时不保留旧通过状态。
- 失败原因就近显示，不重复toast；认证失败提供更换入口，网络/地址问题不误导用户重新输入令牌。

### 2.9 Git 更新（`components/GitUpdate.tsx`，整体重写，配合后端）

**P0**：

- 进度：按 §4 的方案显示「当前仓库 / 阶段 / N/M / 耗时」，有对象或字节数据时再显示。
- 关闭语义：运行中允许关闭（后端继续执行），页头显示 chip；重新打开时显示实时进度或最终结果。
- 结果：每个仓库一行状态，被阻止的显示为「跳过」，而不是失败。
- 认证失败：现在靠正则匹配 `/认证|Codeup|令牌|凭据/` 来判断（第 68–70 行）→ 改为按后端返回的 `code:"auth"` 判断，就近在**那一行**显示「修复认证」，并带上**那个仓库**的 URL。现在用的是 `plan.find(...codeup)` 找到的第一个仓库 URL（第 133 行），这是错误的。
- 部分失败后的重试：「重试失败项」需要后端新增可选参数 `only:[path]`。不影响 MCP，因为不传该参数时行为不变。

**P1**：

- 删除第 85–87 行的安全说明段；主按钮写成「快进更新 2 个仓库」，ff-only 的含义体现在按钮上。
- 三个大卡片 → 行式表格。

### 2.10 会话（`App.tsx:1414–1840`）

**P0**：

- 新草稿保持用户已授权的全自动默认；实际模式以明确文字显示。续聊保留当前会话模式；账号关闭全自动后显示不可发送原因和可选审批方式，不静默改变既有会话或安全开关。
- 审批卡片（`:1648–1689`）目前直接显示原始 JSON → 按工具类型显示：Bash 显示命令，Edit/Write 显示路径，Read 显示路径；原始参数放进折叠（属于原始数据，允许折叠）；按钮「允许」「拒绝」。
- 第1594行说明改为「执行中 · m:ss」。任务耗时必须从稳定的开始时间起算，不能从不断变化的 updated_at 起算；历史缺失开始时间则不伪造耗时。

**P1**：

- 模型显示：`displayModel` 中的「实际模型尚未上报」「远端默认模型」（`lib/conversation.ts:38–39`）→ 改为「请求：x · 实际：y / 等待上报 / 未上报」。
- 运行记录嵌套了两层 `<details>`（`:1611–1623`）→ 一层「N 个步骤」，每个步骤一行（工具名 + 目标），点击展开原始 JSON。
- 会话行时间只显示日期（`:1488–1490`）→ 按 §1.9 的时间格式。
- 第 1523–1533 行用了临时 Tailwind 类 `px-5 py-2 text-sm` 的数据库快照行 → 改为对话头里的 chip「数据库 2」。
- 第 1803 行「Shift ↵ 换行」和第 1833 行「发送 ⌘ ↵」→ 只保留按钮里的快捷键提示。
- 执行失败（`:1598–1610`）的诊断详情属于原始数据，允许折叠；但摘要不要再写「请展开…」。

### 2.11 数据库（`components/Databases.tsx`，只改 UI，不改业务）

- **P1** 「确认删除配置」按钮用的是默认 primary 样式（约第 432–442 行）→ 改为 destructive。
- **P0** 「检查 MCP 握手」和「测试数据库连接」的结果拼在一个字符串里（第 249–269 行）→ 两个独立的 StatusRow，各自带时间；**配置存在 ≠ 握手通过 ≠ 数据库连通**。
- **P1** 第 134–135、190、200、293、368–369、465–466、516–517、535 行的说明 → 按 §3 处理。
- **P1** 连接行使用 `rounded-lg border p-4` 卡片，每行常驻 4 个按钮 → 改为行式列表：「测试」用 outline，「编辑」「删除」放进 ⋯ 菜单。
- **P1** 「连接 / 项目映射」两个用 Button 模拟的标签 → 改为分段控件。

### 2.12 服务器（`App.tsx:1060–1253` 及服务器编辑弹窗 `:2238–2410`）

**P0**：

- Runner 徽标「可检查更新」（`:1176`）语义不对，实际含义是「Runner 版本和 App 不一致」→ 改为「需更新到 {version}」；版本一致时不显示徽标，不显示禁用的「已安装」按钮（`:1204–1210`）。
- 安装 Runner 时只有「安装中…」→ 按钮转圈，并显示「上传 / 校验 / 完成 · m:ss」。目前后端只能给出整体耗时，阶段需要后端补上，否则只显示耗时。

**P1**：

- 第 1076–1077 行「端口 X · 当前账号隔离」「使用账号密码连接，不需要共享同事的凭据」→ 按 §3 处理。
- 第 1169、1222、1226 行三条占位说明（Runner 路径、Claude 状态、Claude 路径）→ 删除；只在检测到实际值时显示。
- 默认模型的 hint（`:1232`）→ 改短；保存按钮写明作用范围。
- 服务器编辑弹窗：
  - 描述「保存你的SSH账号。」删除。
  - 本机明文保存的 checkbox（`:2279–2288`）→ 改为首次保存时的 AlertDialog。
  - 「高级设置 · 通常留空」是真实的高级参数，允许保留折叠，标题改为「高级」。
- 信任指纹（`:2452–2465`）现在需要两步：先填入指纹，再提示用户重新连接 → 改为一个按钮「信任并连接」。失败时弹窗保持打开，并显示错误。

### 2.13 设置（`App.tsx:1982–2084`）

- **P1** 启动自动连接（`:1984–2014`）的成功 toast 和下方的 hint 重复 → 不弹 toast，只更新开关和目标说明；目标说明要显示存储方式和指纹状态（账号已保存、密码可用、指纹已信任；不可用时显示具体原因）。
- **P1** 凭据区的「SSH：本机保存 / 旧凭据待迁移 / 未保存」→ 改为三行 StatusRow：SSH、本机 Codeup、服务器 Codeup，每行带对应操作。
- **P1** 「关于」中的数据目录 → 增加「在 Finder 中显示」（需要后端；暂时可以先提供「复制路径」）。

### 2.14 Codex / MCP（`App.tsx:1850–1901`）

- **P1** 状态（`integration.rs` 返回的 message）、`integration` 文本、底部 hint 三处都在说「需要重新加载」→ 只保留一个 StatusRow「Codex 配置：已写入 · 需在 Codex 重新加载」。
- **P1** `<details>查看配置片段` 和「复制配置」重复 → 删除折叠，只保留复制。
- **P0** MCP配置写入按钮立即进入忙碌态，成功显示「已写入 · 需重新加载」，失败显示原因和重试；保留备份与只改ccbridge段。沿用用户主动点击即发起的交互，不机械增加确认弹窗；发现真实冲突时才明确让用户选择。

### 2.15 应用更新（`App.tsx:1902–1981`）

2026-10-22补充：更新本机App不以SSH连接为前提。保护本机操作和Codex等待；已连接时额外检查远端任务。拦截原因使用具体操作名/任务标题、状态和下一步，不向用户展示“排空”“租约”或无入口的“展开诊断”。未连接不等于远端任务结束，更新不得停止远端独立任务。

- **P1** 第 1906 行描述、第 1916 行「稳定通道」、第 1975–1978 行常驻 hint → 删除。阻止安装的原因只在 `apply_update` 失败时，于安装按钮旁显示。
- **P0** 下载进度：`total` 不存在时现在显示 0%（`:649–655`）→ 有 `total` 时显示百分比；没有时显示「已下载 x MB」。
- **P1** 状态行和按钮改为 OperationBar 的统一样式。

### 2.16 共用：toast、弹窗、空、加载、错误

- **P1** `Toaster richColors`：保留，但颜色要使用 token。
- **P1** 所有 Dialog 改为 §1.3 的尺寸档位和固定 Footer。
- **P1** 所有 `role="alert" className="text-destructive"` 的散落写法 → 统一用 InlineAlert。

---

## 3. 关键文案对照表（现文 → 处理）

> 处理方式：「删」= 删除；「→」= 替换为；「保留」= 原样保留；「移」= 移到别处。

### 全局与外壳

| 位置 | 现文 | 处理 |
|---|---|---|
| App:847–851 | 正在连接服务器… / 尚未连接服务器 | → 连接中… / 未连接 |
| App:986 | 当前App不在“应用程序”中。 | → 未安装到“应用程序”。MCP 和应用内更新需要安装后使用。 |
| App:996 | 安装到Applications并重新打开 | → 移到“应用程序”并重新打开 |
| App:1008–1009 | 正在连接服务器，请稍候… / 先连接服务器，才能读取远端项目与任务。 | → 连接中… / 未连接（配合「连接 user@host」按钮） |
| App:1016 | 前往连接 | → 连接 user@host |
| App:587 | 这是界面预览；请在 CCBridge App 内连接和执行，浏览器不会连接 SSH。 | 移：仅开发模式（`import.meta.env.DEV`）显示 |

### 工作台

| 位置 | 现文 | 处理 |
|---|---|---|
| Workbench:72 | 连接服务器后选择项目 | → 未连接 |
| Workbench:152 | 未检测 | 保留（Runner已有值时显示版本及是否需更新） |
| Workbench:149 | 已就绪 | → 显示 Claude 版本号 |
| Workbench:164 | 管理连接与权限 | → 服务器设置 |
| Workbench:169–177 | 快捷操作 / 克隆仓库 / 配置 Codex MCP | 删除整节（克隆移到页头，MCP 移到「需要处理」区） |

### 项目与文件

| 位置 | 现文 | 处理 |
|---|---|---|
| App:961 | 设为MCP默认项目 | → 设为 MCP 默认项目（统一空格） |
| App:972 | Codeup认证 | → Codeup 认证 |
| App:1399 | 正在打开… | 删（改用骨架行） |
| App:1409 | 服务器文件 | 删 |
| App:2101–2103 | 仅用于此草稿，不改变MCP默认项目 | 保留（属于决策信息），改为：仅本次新任务使用 |

### 文件操作与回收区

| 位置 | 现文 | 处理 |
|---|---|---|
| WT:197 | 移动文件或文件夹 | → 移动 |
| WT:221 | 目标文件夹（保留原名称） | → 移动到 |
| WT:269 | 不会覆盖同名目标；不能移动到自身或子目录。 | 删（改由选择器禁用） |
| WT:274–275 | 将此项移入当前账号的 CCBridge 回收区，文件夹包含其全部内容。可在“回收区”恢复；不永久删除。 | → 将「{name}」移入回收区{是文件夹时加：（含全部内容）}，可恢复。 |
| WT:279 | 进行中的任务、已注册项目根目录和当前工作区受到保护。 | 删 |
| WT:303 | 确认移入回收区 | → 移入回收区 |
| WT:171–172 | 操作已完成，但目录刷新失败；请重新连接后刷新，不要重复执行 | → 已完成，列表未刷新（toast 附「刷新」按钮） |
| WT:362 | 当前账号回收区 | → 回收区 |
| WT:364 | 恢复到原路径，不覆盖已有同名文件。这里不提供永久删除。 | 删 |
| WT:373 | 加载 / 刷新 | → 刷新图标（tooltip「刷新」） |
| WT:395 | 回收区为空。 | → 回收区为空 |

### 分支

| 位置 | 现文 | 处理 |
|---|---|---|
| WT:475 | {path} · 请先打开一个 Git 仓库根目录 | → 仓库名 |
| WT:480 / 487 | 读取 / 刷新分支、获取远端分支（fetch） | → 刷新图标 / 获取远端 |
| WT:424 | 正在获取 origin 引用，不合并代码… | → 获取 origin · m:ss |
| WT:494 | 有未提交改动，禁止切换 | → 有未提交改动，无法切换 |
| WT:559–560 | 不自动 stash、reset 或推送。快进同步不会生成合并提交，分叉时拒绝操作；有未提交改动或运行任务时禁止变更。 | 删 |
| WT:590 | 快进同步到 {remote} | → 快进到 {remote} |

### 克隆

| 位置 | 现文 | 处理 |
|---|---|---|
| App:2480 | 仓库将克隆到 {path}。每行一个 HTTPS 地址，已有目录不会覆盖。 | → 描述：克隆到 {目录名}；placeholder：每行一个 HTTPS 地址 |
| App:2499–2500 | 本机获取，再通过SSH传输 / 服务器直接获取 | → 本机获取 / 服务器获取 |
| App:2511 | 配置 Codeup 认证 / 检测仓库访问 | → 紧凑「认证」入口；失败行另显示「修复认证」 |
| App:2542 | 正在执行，请勿重复提交；本机 Git 认证可能需要系统钥匙串授权。 | 删；处于钥匙串阶段时显示：等待 macOS 钥匙串授权 |
| App:451 | 处理完成：N 个仓库（已有仓库保持不变） | → 已克隆 a · 已存在 b |
| App:453 | 仓库已准备好 | 删（结果已显示在弹窗中） |
| App:2565 | 正在克隆… / 克隆到当前目录 | → 进行中… / 克隆 |

### Codeup 认证

| 位置 | 现文 | 处理 |
|---|---|---|
| CA:188 | Codeup 认证与访问检测 | → Codeup 认证 |
| CA:190 | HTTPS 用户名与密码 / 访问令牌。无需到终端配置，不使用 SSH 登录密码。 | 删 |
| CA:193 | 在哪里执行 Git | 删（由分段控件替代） |
| CA:199–200 | 本机获取，再通过 SSH 传仓库 / 服务器当前 SSH 账号直接获取 | → 本机 Git / 服务器 Git · {account} |
| CA:205–206 | 当前账号：…。App 档认证保存在 Mac…；VPS 终端 / Claude 的认证请在下方单独同步。/ 认证保存在 Mac…可在下方单独授权同步到 VPS。 | 删 |
| CA:210–211 | App 已保存：{u} / App 尚未保存认证 · 可先检测现有 Git 凭据 | → StatusRow「App 凭据」：已保存 · {u} / 未保存；本机 scope 下未保存时补充：使用本机 Git 凭据助手 |
| CA:213 / 221 / 229 | Codeup 仓库 HTTPS 地址 / Codeup HTTPS 克隆用户名 / HTTPS 密码 / 访问令牌 | → 仓库地址 / 用户名 / 访问令牌 |
| CA:239–240 | 留空使用已保存认证；填写则更新 / 请输入 Codeup 的 HTTPS 密码或访问令牌 | → 留空则保持已保存 / （空） |
| CA:244 | 检测只读取远端引用，不克隆、不推送。填写但未保存的凭据也可直接检测；不会显示或回填已保存的令牌。 | 删 |
| CA:251 | 迁移此作用域已保存的认证 | → StatusRow 行内操作：迁移到本机文件 |
| CA:261 | 我理解密码以明文保存在当前用户私有目录，同一用户程序仍可读取。 | 移到首次保存时的 AlertDialog：「令牌将明文保存在本机当前用户目录，同一用户运行的程序可读取。」按钮：保存 |
| CA:266 | VPS 持久认证 | → VPS 凭据 |
| CA:274–276 | VPS 已配置 / 未由 App 配置 / 检测中… / 检测失败 | → 已写入 / 未写入 / 读取中… / 读取失败 · 原因 |
| CA:281 | 可在克隆之后随时补同步，不需要重新克隆。 | 删 |
| CA:284–285 | 同步使用上方输入的令牌；留空则使用所选档已保存认证…不修改 SSH 登录。 | 移到写入确认框，改为一行：「凭据来源：输入的令牌 / App 已保存 / 本机 Git 凭据助手」（按实际情况显示一种） |
| CA:294–295 | 我同意将该认证以明文保存到此 VPS 账号的私有文件…推荐专用最小权限令牌。 | → AlertDialog「写入 VPS 凭据？」，正文：账号 {account}；位置 ~/.local/share/ccbridge/codeup-auth（明文，权限 0600，同账号程序与服务器管理员可读）；建议使用只读令牌。按钮：取消 / 写入 VPS |
| CA:303 | 同步到当前 VPS | → 写入 VPS… |
| CA:310 | 检测 VPS 自身认证 | → 检测（VPS 访问行） |
| CA:317 | 移除 VPS 认证 | → ⋯ › 移除 VPS 凭据 |
| CA:323–324 | 只移除此 App 写入的 VPS Codeup 认证，Mac 认证及服务器其他 Git 配置保留。 | → 确认框正文：「移除 {account} 上由 CCBridge 写入的 Codeup 凭据。」按钮：移除 |
| CA:357–358 | 忘记 App 为…保存的 Codeup 认证？之后仍可重新输入保存。 | → 「移除 App 为 {scope} 保存的 Codeup 凭据？」按钮：移除 |
| CA:162–163 | 已忘记 App 保存的认证；系统原有 Git 凭据助手不受影响 / 认证已保存，可检测访问或直接克隆 | → 已移除 / 已保存（StatusRow 同步更新，不弹 toast） |
| vps_auth.rs:191 | VPS 配置已写入；请检测仓库访问以确认权限 / VPS 尚未配置；可随时从 App 补同步，无需重新克隆 | 后端保留（MCP 和日志会用到），UI 不显示；UI 只使用 `configured` 字段 |
| backend git_vps_auth_test 失败 | VPS 自身 Git 认证或仓库权限不足；请在此窗口点击‘同步到当前 VPS’…本次未使用 Mac 凭据。 | UI 显示为：VPS 访问失败 · 认证或仓库权限不足（行内附「写入 VPS…」按钮） |

### Git 更新

| 位置 | 现文 | 处理 |
|---|---|---|
| GU:81 | Git 更新 | 保留；副标题：{目录名} · {n} 个仓库 |
| GU:83 | 完整路径 | 移到 tooltip，并提供复制 |
| GU:86 | 获取各仓库当前跟踪分支并快进更新。不切分支、不推送；未提交改动、分叉或运行中的任务会阻止对应仓库。 | 删 |
| GU:92 | 未检出分支 | → 分离 HEAD |
| GU:95 | 跟踪 {upstream} / 未设置跟踪分支 | → `master ← origin/master` / 无上游 |
| GU:104 | 正在检查 / 更新，请勿重复提交… | → OperationBar：检查中… 或 「2/3 · 获取 ehs-miniprog · 0:42」 |
| GU:138 | 配置 Codeup 认证 / 同步到 VPS | → 在失败行显示：修复认证 |
| GU:150 | 刷新状态 | → 刷新 |
| GU:156 | 更新中… / 获取并快进更新 | → 进行中… / 快进更新 {n} 个仓库 |
| git_update.rs:46 | 有未提交改动，请先处理 | 行内显示为：跳过 · 有未提交改动 |
| git_update.rs:48 | 当前分支未配置远端跟踪分支，请先在分支管理中设置 | → 跳过 · 无上游（附「分支」按钮） |
| git_update.rs:121 / 126 / 134 | 已是最新 / 本地有领先提交，保留原状；不会推送 / 已快进更新 | → 已是最新 / 本地领先，未改动 / 已快进 a1b2c3d → e4f5a6b |
| git_update.rs:130 | 本地与远端已分叉，不能快进；请人工处理，不会 reset 或 stash | → 失败 · 已分叉，需手动处理 |
| backend 校验 | 仓库分支或提交已变化，请刷新后重新确认 | → warning 提示：状态已变化，请刷新（附「刷新」按钮） |

### 会话

| 位置 | 现文 | 处理 |
|---|---|---|
| App:1448 | 请先在项目页注册此目录 | → 未注册为项目 |
| App:1499 | 还没有远端任务 | → 还没有会话 |
| App:1519 | 全自动 / 审批模式 | → 全自动（警示色）/ 审批 |
| App:1536 | 项目连接已变更；续聊仍使用原快照。要使用新映射或新凭据，请新建对话。 | → 项目数据库配置已更改，本会话仍用旧配置（附「用新配置新建任务」按钮） |
| App:1577 / 1595 | 本轮没有文本结果 / 本轮没有文本结果。 | → 无文本输出 |
| App:1594 | 正在执行；状态每 5 秒更新，不会重复派发任务。 | → 执行中 · m:ss |
| App:1603 | 本轮执行失败，请展开诊断查看原因。 | → 执行失败 |
| App:1606 | 诊断详情 | → 原始错误 |
| App:1614 | 运行记录 · N 条 | → N 个步骤 |
| App:1653 | 需要确认 · {tool_name} | → 需要确认 · {工具中文名} |
| App:1668 | 允许本次 | → 允许 |
| App:1725 | 审批模式 | → 审批 |
| App:1756 | · 账号默认配置 / · 当前会话配置 | → 账号默认 / 本会话 |
| App:1759 | 远端配置（不指定模型） | → 远端默认 |
| App:1761 / 1773 | 指定其他模型… / sonnet / opus / 完整 ID | 保留 |
| App:1803 | Shift ↵ 换行 | 删 |
| conversation.ts:38–39 | 远端默认模型 / 实际模型尚未上报 | → 远端默认 / 执行中：等待上报；结束后：未上报 |
| conversation.ts:121 / 126 | 模型待确认（远端默认） | → 远端默认 |

### 数据库

| 位置 | 现文 | 处理 |
|---|---|---|
| DB:134–135 | 连接保存在当前 SSH 账号下。每个项目可以映射多个连接，同一连接也可供多个项目使用。 | 删 |
| DB:150 | 探测远端 MCP 环境 | → 运行环境状态行 + 「重新探测」 |
| DB:179 | 运行文件已找到 / 版本暂不兼容 | → universal-db-mcp {版本} ✓ / 版本 {x} 未验证 |
| DB:190 | （仅识别运行环境，不继承共享凭据与权限） | 删 |
| DB:200 | 还没有连接。探测只查找软件，不会自动读取共享数据库凭据或连接业务数据库。 | → 还没有数据库连接（附「新增连接」按钮） |
| DB:217–218 | 已保存密码 / 未保存密码 · 只读 safe · N 个项目引用 | → 只读 · 被 N 个项目使用；未保存密码时加警示：未保存密码；仅当实际认证要求密码且没有其他有效来源时显示“缺少凭据” |
| DB:255 / 273 | 检查 MCP 握手 / 测试数据库连接 | → 合并为一个「测试」按钮，结果分两行：MCP 握手、数据库连接 |
| DB:293 | 按准确项目目录匹配，不继承父目录或其他项目。变更仅对新会话生效；续聊保留原连接快照。 | → 保存按钮旁：仅对新会话生效 |
| DB:368–369 | 用途别名使用英文字母、数字、下划线…Claude 将看到 db_用途别名。 | → 在别名输入框右侧实时显示：工具名 db_{alias} |
| DB:394 | 项目映射已保存；已有会话保持原配置，新会话使用新映射。 | → 已保存，新会话生效 |
| DB:420–421 | 删除“…”的连接配置，不会删除数据库或业务数据。被项目引用的连接不能删除。 | → 「删除连接配置「{name}」？数据库本身不受影响。」（被引用时直接禁用删除，tooltip 说明原因） |
| DB:442 | 确认删除配置 | → 删除（destructive 样式） |
| DB:465–466 | 固定只读 safe，建议使用数据库本身仅有 SELECT 权限的账号。密码不显示在列表、模型指令或 MCP 启动参数中。 | → 用户名字段下方：建议使用只读（SELECT）账号 |
| DB:516–517 | 可先在页面探测。当前适配 universal-db-mcp 2.14.0；发现其他版本时明确提示，不自动改服务器依赖。 | 删 |
| DB:535 | 连接已保存，尚未测试数据库。请添加项目映射后开启新会话。 | → 已保存（未测试） |

### 服务器

| 位置 | 现文 | 处理 |
|---|---|---|
| App:1072 | 添加你的服务器 | → 还没有服务器 |
| App:1076 | 端口 {port} · 当前账号隔离 | → {user}@{host}:{port} |
| App:1077 | 使用账号密码连接，不需要共享同事的凭据 | 删 |
| App:1151 | App 与 MCP 可按任务请求全自动 | 保留（决策信息） |
| App:1163 / 1164 | 尚未检测到 / 连接后检测 | → 未检测到 / （未连接时整行隐藏） |
| App:1169 | 自动使用远端 HOME/.local/bin/ccbridge | 删 |
| App:1174–1177 | 已是最新版本 / 可检查更新 / 待检测 | → 已安装 / 需更新到 {version} / 未检测 |
| App:1205–1212 | 已安装 / 安装 / 更新 Runner | → （版本一致时不显示按钮）/ 安装 Runner / 更新到 {version} |
| App:1222 / 1226 | 使用远端已安装并登录的 Claude Code / 自动探测可执行文件 | 删；检测失败时显示：未检测到 · {原因} |
| App:1232 | 留空沿用远端 Claude 设置。修改后保存；已存在的对话保留各轮模型。 | → placeholder：远端默认 |
| App:1249 | 保存默认模型 | → 保存为新会话默认 |
| App:1245 | 默认模型已保存 | 删（按钮旁显示 ✓） |
| App:2242 | 保存你的SSH账号。 | 删 |
| App:2259 | 已保存，留空即可 / 只在本次连接中使用 | → placeholder：已保存；后者删 |
| App:2278 | 密码保存在本机，仅当前Mac账号可访问。 | 删（并入首次保存的确认框） |
| App:2286 | 我了解：本机保存不会弹钥匙串授权，但当前Mac账号下的其他程序可能读取密码。 | 移到首次保存时的 AlertDialog：「密码将明文保存在本机当前用户目录，同一用户运行的程序可读取。」按钮：保存 |
| App:2292 | 高级设置 · 通常留空 | → 高级 |
| App:2364 | 只读取此账号的钥匙串密码并保存到本机，原副本保留；macOS可能要求一次授权。 | → 迁移按钮旁：可能弹出钥匙串授权 |
| App:2444 | 请通过可信渠道核对服务器指纹。确认后才会保存并重新连接。 | → 核对指纹后再信任 |
| App:2465 / 2461 | 信任此指纹 / 已填入指纹，请再次连接确认。 | → 信任并连接；后者删 |
| App:2416–2417 | Claude 将跳过工具审批，拥有当前 SSH 账号的文件、命令和网络权限。仅对可信任务和专用低权限账号开启。 | 保留（授权点） |
| App:2434 | 确认允许全自动 | → 允许全自动 |

### Codex / MCP、应用更新、设置

| 位置 | 现文 | 处理 |
|---|---|---|
| App:1853 | 正在检查配置… | → 检查中… |
| App:1865 | 自动写入MCP配置 | → 写入 Codex 配置（点击后立即有忙碌与结果反馈） |
| App:402 | 正在备份并写入 Codex MCP 配置… | 删（按钮内转圈即可） |
| App:1894–1895 | 查看配置片段（折叠） | 删（保留「复制配置」） |
| App:1898 | 写入后在Codex重新加载MCP。配置已保存不代表当前会话已加载工具。 | 删（状态行已说明「需在 Codex 重新加载」） |
| App:1906 | GitHub Releases · 更新包在安装前验证发布签名。 | 删 |
| App:1916 | 稳定通道 | 删 |
| App:1921 | 检查是否有新版本 | → 上次检查：{时间} / 未检查 |
| App:1925 | 正在下载并验证签名… | → 下载中 x MB / y MB（有 total 时显示百分比） |
| App:1928 | 更新未完成，当前 App 保持可用 | 保留（只在失败时显示） |
| App:1976–1977 | 运行中的任务、审批、克隆或 MCP 等待会阻止安装。请先完成活动，再安装；不会强制中断远端任务。 | 删（安装被阻止时，按钮旁显示后端给出的原因） |
| App:1997–1998 | 已开启：下次启动连接上次账号 / 已关闭自动连接 | 删（不弹 toast） |
| App:2010 | 先保存服务器账号 | → 需要先保存一个服务器账号 |
| App:2044–2046 | 本机保存 / 旧凭据待迁移 / 未保存 | → StatusRow：SSH 密码 · 本机文件 / 钥匙串（待迁移）/ 未保存 |

---

## 4. 实施顺序、事件字段、接口与伪代码

### 4.0 顺序（每一步可以单独合并）

1. **全局视觉 P0**：base border 规则、outline 按钮、删除旧 token、补 success/warning/dark destructive。只改 CSS 和 `ui/button.tsx`。
2. **共用组件**：`ui/alert-dialog`、`ui/toggle-group`、`ui/tooltip`、`common/{StatusRow,OperationBar,InlineAlert,EmptyState}`、`lib/format.ts`（时间、耗时、字节格式化）。
3. **后端统一操作进度**（git_update、clone），**不改**现有返回结构。
4. **fetch 流式进度**和 bundle 上传字节计数。
5. **重写 GitUpdate**（使用 `useOperation`）。
6. **重写 CodeupAuth**（StatusRow + AlertDialog），修复 VPS 优先级显示。
7. **克隆弹窗**改用 OperationBar。
8. **会话**：保持用户模式选择，审批卡片按工具展示、稳定开始时间计时，具体模型展示。
9. 其他页面的文案和布局 P1。
10. 补充 vitest、Rust 单元测试，以及截图回归。

### 4.1 后端：统一的操作快照

在 `src/backend.rs` 的 `BackendState` 中，把 `clone_progress` 替换为下面的 `operations`，同时保留 `clone_progress` 这个 op 名，作为兼容别名返回 `operations["clone"]`：

```rust
pub operations: std::sync::Mutex<serde_json::Map<String, Value>>, // key = "git_update" | "clone"
```

快照字段（只用于 UI，MCP 不依赖）：

```json
{
  "id": "uuid", "kind": "git_update", "source": "app|mcp",
  "state": "running|done|partial|failed|interrupted|unknown",
  "started_at_ms": 0, "updated_at_ms": 0, "finished_at_ms": null,
  "path": "/home/cyb/projects/shenniudata",
  "total": 3, "completed": 1,
  "current": {"path": "...", "name": "ehs-miniprog", "phase": "check|fetch|apply|bundle|upload|import",
              "since_ms": 0, "objects": {"received": 450, "total": 1000} | null,
              "bytes": {"received": 1258291, "total": null} | null},
  "items": [{"path": "...", "name": "bro_uhp", "branch": "master", "upstream": "origin/master",
             "state": "pending|running|updated|unchanged|ahead|skipped|failed|unknown",
             "code": "dirty|no_upstream|task_running|diverged|auth|network|stale|connection_lost|other|null",
             "message": "...", "before": "sha", "after": "sha", "elapsed_ms": 0}]
}
```

新增一个查询 op（只读，不进写锁，不需要人工操作）：

```rust
if op == "operation_progress" {
    let k = v["kind"].as_str().unwrap_or("");
    return Ok(state.operations.lock().unwrap().get(k).cloned().unwrap_or(json!({})));
}
```

改造 `git_update` 循环（`backend.rs` 中现有的 for 循环），保持原来的返回结构：

```rust
let op_id = uuid();
let mut snap = OpSnap::new("git_update", if human {"app"} else {"mcp"}, path, &repos, &v["only"]);
publish(&state, &snap);
for repo in selected_repos {               // only: 可选的 [path]；不传时为全部（MCP 行为不变）
    snap.start(repo, "check"); publish(..);
    let r = async {
        c.rpc(preflight).await.map_err(classify)?;          // classify -> code: dirty/no_upstream/task_running/stale
        snap.phase("fetch"); publish(..);
        c.git_fetch_remote_progress(path, remote, |p| { snap.fetch(p); publish_throttled(..) })
            .await.map_err(classify_fetch)?;                // auth/network
        snap.phase("apply"); publish(..);
        c.rpc(apply).await.map_err(classify)                // diverged
    }.await;
    let item = match r { Ok(x) => x, Err(e) => json!({"path":..,"branch":..,"state":"failed",
                          "code": e.code(), "message": format!("{e:#}")}) };   // 原有字段不变，新增 code
    snap.finish_item(item.clone()); publish(..);
    results.push(item);
    if c.handle.is_closed() { snap.mark_rest("connection_lost"); break; }      // 剩余项 unknown/中断
}
snap.close(); publish(..);
Ok(json!({"ok":failed==0,"updated":updated,"failed":failed,"results":results}))  // MCP 结构不变
```

- 被阻止（blocked）的仓库：`state` 仍为 `"failed"`，`code` 取 `"dirty"` 或 `"no_upstream"`，UI 显示为「跳过」。
- 错误分类不再依赖正则匹配中文：`git_update.rs` 和 `ssh.rs` 在 `bail!` 时附带 code（例如用一个小的 `CodedError` 类型，或者在消息前加 `CODE:` 前缀，再由 `error_summary` 剥离）。

### 4.2 fetch 真实进度（`src/ssh.rs`）

新增 `execute_streaming(cmd, input, dur, on_stderr: impl FnMut(&[u8]))`：在 `ChannelMsg::ExtendedData` 到达时调用回调，其余逻辑与 `execute_reader` 相同。新增：

```rust
pub async fn git_fetch_remote_progress(&self, path:&str, remote:&str, mut on: impl FnMut(FetchProgress)) -> Result<Value> {
    // 与 git_fetch_remote 相同的校验和凭据选择
    cmd += &format!("git_auth -C \"$path\" fetch --progress -- {}\n", shell_quote(remote));
    let mut buf = Vec::new();
    self.execute_streaming(cmd, &input, Duration::from_secs(300), |chunk| {
        buf.extend_from_slice(chunk);
        while let Some(i) = buf.iter().position(|b| *b==b'\r' || *b==b'\n') {
            let line = String::from_utf8_lossy(&buf[..i]).to_string(); buf.drain(..=i);
            if let Some(p) = parse_git_progress(&line) { on(p) }
        }
    }).await
}
// Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s
fn parse_git_progress(l:&str)->Option<FetchProgress>{
    let re = Regex::new(r"Receiving objects:\s+\d+% \((\d+)/(\d+)\)(?:,\s+([\d.]+) (KiB|MiB|GiB))?").ok()?;
    let c = re.captures(l)?; /* objects 来自 git；bytes 是 git 报告的已接收量，total 未知 */ ...
}
```

- 如果不想引入 `regex` 依赖，可以手写解析。
- `stderr` 仍要完整保留，用于错误分类。注意 当前输出限额：进度行带 `\r`，长时间 fetch 的 stderr 可能很长，所以流式模式下 stderr 只保留最后 64 KiB。
- 现有的 `git_fetch`、`branch_fetch` 也可以复用这个函数，让分支弹窗也能显示进度。

**bundle 上传的真实字节**（`clone_into_progress` 的本机获取路径）：bundle 文件大小可以通过 `metadata().len()` 拿到；给 `execute_reader` 的 input 包一层计数用的 `AsyncRead`，每读取 256 KiB 上报一次 `bytes{received,total}`。只有这种情况允许显示百分比。

### 4.3 前端：`lib/useOperation.ts`

```ts
export function useOperation(kind: "git_update" | "clone", enabled: boolean) {
  const [snap, setSnap] = useState<Op | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let alive = true, t: number;
    const tick = async () => {
      const s = await invoke<Op>("operation_progress", { request: { kind } }).catch(() => null);
      if (alive) { setSnap(s?.id ? s : null);
        t = window.setTimeout(tick, s?.state === "running" ? 700 : 3000); }
    };
    void tick(); return () => { alive = false; clearTimeout(t); };
  }, [kind, enabled]);
  return snap; // running 由后端决定；MCP 发起的运行也能看到（source:"mcp"）
}
```

- 在 App 层（而不是弹窗内部）调用 `useOperation("git_update", connected)`。页头 chip 和 GitUpdate 弹窗读取同一个快照。
- 弹窗关闭时**不影响**后端执行；重新打开时，如果 `snap.state==="running"`，直接显示进度。

### 4.4 GitUpdate 状态与 UI（伪代码）

```tsx
const running = snap?.state === "running";
const phase = running ? "running" : results ? (failed ? (updated||unchanged ? "partial" : "failed") : "done")
            : plan ? "ready" : "checking";
Header: 「Git 更新」 · {dirName} · {plan.length} 个仓库   [刷新图标(disabled=running)]
<OperationBar
  running={running}
  label={running ? `${snap.completed}/${snap.total} · ${phaseText(snap.current.phase)} ${snap.current.name}` : summary}
  elapsed={running ? now - snap.started_at_ms : lastDuration}
  value={running ? snap.completed / snap.total : undefined}        // 只按仓库数
  detail={snap.current?.objects ? `对象 ${o.received}/${o.total}` + (b ? ` · ${fmtBytes(b.received)}` : "") : null}
/>
<RepoTable rows={merge(plan, snap?.items)}>                        // 44px 行：图标 名称 | 分支←上游 | HEAD | 状态
  状态列：pending→等待；running→阶段名；updated→已快进 a→b；unchanged→已是最新；ahead→本地领先，未改动；
          failed+code∈{dirty,no_upstream}→跳过·原因(+分支按钮)；failed+auth→失败·认证未通过[修复认证(该行url)]；
          failed+connection_lost/unknown→结果未确认[刷新]；其他 failed→失败·message
Footer:
  running:  [关闭]                                      // 后台继续
  ready:    [关闭] [快进更新 {eligible} 个仓库](primary, disabled=eligible===0)
  partial/failed: [关闭] [刷新] [重试失败项](primary，仅当存在可重试项，即 auth/network/stale)
  done:     [完成](primary)
stale: 相关对象失效或后端返回 stale → <InlineAlert warning>状态已变化 [刷新]</InlineAlert>
```

- 重试失败项：调用 `git_update` 时带上 `{expected: freshPlan, only: failedPaths}`。前端在重试前先重新获取一次 plan，以满足后端的 expected 校验。
- **防重**：开始按钮在 `running` 或本地 `starting` 时禁用。如果后端返回写锁错误，显示「正在执行：{snap.kind}」。

### 4.5 CodeupAuth 状态

```ts
type Scope = "local" | "remote";
appCred:   {state: "saved"|"none"|"error", username?, storage?: "file"|"keychain"}   // git_auth_status
access:    Map<`${scope}|${url}`, {state:"idle"|"testing"|"ok"|"fail", at?, message?}> // 仅本次会话内存，URL 改变自动对应新 key
vpsCred:   {state:"loading"|"installed"|"none"|"error"|"runner_outdated", message?}    // git_vps_auth_status.configured
vpsAccess: 同 access，key 为 `vps|${url}`
effective = scope==="remote" && vpsCred.state==="installed" ? "vps" : "app";        // 与 remote_git_credential 优先级一致
```

- 写入 VPS：按钮打开 `<AlertDialog>`，确认按钮执行 `run("git_vps_auth_install", {server_storage_ack: true})`。确认框每次打开都重新渲染，**没有**任何持久化的 ack 状态。
- 写入成功后，`vpsCred` 变为 `installed`，`vpsAccess` 重置为 `idle`，界面显示「未检测」。保存操作结束后可由明确的「保存并检测」动作连续检测；若用户仅选择保存则保持「未检测」，绝不把写入结果当检测结果。

### 4.6 会话模式与模型

- 模型选择只使用当前SSH账号的远端Claude初始化返回清单，服务器默认与会话入口均为下拉框，不提供手填回退，不用历史模型冒充可选清单。远端默认的解析值属于配置，不能替代轮次实际模型。
- Claude额度在工作台和服务器页共用查询结果与状态。优先通过Claude自身账户接口查询，标明更新时间；接口不提供数据或失败时明确原因，并将已有事件标为会话历史，不能冒充实时账户额度。账户百分数与事件比例按各自协议解析，不猜单位。未知不等于0%，恢复时间缺失不编造；查询不发模型提示、不读或转发模型凭据，短缓存避免重复请求。

- 新任务保持本用户已授权的全自动默认，不将Claude的通用安全偏好变成行为回退。
- 模式与服务端授权分别展示、分别校验；授权关闭不自动改变历史轮次的模式。
- 续聊按当前会话保留模式与目录，用户明确改模型只影响下一轮；轮次记录实际模型。实际模型未上报时明确写「等待上报」或「未上报」，不能用请求值冒充实际值。
- 不引入新仓库前置条件，原有无需仓库的一般问答和查询仍应可用。

---

## 5. 验收清单

### 5.1 开发前必查（人工，每个 PR 描述中逐条回答）

1. 这个改动新增了哪些文字？每条分别满足 §1.1 的哪一项（状态 / 决策 / 授权点 / 下一步）？
2. 新增的操作需要多长时间？如果超过 1 秒，当前对象、阶段、N/M、耗时分别从哪个字段来？百分比的数据来源是真实的吗？
3. 关闭弹窗后，后端是否还在继续？在哪里可以看到进度？是否真的支持取消？
4. 是否涉及写凭据、写配置、删除或改变工作区？确认框里写的对象和后果是什么？确认状态会不会被记住？
5. 是否改动了 MCP 可调用的 op，或者它的返回结构？只做了新增吗？

### 5.2 机器检查（可以做成 `pnpm lint:ui` 脚本或 CI 步骤；现在先列出命令）

| # | 检查 | 命令 / 方法 | 预期 |
|---|---|---|---|
| M1 | 没有颜色的边框 | `rg -n '\bborder\b(?![-\w])' ui/src --glob '*.tsx'`，人工确认每一处都有 `border-border` 或 `border-(color)` | 0 处缺少颜色；或者全局 base 规则存在：`rg -n 'border-color: var\(--color-border\)' ui/src/index.css` 命中 1 次 |
| M2 | 禁用词 | `rg -n '请勿重复\|请稍候\|一目了然\|开源版\|已就绪\|不会.*不会' ui/src` | 0 |
| M3 | 假进度 | `rg -n 'animate-pulse' ui/src` | 0（或者只出现在骨架屏中） |
| M4 | 授权确认路径 | 检查VPS写入、删除及首次本机保存的请求发起位置与前端测试 | 未确认不得发送授权写请求；不禁止真实业务checkbox |
| M5 | 硬编码颜色 | `rg -n '#[0-9a-fA-F]{3,8}\b' ui/src/index.css`，只允许出现在 `:root`、`:root.dark` 块中；`rg -n 'bg-white\|blue-\|gray-' ui/src --glob '*.tsx'` | token 块以外 0 处 |
| M6 | 折叠里放说明 | `rg -n '<summary>' ui/src`，逐个确认内容是日志、JSON、诊断或「高级」字段 | 没有解释性文字 |
| M7 | 主按钮数量 | 每个 `DialogFooter` 中 `variant` 未设置（即 default）的 Button 数量不超过 1（写一个 vitest 渲染测试） | ≤ 1 |
| M8 | 模式合同 | vitest：已授权全自动默认保持；未授权被阻止；续聊不静默改变模式 | 通过 |
| M9 | 进度解析 | Rust 单测 `parse_git_progress`：真实 git 输出样本（包含 `\r` 分隔、KiB/MiB、没有字节数的行） | 通过 |
| M10 | MCP 兼容 | Rust 测试：`git_update` 返回的顶层键仍是 `ok, updated, failed, results`；`results[].state` 属于 `{updated, unchanged, ahead, failed}`；只多出 `code` 等新字段。保留现有的 `fast_forward_dirty_divergence_and_changed_head_guards` 测试 | 通过 |
| M11 | VPS 确认 | 保留现有 `vps_auth` 测试（不带 ack 被拒绝）。新增前端测试：直接调用写入 VPS 时，参数中没有 `server_storage_ack:true`；只有确认框按钮的路径会带上 | 通过 |
| M12 | 操作快照 | Rust 测试：模拟 3 个仓库（其中 1 个 dirty），`operation_progress` 依次出现 check/fetch/apply 阶段，`completed` 单调递增，结束时 `state=partial` | 通过 |

### 5.3 人工验收场景（真实 App，亮暗各一次，窗口 1200×800 和 1000×700）

1. **视觉**：项目页、Git 更新、Codeup 认证、克隆、服务器、数据库各截一张图。
   - 不应有黑色边框。可以在 DevTools 中查看任意 outline 按钮、Dialog 的 computed `border-color`，应等于 `--line` 或 `--strong`。
   - 每个弹窗只有一个主按钮。
2. **Git 更新（隔离的3个测试仓库：A 落后远端，B 已是最新，C 有未提交改动）**：
   - 打开弹窗：A、B 显示「可检查更新」（fetch前不宣称已知远端是否落后），C 显示「跳过 · 有未提交改动」。主按钮显示「快进更新 2 个仓库」。
   - 点击更新：摘要行依次显示 `已处理 1/3 · 获取 A · 0:0x`；如果有对象数据，显示「对象 x/y · n MiB」，并标明这是Git当前阶段。
   - 中途关闭弹窗：页头显示 chip「Git 更新 1/3」。重新打开时继续显示实时进度。
   - 结束时：A 显示「已快进 a→b」，B 显示「已是最新」，C 显示「跳过」；摘要显示「已更新 1 · 已是最新 1 · 跳过 1 · 用时」。
3. **认证失败**：使用隔离测试凭据与专用测试仓库模拟失败，不修改用户实际VPS令牌。A 行显示「失败 · 认证未通过 [修复认证]」，打开的认证弹窗预填 **A** 的 URL。修复后点击「重试失败项」，只更新 A。
4. **断线**：更新过程中断开网络。正在处理的仓库显示「结果未确认」，后续仓库显示中断，界面提供「刷新」而不是「重试」。刷新后显示真实状态。
5. **状态已变化**：打开弹窗后，在终端里对 A 执行 commit，再点击更新。界面显示 warning「状态已变化 · 刷新」，没有任何仓库被修改。
6. **MCP**：在 App 打开的状态下，由 Codex 调用 `git_update`。App 页头 chip 显示「Git 更新（MCP）」，App 内的开始按钮被禁用。MCP 拿到的返回结构与之前一致。账号关闭全自动后，MCP 调用被拒绝（行为不变）。
7. **Codeup 认证**：
   - 服务器 scope，VPS 未写入：显示 App 凭据、访问检测、VPS 凭据「未写入」三行，没有任何 checkbox。
   - 点击「写入 VPS…」：确认框显示账号、路径、可读范围。点「取消」后没有任何请求发出（在 Network/IPC 日志中确认）。点「写入 VPS」后，状态变为「已写入 · 未检测」。
   - 重新打开弹窗，再次写入时**仍然要求确认**。
   - VPS 已写入时，App 凭据行显示「服务器 Git 当前使用 VPS 凭据」。
   - 修改仓库地址后，访问检测结果变回「未检测」。
8. **克隆（本机获取）**：显示「上传 x MB / y MB」，并显示真实百分比；导入阶段显示不确定进度条加阶段文字和耗时。关闭弹窗后，后台继续执行。
9. **会话**：本用户新任务保持「全自动」，未授权账号被明确阻止；审批任务显示「审批」。审批卡片中 Bash 显示命令而不是 JSON。执行中显示「执行中 · m:ss」。
10. **全局错误**：先在项目页触发一个文件预览错误，再切到设置页，看不到这条错误。断开 SSH 后，只在侧栏和服务器页看到连接错误。

### 5.4 开发后必查（人工判断，和机器检查分开）

- 对着 §3 文案表逐条核对；新增的文字也逐条套用 §1.1 判断。
- 截图对比亮色和暗色两套主题，检查对比度和层级（是否只有一个主按钮，状态色是否只用于状态）。
- 每个长操作都经过「打开 → 关闭 → 再打开」的流程，状态连续。
- 每个授权点都试一次取消，确认没有发出写请求。

---

**开发约束引用方式**

- `docs/UI_GUIDELINES.md` 为唯一正文，开发前后检查以第5章为准。
- `AGENTS.md` 和 `CONTRIBUTING.md` 各加一行引用：「UI 改动遵循 docs/UI_GUIDELINES.md，PR 需附 §5.1 回答与 §5.2 检查输出」。
- §5.2 的 M1–M7 收进 `ui/scripts/ui-lint.mjs`，在 CI 的前端测试之前运行。

## 6. 技术落地时必须补齐的约束

### 6.1 操作身份与生命周期

Claude提出的按kind索引快照只适合作为草图，正式接口以不可变operation_id为主键，并绑定账号/连接世代、工作区和请求来源。`latest[kind]`仅用于发现，不可直接作为某个弹窗自己的结果。并发、不同目录、旧请求晚返回时都按ID过滤，轮询和订阅只读取自己有权查看的操作。操作创建须有幂等键；写锁仍是最终执行互斥，不因进度接口而放宽。

快照包含revision、状态、当前阶段、稳定开始时间、完成时间、明细。读取快照不得等待长操作写锁；不得在持有快照Mutex时等待SSH或其他网络调用。阶段更新节流，完成快照保留至界面可重新取得结果；未做持久化前，重启只能显示结果未知并重新核实，不能声称自动恢复运行。原同步App/MCP入口及结果保持兼容，新接口只作增量扩展。

### 6.2 进度与计数语义

- `processed/total`表示本次计划已处理项数，成功、失败、跳过分别计数；100%表示处理结束，不代表全部成功。取消、断线和未知不伪装为完成。
- Git对象数只表示当前Git阶段，不作为整仓库百分比。上传已发送字节不等于服务端已接收或导入完成，上传后显示独立校验/导入阶段。
- `partial`包括updated/unchanged/ahead任一成功结果与失败并存，不仅看updated数量。
- 未执行项、主动跳过和执行失败分别显示。MCP保留原state语义；新增code用固定枚举，UI跳过计数不重定义旧failed字段。
- `only`重试范围必须为本次显式根目录解析出的合法仓库子集；fresh expected的比较范围与执行范围先统一，禁止为方便重试跳过快照校验。
- structured error优先使用类型化code，不把文案正则或CODE前缀当作长期协议。stderr限长且脱敏，不能把凭据、带认证URL或环境变量发入进度/日志。

### 6.3 认证与前端异步

有效凭据来源按实际仓库URL及当前账号确定：VPS优先只适用于匹配的Codeup HTTPS，不扩大到任意Git主机。已保存不等于已检测，检测结果绑定URL、scope、账号、连接世代及凭据修订；输入、保存、移除或切账号使旧结果失效。旧请求完成后不得回填新表单。VPS确认绑定本次账号、来源与内容；期间改变任一项须重新确认，不在确认前把ack写入持久状态。

服务器安装、认证、数据库、更新的通用状态呈现可复用，但不要为了统一外观另建一套平台。读取数据可去重或合并同一Promise；写操作遇inflight明确返回已有operation_id/状态，不能返回一个看似成功的undefined。idle不轮询；有正在运行的操作才按有界频率取进度，完成后停止。

### 6.4 规范检查的取舍

规范的目标是可理解、可操作，不是把界面变成禁词扫描结果。M1–M7扫描只列候选，需要语义审查；`rg`前瞻表达式需`--pcre2`，机器检查脚本必须以真实项目语法测试后再纳入CI。不禁止所有checkbox、不禁止有意义tooltip、不禁止品牌图标与必要语法色；纯文本提示若表达未知结果或重要后果仍应保留。系统风险和破坏性后果在确认处可超过普通说明长度。

### 6.5 首批视觉预览与审核点

实施前先出Git更新（检查/执行/部分失败）、Codeup认证（Mac/VPS/确认）、文件页带后台进度、会话、服务器五组预览，亮暗两套。重点审核普通边框是否退到背景、状态是否一处可找、主操作是否唯一、正文是否易读、长路径和长模型是否不挤坏布局。预览确认后再实现；本稿不包含已完成的新效果图。



## 7. 关键界面结构审核示意

以下是信息层级草图，不是已完成的App效果图。各页详细整改以第2章为准；字段来自现有能力，新增进度字段见第4和第6章。

### 7.1 Git更新

```text
Git 更新                       神牛 EHS · 3 个仓库   [刷新] [×]
已处理 1/3 · 正在获取 ehs-miniprog                  0:18
[整体条：按已处理仓库数计算]
bro_uhp               master ← origin/master       已快进
  0ad04fa → 新HEAD
ehs-miniprog          master ← origin/master       获取中
  对象 450/1000 · 已接收1.2 MiB（仅有真实数据时）
qiankun-vue3-ehs       master ← origin/master       等待
                                                   [关闭]
```

完成后摘要替换进行中信息，仓库行原位更新，不再另堆一套结果卡片。失败行展示具体原因和对应动作；关闭按钮不叫取消。完整目录可复制，当前目标有歧义时直接展开父路径。无跟踪分支显示“未设置跟踪分支”，不能生成虚假的origin/master。

### 7.2 Codeup认证

```text
Codeup 认证                                      [×]
[本机 Git | 服务器 Git]

认证来源    本机 Git 助手              [↻] [更换认证]
──────────────────────────────────────────────────
仓库访问
[粘贴仓库地址                              ] [检测]
未检测 / 检测中 / 通过 · 时间 / 失败原因
                                           [关闭]
```

服务器页签用“当前SSH账号 + 服务器认证状态”替换本机来源行；不同时渲染两套内容。未同步时Footer提供“同步本机认证…”，已同步时重新同步和移除在⋯中。更换认证后才出现用户名/令牌；Footer按目标显示“保存到 App”或“同步到服务器…”。原App服务器档管理和明确来源选择仍在行菜单，不改变后端认证优先级。刷新配置不依赖URL，访问检测依赖URL；写入授权确认只在当次动作出现。

### 7.3 全页面统一信息分配

| 页面 | 首屏必须看见 | 主操作 | 不常驻内容 |
|---|---|---|---|
| 工作台 | 当前连接、项目、需处理任务、最近会话 | 新任务 | 产品说明、重复快捷入口 |
| 项目与文件 | 当前项目/目录、文件、当前分支、正在进行的操作 | 克隆仓库或当前上下文操作 | 安全保证清单、重复路径 |
| 会话 | 项目分组、对话、实际模型、模式、当前轮状态 | 发送或续聊 | 协议JSON、轮询频率说明 |
| 数据库 | 连接与项目映射、分别的检测状态 | 添加连接 | 与实际结果无关的安装教程 |
| 服务器 | 当前账号、连接状态、执行权限、Runner/Claude版本 | 连接/断开（随状态） | 隔离宣传、默认路径占位说明 |
| Codex/MCP | 配置状态、是否需要重载、动作结果 | 写入配置 | 多份同义重载提示 |
| 应用更新 | 当前版本、检查状态、可安装版本、真实下载状态 | 检查/下载/安装（随阶段） | 稳定通道口号、常驻保证清单 |
| 设置 | 自动连接目标、外观、凭据入口、版本 | 每行就地操作 | 开关状态重复toast、第二个应用更新入口 |

## 8. 执行与维护

本文件已作为开发约束生效。AGENTS.md与贡献指南只引用本文，不复制规范形成多个版本。整改项单独跟踪状态；每项完成需注明证据。视觉基准经用户确认后锁定；发布以实现及实机验收完成为前提。确认后先落基础样式、公共状态组件和操作身份，再实施长操作进度与认证布局，最后按页清理。每次新增页面、状态或文案都按必要性判定与验收门槛复核。

## 任务中心与会话管理

沿用已审核的单入口任务中心 Sheet，宽窗双栏、小窗列表/详情切换；不新增第二个任务侧栏入口。会话按准确项目路径分组，首轮回退标题稳定，搜索范围明确为标题、用户输入和最终结果。归档一次点击可撤销，不取消或删除任务；App 恢复后才可发送。重命名使用 revision，冲突保留输入。任务中心恢复原操作须绑定 ID，核实未知结果只读，不自动重放。接口与使用方式见 [会话管理](SESSIONS.md)。

## 文件夹项目入口

文件页“添加为项目”和会话页“添加项目”共用项目注册表、目录浏览与同一个弹窗。当前目录自动带入；会话页允许选择远端文件夹，普通目录不要求 Git。添加项目不改变 MCP 默认目录，不影响已有会话目录；已登记准确路径不可重复创建或覆盖名称。最近列表在无搜索时保留空项目分组，搜索与归档列表只显示真实匹配会话。新建会话继续沿用全自动默认，普通会话可使用账号 HOME。连接变化后丢弃旧浏览响应，禁止陈旧连接写入；失败在原弹窗修复，不另建项目系统。
