# ADR-008：v0.1 采用会话级、用户批准的本地根目录授权

## 状态

Accepted；session root grant 与真实 I/O 权限模型继续有效。仅“本 ADR 不解锁 Gate A/B”的治理措辞由 [ADR-009](009-open-source-assurance-and-local-e2e.md) supersede。

## 日期

2026-07-19

## 背景

Migration Vault 的文件数据面只有两条基本路径：备份时从本地目录读取普通文件，经 Core 加密后上传；恢复时下载密文，经 Core 认证、解密后写入本地目标目录。source/target 目录可由用户自行安装的 Codex、Claude Code 等外部 Agent 提议，但 Agent 不是本地文件系统权限主体，不能仅凭路径字符串授予 Core 访问权。

此前 v0.1 设计要求为 source/target 根建立跨进程持久文件系统身份：macOS 依赖 APFS volume UUID、persistent file ID、extended ACL 准入和对应原生 FFI，并允许 Core 重启后把新打开的 handle 与旧计划重新绑定。该模型超出了首版产品所需的权限语义，也把特定文件系统身份能力变成了普通文件迁移的前置条件。

产品负责人决定把 v0.1 收敛为会话级根目录授权：用户批准当前任务要访问的根，Core 在当前进程中持有真实目录 handle；Agent 只能在已批准根内选择相对位置。Core 重启后不尝试证明重新打开的是同一物理目录，而是要求重新授权、重新扫描并重新规划。

## 决策

### 1. 用户批准产生会话级 root grant

每个 source/target root 都必须由用户通过 Core-private 本地动作，在当前 task、task revision 和用途（source 或 target）下明确批准。Core 从该动作取得并验证一个已经打开的 directory handle，随后使用 OS CSPRNG 产生不可预测、非零、opaque 的 `root_grant_id`。

<code>root_grant_id</code> 只是在当前 Core 进程内索引 live handle 与批准记录的 opaque selector，不是 bearer credential。Agent 可以在 safe DTO 中取得该 ID 以引用已批准 root，但仅持有、猜中、重放或从日志取得该 ID 均不能创建授权；Core 还必须同时验证当前 task、revision、purpose、process instance、grant record 与 live handle。不会离开 Core 的是批准记录、raw handle 和绝对 root path；ID 不写入持久 authority，Core 重启后即使重放同一 ID 也必须拒绝。

### 2. Agent 只能在已批准根内提交相对选择器

Agent 可以提交已批准 `root_grant_id` 下的 canonical relative selector，以选择该根本身或根内的 source/target 子目录。该输入面不接受绝对路径、drive/UNC 路径、`.`、`..`、空组件、NUL、反斜杠逃逸、非 canonical 别名或未获批准的另一根。

Core 必须从 grant 的 live directory handle 出发，逐 component 进行 capability-relative、no-follow/reparse-aware 的解析；不得用字符串前缀、`canonicalize` 后比较、当前工作目录或环境变量证明 containment。任何 symlink、junction、reparse point、mount/volume 意外切换或解析中 handle 漂移都使当前选择失败，不能逃出用户批准的根。

这是对 ADR-001 原有“Agent 零路径输入”的窄化调整：Agent 仍不能提交绝对 root path、任意 host path 或 caller-selected handle，但可以提交用户已批准根内的 canonical relative selector。文件内容、恢复因素、OAuth token、绝对用户名路径以及未清洗的 provider/Git 秘密仍不得进入 Agent。

### 3. 以真实 I/O 判断权限不足

Core 不把 `access()`、mode bit、owner、ACL 外观或 Agent 声明当作可读写 authority：

- backup 必须以 grant 的 live handle 实际打开、枚举并读取 source；
- restore 必须以 grant 的 live handle 实际 create-new、写入并 fsync target 内的 staging/parent；
- 任一真实操作返回权限不足时，Core 暂停当前 attempt，签发绑定 task/action/revision 的本地动作，要求用户在本机完成授权或重新选择；
- Core 不自动执行或建议 Agent 执行 `sudo`、`chmod`、`chown`，也不把失败降级为跳过后继续。

用户完成本地动作后，Core 必须取得新的 live handle、递增或核对相应 revision，并重新验证操作；旧 completion、旧 handle 或旧 grant 不能恢复权限。Agent 只能看到脱敏状态与 `open_local_action`，不能伪造“用户已授权”。

### 4. Core 重启使 grant 和旧计划失效

`root_grant_id`、批准记录和 live handle 都是当前 Core 进程实例的内存 authority。Core 退出、崩溃、升级或丢失 handle 后，所有 root grant 立即失效；它不得从 journal 中的 ID/hash、旧 plan、旧 path 或临时文件恢复 grant。

