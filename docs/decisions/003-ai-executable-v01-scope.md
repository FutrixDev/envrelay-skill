# ADR-003：冻结 AI 可执行的 v0.1 范围

- 状态：Accepted
- 日期：2026-07-14

## 背景

Migration Vault 的长期规范覆盖文件、软件、凭据、Git、多个云盘和完整桌面产品。如果直接让 AI Agent 按长期规范并行实现，任务边界、平台承诺和机器输出会在开发中继续漂移，Codex、Claude Code 与 Core 也容易对同一状态作出不同解释。

v0.1 必须先证明一件完整而可验收的事：用户能在 macOS、Windows 11 和 Ubuntu Desktop 24.04 之间，通过 Google Drive 备份并恢复普通文件和目录；数据离开本机前已经加密；Codex 或 Claude Code 可以通过同一份 Skill 调用本地 Core 自动推进任务，同时不能越过本地可信门禁。

## 决策

### 1. 唯一交付范围

v0.1 只交付：

- Rust Core 和公开 CLI；
- 最小本地可信 UI/TTY；
- 普通文件和目录的扫描、加密备份、恢复及验证；
- Google Drive 云端 provider；
- 手动下载密文目录后的 local provider 离线恢复；
- system-owned supervisor/attestor、由supervisor管理的MCP bridge、可信launcher与三平台native containment；
- 把CLI、Core、attestor、bridge、registration与同一份canonical Skill一并安装的macOS `.pkg`、Windows `.msi`、Ubuntu signed-APT `.deb` 三个原生安装器；
- 一份同时适用于 Codex 和 Claude Code 的 canonical Skill。

不实现软件、凭据、Git、整机镜像、其他云盘、其他 Linux 发行版和 Tauri 桌面端。这些能力不能出现在 v0.1 的成功报告或支持承诺中。

### 2. 平台基线

v0.1 的运行时 Tier 1 候选为：

- macOS 26，Apple Silicon `aarch64`、本地 APFS、Aqua；
- Windows 11 25H2（servicing build `26200.x`），`x86_64`、本地 NTFS、interactive desktop；
- Ubuntu Desktop 24.04 LTS，`x86_64`、GA 6.8 kernel、本地 ext4、GNOME Wayland。

每个 artifact-only candidate 必须从完成当前安全补丁并重启的三个真实 runner 各采集一个 exact runtime tuple，并只对这些 candidate 专属 JCS/hash 采证；Schema 只固定上述 target family，不能把同一家族解释成范围支持。Task 13A还必须把target lifecycle接入candidate/CAS图：Windows/Ubuntu绑定带可信抓取/解析receipt的厂商官方原件、family coverage、与C tuple相等的observed exact build和保守转换的`support_end_exclusive_utc`；Windows tuple未绑定edition且`VER_NT_WORKSTATION`不能排除IoT Enterprise，因此固定加载25H2 Home/Pro/Pro Education/Pro for Workstations、Enterprise/Education/Enterprise multi-session、IoT Enterprise三份官方生命周期原件并取最早截止时间，Ubuntu月份级LTS页面不得冒充覆盖exact dpkg build。macOS因Apple不公布同等形式的未来EOL，绑定官方当前版本/安全发布原件、受管runner更新状态、无deferral/beta policy、exact tuple，以及由Security/Release owner在仓库外签批且由bootstrap cap限制的`rolling_current_release`重验/监控/撤销政策，不得虚构`support_end`。C冻结与final T有效期必须受对应exclusive fixed bound或`revalidate_by`约束。任一 exact patch/build/kernel、Apple当前版本/适用更新状态、滚动窗口/监控或受支持 family 变化都必须创建新 candidate、重建产物并全量重采证；family/contract变化还要升级Schema。macOS Intel `x86_64` 保留构建目标，但只能标为 `compile_only`，不得写成运行时已支持。

[`../release-support-v0.1.json`](../release-support-v0.1.json) 是当前计划/未支持状态的机器可读视图，不是发布 authority。未来 production 支持只能由 candidate 内嵌 tuple、installer/runtime closure、typed evidence、final T 与 installed CAS 的完整图共同授予；单独修改该状态文件、提供Schema-valid tuple或非零hash都不能解锁支持。

平台族验收必须覆盖 macOS、Windows、Ubuntu 的 3 × 3 共 9 条 source-target 路线，包括同平台恢复。每条路线只有在真实运行、密文篡改失败和文件验证证据都存在后，才能从 `planned` 改为 `verified`。

### 3. 加密边界

文件内容、文件名和相对路径必须在离开本机前加密。Google Drive、Agent 输出、日志和遥测不得出现明文路径、文件名、账户、OAuth URL/token、恢复因素或底层 stderr。

加密格式继续采用 `migration-vault-format/v1` 和现有 2-of-2 恢复因素设计。任何密码学或持久化格式的破坏性调整都必须先更新规范、Schema 和独立测试向量。

### 4. Agent 的执行边界

Codex 和 Claude Code 只通过 canonical Skill 发现安装器管理的 MCP tools：host 连接固定 managed MCP bridge，supervisor 再为每条请求启动短生命 broker，由 broker 使用已安装的稳定 CLI 完成创建、继续、监控、重试、取消和读取脱敏报告。Agent 不得直接调用 CLI、shell 或 Core IPC。Agent-facing JSON 必须符合严格 Schema，未知 major 和未知字段一律拒绝。

Agent 可以发起任务和提出枚举决策，但不能：

- 提供或读取备份源路径、恢复目标路径；
- 提供恢复短语、恢复密钥或 OAuth 凭据；
- 伪造 `local_action_completed`；
- 绕过 `waiting_for_local_action`；
- 执行 Core 未声明的任意命令、URL 或参数。

路径、恢复因素、系统浏览器授权和确认动作只能由本地用户通过可信 UI/TTY 完成。Core 是状态迁移和门禁校验的唯一权威。

### 5. 合同和失败策略

- 公共 CLI 响应固定使用 `migration-vault-cli/v1` 信封；
- 成功数据只能是合同列出的封闭 payload 联合；
- 失败只返回稳定 `code`、`retryable`、`message_key` 和安全参数；
- 未知合同 major、未知字段和违反语义约束的数据 fail closed；
- 任务状态、支持矩阵和最终报告都必须由专用 Schema 与语义校验器验证。

ID 不是同一种编码：Task、Vault、Snapshot、Manifest 等 128-bit ID 使用 26 字符 RFC 4648 lowercase base32 no-padding；最后一组只有 3 个数据 bits、低 2 bits 必须为零，因此末字符只能是 `a/e/i/m/q/u/y/4`。Commit/Object/digest ID 使用 64 字符 lowercase hex。`commit-v1` 同时认证 `parent_commit_id` 与 `parent_snapshot_id`；v0.1 没有 `head-v1`、`previous_commit_id` 映射或 mutable current pointer。

## AI 实施约束

每个实现任务必须能够由 Agent 在一个明确的 red → green → refactor → verify 循环中完成：

1. 先增加失败测试或独立 fixture；
2. 只实现让当前合同通过的最小生产行为；
3. 运行 workspace 测试、lint 和依赖策略检查；
4. 用机器可读报告证明目标，而不是依赖日志文本或 Agent 自述；
5. 独立提交，不在同一提交提前实现后续 Task。

## 后果

这个决定主动放弃首版的功能广度，换取可由 AI 可靠执行、可跨平台复现并能逐条验收的闭环。长期规范仍可作为 v0.2+ 参考，但它不能扩大 v0.1 的验收口径。任何新增范围都必须先更新本 ADR 或创建后继 ADR，并同步修改 release support manifest、合同和测试。
