# ADR-013：Agent 决定目录，Core 仅在权限不足时请求本地授权

## 状态

Accepted（产品负责人于 2026-07-21 明确批准下列修订验收案例）。其中**新建 local vault 目录**这一种情形已由 [ADR-016](016-core-creates-the-local-vault-directory.md) 窄化：该目录改由 Core 创建，权限回退请求的对象是它的容器目录。backup source、restore target 与既有 vault 不受影响。

## 日期

2026-07-21

## 背景

EnvRelay 是由 Claude Code、Codex 等外部 Agent 协助用户完成迁移的客户端。Agent 会先结合当前任务和用户意图决定备份源、local vault 或恢复目标目录；Core 的职责是验证并使用这些目录，而不是在每次任务中重新让用户通过 Finder 选择一遍。

当前实现把 `backup_setup` 固定为 native directory picker，因此即使 Core 已经能够读取 Agent 选定的普通目录，用户仍必须重复选择 source。这个交互既重复，也违背 agent-first 产品边界。

当前恢复短语确认还存在独立的可用性问题：首次输入和确认分成两个连续 modal；两次不一致或短语格式错误后，Core 只向 CLI 返回统一的 `MV_CRYPTO_FACTOR_FORMAT_INVALID`，本地界面没有解释、没有保留在输入阶段，也不能直接重试。因此用户看到的是“确认阶段走不下去”，而不是一个可修正的输入错误。

本 ADR 对 [ADR-008](008-session-scoped-user-approved-root-grants.md) 做窄化修订：保留 task/revision/process 绑定的 live handle、no-follow、真实 I/O、重启失效与自动提权禁令；取消“每个可访问目录都必须先经 picker 批准”以及“Agent 不得提交绝对 root path”。

## 决策

### 1. Agent 提交本次任务的目录意图

- pending local action 根据种类接受严格、封闭的目录参数：backup setup 接受 source；Local/Portable backup 另接受 local vault；restore target 接受 target；只有 Local restore unlock 接受已有 local vault。Portable restore unlock 不接收目录，因为 `.mvb` 必须从 Google Drive 下载到 Core-private 临时 staging，校验、解包并在进程内移交，不能把用户目录当作隐式下载缓存。
- 路径必须是当前平台的绝对目录路径，并受长度、NUL、类型和 no-follow 约束；未知、重复、缺失或与 action/provider 不匹配的参数一律拒绝。
- CLI、IPC 和 Core 不在 stdout/stderr、task status、report 或诊断日志中回显路径。Agent 本来知道自己提交的路径，但它不能从响应中枚举其他路径。
- 目录参数只描述意图，不是文件系统 authority。Core 只有在当前 task/action/revision 中实际打开并验证目录后，才能签发 process-scoped root grant。

### 2. 可访问时不弹目录选择器

- backup source 以真实 open/enumerate/read 能力验证；它不要求写权限。
- local vault 与 restore target 以真实 create-new/write/fsync/remove 探针验证写入能力，不能只看 mode bit、ACL 外观或 `access()`。
- 验证成功后，Core 直接把 live handle 绑定到当前 task/action/revision，并继续因素或恢复流程；不得再弹目录选择器。

### 3. 只有权限不足才请求用户授权

- 只有真实 I/O 返回 permission denied / macOS privacy denial 时，Core 才打开 native directory panel，请用户授权 Agent 已选定的那个目录。
- 不存在、不是目录、symlink、路径格式错误、source/vault 重叠或内容/identity 漂移不是“权限不足”，不得用 picker 掩盖。
- picker 只接受与 Agent 请求相同的目录；若用户想换目录，应由 Agent 根据用户新意图重新提交，而不是静默替换本次 source/target。
- Core 不执行 `sudo`、`chmod`、`chown`，也不静默跳过无权访问的文件。

### 4. 短语输入与确认使用一个可重试的本地步骤