- 在 backup 的首次 provider mutation、或 restore 的首次 target mutation发生前，当前 task 可以回到本地动作：用户重新批准 root，Core 全量重扫、重新做 policy coverage、生成新的 plan/revision，再继续同一 task。旧 scan/plan/grant 全部失效，不能被重新 mint 为 authority。
- 首次 provider mutation 或 target mutation一旦发生，Core 重启后旧 attempt 必须稳定终止；不得把新 grant 硬绑到旧 plan、自动替换任务或猜测继续点。只有用户明确发起一个新 task，才可重新授权、重新扫描、重新规划并按既有 immutable/conflict 规则处理遗留结果。

这里的 provider mutation 包括 create/upload/update/copy/move/delete 等远端写操作；target mutation 包括创建目录、创建 staging 文件、写入、rename/replace 等目标树写操作。只读 provider discovery 和 source read 不属于这两个提交边界，但其结果在重启后也不恢复 authority。

### 5. 移除持久文件系统身份作为 v0.1 准入条件

v0.1 不再要求或承诺：

- 跨 Core 进程稳定的 physical root identity；
- macOS APFS-only 准入、volume UUID 或 persistent file ID；
- source/target root 的 extended ACL 准入分类；
- 为上述 identity/ACL 单独引入或提前完成的 macOS FFI。

这只移除 root grant 的持久 identity/ACL 前置，不表示删除整个 platform FFI，也不放松其他原生边界。Windows handle/reparse 操作、native local action、secure zero、安装/IPC 或未来确有需要的原生 primitive 仍按各自合同评审。

### 6. 不变的文件系统安全边界

下列要求不因本 ADR 放松：

- Core 的 trust-state、journal、plan spool、密文 staging、credential 与安装目录仍必须执行当前用户/系统边界所需的 owner、mode、ACL、link-count、no-follow 和 create-new 检查；本 ADR 只取消用户所选 source/target root 的 extended ACL 准入；
- 当前进程内必须持续重验 live root、parent 与 leaf handle 的类型和 identity marker，发现替换、symlink/reparse、mount/volume 漂移或 source 内容变化立即停止；这些检查用于同一 attempt 的 TOCTOU 防护，不得冒充跨重启持久身份；
- restore 继续使用目标目录内 create-new staging、内容认证、file fsync、同目录原子 rename 与 parent directory fsync；默认不覆盖已有文件，任何覆盖策略仍须独立、明确的本地批准；
- 绝对路径、相对逃逸、symlink/reparse 穿越、跨文件系统非原子 fallback、自动提权与静默跳过继续保持 fail closed；
- CLI/Skill/Agent 不能取得 raw directory handle、任意 filesystem API 或 Core-private completion/capability。

plan/manifest 中需要保留的 `root_ref`、`root_id` 或 `target_ref` 只表示 task/grant 下的 opaque 逻辑映射与内容绑定，不再表示一个可跨进程重新识别的物理文件系统对象。它们不能单独恢复 live grant。

## 被拒绝的方案

### 继续要求三平台持久 physical root identity

它可以在重启后尝试复用旧计划，但需要文件系统专属 API、跨平台 identity 语义和更复杂的 ACL/FFI 审计。首版选择以重新授权、重扫、重规划换取更小且更清晰的权限边界。

### Agent 有 OS 权限即可直接访问任意目录

Agent 与 Core 可能本来就能读取大量用户目录；OS 进程权限不等于用户对本次迁移的批准。没有 root grant 时直接读取或写入会扩大任务范围，因此拒绝。

### 权限失败后自动提权或修改目录权限

自动 `sudo`、`chmod`、`chown` 会改变用户系统状态并扩大影响面。v0.1 只暂停并请求本地用户动作，因此拒绝。

## 后果

- source/target root 授权从“持久物理身份”变为“task/revision/process 绑定的 live capability”；Core 重启会增加一次用户重新授权，但不再把 APFS identity/extended ACL/identity-only FFI 作为普通文件迁移前置。
- Agent 获得的新增信息面仅为 opaque grant ID 与该 grant 内的 canonical relative selector；绝对 root path 和 Core handle 仍不进入 Agent。
- 状态机、security format、plan/root-ref 语义、错误合同、Skill/CLI 合同、实现状态和测试向量必须在后续独立任务中按本 ADR 收缩。本 ADR 本身不解锁 Gate A/B，不使真实用户数据路径可用，也不改变当前 `implementation_ready=false`。
- ADR-001 的 Agent 输入边界与 ADR-007 的 Skill/CLI/Core 安全模型在 root selection 上以本 ADR 为准；其他秘密隔离、zero-secret 三层边界和 fail-closed 纪律不变。