- create-new 在同一个 native dialog 中显示两个 secure text fields：恢复短语和确认恢复短语；恢复仍只显示一个 secure field。
- 点击继续后先验证规范格式，再比较两次输入。格式错误时显示本地格式提示；两次不一致时显示“不一致”提示；清空输入并停留在本地步骤，允许用户重试或取消。
- 取消保持任务在 local-action 等待态，provider mutation 为零。输入内容、长度、单词、剪贴板内容和 QR payload 均不得进入日志；诊断日志只记录 `prompt_shown`、`format_rejected`、`confirmation_mismatch`、`accepted`、`cancelled` 等类别。
- 正确短语仍严格遵循 ADR-012 的七词 EFF canonical 规则；本 ADR 不放宽密码学格式，也不改变 2-of-2。

## 修订验收案例

### A. Agent 选定目录

1. **普通 source**：Agent 为 Google Drive backup setup 提交一个存在、无 symlink 且可读的绝对目录；Core 不显示 source picker，直接取得 live handle，随后只显示恢复材料界面。
2. **provider 参数矩阵**：Direct backup 只需 source；Local/Portable backup 需要 source + vault；Direct/Portable restore unlock 不接收目录；Local restore unlock 需要 vault；restore target 只需 target。缺失、多余、重复或错位参数在打开目录和任何 provider/target mutation 前拒绝。Portable restore 必须有 Drive download receipt、bundle digest/length 校验和 Core-private staging 清理证据。
3. **路径边界**：空、相对路径、NUL、超限、文件而非目录、不存在、root/link 替换、symlink/reparse、source 与 vault 重叠均 fail closed；不弹权限授权 picker。
4. **不回显**：CLI JSON、Core 日志、journal task record、task report 与 E2E evidence 不包含 source/vault/target 明文路径；必要的 CLI durable outbox 仅在当前用户私有文件中短暂保存未观察请求，并在观察完成后清除。

### B. 权限不足 fallback

5. **source 读权限不足**：Core 的真实 read/enumerate 返回权限错误后才显示本地授权；用户授权同一目录后重新打开、重验并生成新 session grant，任务继续。
6. **vault/target 写权限不足**：真实 create-new/write/fsync/remove 探针失败后才显示本地授权；授权成功后重新执行完整探针。探针文件必须清理，不得覆盖现有文件。
7. **取消或选错目录**：用户取消，或 picker 选择的不是 Agent 请求目录，任务保持等待且所有 provider/target mutation 为零；旧 handle/grant 不可复用。
8. **无自动提权**：权限失败不会执行或建议执行 `sudo`、`chmod`、`chown`；只允许本地授权或由 Agent 重新选择目录。

### C. 短语确认可用性

9. **一次完成**：同一 dialog 的两个 secure fields 输入相同合法短语后进入 QR 保存步骤；不再出现第二个独立确认 modal。
10. **格式错误可重试**：空、非七词、非 EFF、大小写、双空格、首尾空格等输入显示格式提示并留在本地步骤；修正后无需重新选 source 或重新 OAuth。
11. **确认不一致可重试**：两个字段不同只显示不一致提示，清空敏感输入并允许重新输入；CLI action 不以 `MV_CRYPTO_FACTOR_FORMAT_INVALID` 提前结束。
12. **取消与保密**：任意一次取消都保持 provider mutation 为零；stdout/stderr、Core 日志、journal 和 report 对短语及其长度零泄漏，只允许记录不含秘密的阶段/原因类别。

### D. 状态与 E2E

13. **重启边界**：Core 重启后绝不从旧路径、ID、journal 或 plan 恢复 authority；Agent 重新提交路径，Core 重开 handle、全量重扫并重规划。
14. **四条真实 E2E**：Direct 与 Portable 各覆盖普通目录和 macOS 敏感目录；普通目录不出现 picker，敏感目录只在实际权限不足时出现；两者都完成 Google Drive 上传、fresh restore 和 source/target tree digest 相等。

## 影响

- CLI/IPC/local-action 需要增加 action-kind-aware 的目录输入，但 root grant 仍只由 Core 根据真实 live handle 签发。
- ADR-008 中“Agent 只能在用户先批准的 root 内提交 relative selector”和“每个 root 必须先经 picker”的部分被本 ADR supersede；其 session capability、真实 I/O、TOCTOU、重启失效和禁止自动提权要求继续有效。
- ADR-012 的 2-of-2 密码学合同不变；只修正 create-new 的短语输入交互与错误恢复。
