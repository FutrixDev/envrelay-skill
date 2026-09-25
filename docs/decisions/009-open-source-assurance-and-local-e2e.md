# ADR-009：以开源可复现验证替代外部 Gate A，并把 local backup 纳入 v0.2

## 状态

Accepted（产品负责人于 2026-07-19 批准决策及配套行为案例并授权实施）

## 日期

2026-07-19

## 背景

Migration Vault 的核心价值是把用户选择的目录加密备份到密文 provider，再在用户授权的目标目录中认证、解密和恢复。当前合同却只允许 Google Drive backup 和 local restore；同时，历史设计把仓库外独立评审 `R`、protected-signing `Q/S` 及 authority-producing verifier 设为 production crypto、真实用户数据和产品 readiness 的硬前置。这使本机已经存在的 scanner、加密、local provider 与 restore 原语无法通过正式 Skill/CLI/Core 路线组成可测试产品。

产品负责人确认：

- `migration-vault backup --provider local` 是 v0.2 正式产品能力，不是 QA 旁路；
- 第一阶段在同一台 Mac 上完成完整 backup→local ciphertext directory→restore E2E；物理双 Mac 是后续可搬运性证据，不是同机功能验收的前置；
- 测试同时覆盖普通目录和用户明确授权的敏感/受保护目录；用户亲自完成本机授权；
- 当前不进行外部安全审计，删除 Production Gate A 外部批准及 `R/Q/S` 作为开发、测试、运行或发布 readiness 的要求；
- 核心加解密格式、实现和向量验证会开源，允许公众独立复核；
- Apple、Google 或其他集成 credential 由产品负责人按需在本机安全提供，不进入仓库、argv、日志或 Agent channel。

## 决策

### 1. Local provider 同时支持正式 backup 和 restore

v0.2 canonical CLI 必须包含：

```text
migration-vault backup create --agent --provider local --format json
migration-vault restore create --agent --provider local --format json
```

两条路线都必须经过同一套正式 task、authenticated journal、用户本地 root approval、session grant、plan、Core crypto 和 local provider binding。不得把现有 `cfg(test)` C2 runner、固定密钥或隐藏命令包装成产品成功路径。

新增命令发布为 CLI grammar v3；v2 保持历史字节不变。v3 只能显式增加 local backup variant，不得把 provider 解析放宽为任意字符串。

### 2. 同一台 Mac 是首个产品 E2E 环境

同机 E2E 使用两个不同的 source/target 目录和一个独立 local ciphertext directory，必须从 canonical Skill/CLI 调用正式 Core service。它至少证明：

- source 扫描、加密、密文写入、fresh reopen、认证、解密、target staging、原子发布和最终复核全部通过；
- restore 不共享 backup 的 source handle、明文内存对象或未持久化 authority；
- provider 中不存在可直接识别的明文文件内容；
- 普通目录与用户明确授权的敏感/受保护目录都能按同一合同完成，或在用户拒绝/权限仍不足时安全停止；
- source、local ciphertext vault 与 restore target 三个 live root 彼此不同且不存在祖先/后代重叠；
- 证据只包含脱敏 task ID、计数、阶段、稳定错误码和摘要，不记录绝对路径、文件名、内容、恢复因素或 credential。

物理双 Mac 仍用于证明密文目录可搬运及两台真实系统间的恢复，但不再阻塞首个同机产品 E2E。

### 3. 删除外部 Gate A / R/Q/S 运行与发布前置

从当前生效的产品规范、Schema registry、fixture、状态机、CLI/Core 错误路径、release readiness 和实现计划中移除下列要求：

- 仓库外独立 review `R` 是 production crypto 的前置；
- protected-signing request/response `Q/S` 是真实数据或 runtime authority 的前置；
- external keyring、revocation、rollback、trusted-time、single-use replay receipt 或 authority-producing attach/finalize verifier 才能构造 product capability；
- `GateAVectorAuthorityUnavailable`、`Gate A CLOSED` 或等价状态阻止正式 task、local provider 或用户授权目录测试。

这些对象可以保留在历史 ADR、Git 历史或明确标为 superseded/archive 的研究材料中，但不能继续被 active contract、代码或 readiness 判断消费。ADR-006 中“售前独立门阻止真实数据/Beta/销售”、ADR-007 中“Gate A/B 与售前独立门保持不变”以及其他同义条款由本 ADR supersede。

外部审计以后如恢复，必须另立 ADR，说明范围和引入时点；不得以旧 Gate 文本静默恢复为产品 authority。

### 4. 以开源、可复现的内部 assurance 取代签名审批

产品 readiness 由可重复执行的工程证据决定，不由外部签名对象决定：

1. 核心加解密格式、Rust 实现、独立 Go 向量生成器、正负向向量和验证说明公开；
2. Rust 不能生成自己的权威 expected vectors，仍由独立 Go 实现产生并逐字节交叉验证；
3. AEAD 认证失败零明文、wrong-factor、nonce/RNG、corruption、truncation、reorder、resource-limit、parser fuzz/property 测试通过；
4. session root grant、三根非重叠、handle-relative no-follow、source drift、permission denied、本地重授权、默认 no-overwrite、explicit KeepBoth、atomic publish、fsync 和 crash/restart 测试通过；
5. canonical CLI/Core local backup→restore E2E 在普通目录和用户授权敏感目录通过；
6. trusted-core boundary、四 target 编译、全量测试、格式化、工作树指纹和发布构建可复现；
7. 对外明确写明“核心加密已开源并通过项目测试，但当前没有第三方安全审计”，不得把开源或自测描述成独立审计。

正向 probe/readiness 使用实际能力与证据字段，例如 `internal_security_checks_passed`、`local_user_test_ready` 和固定为事实值的 `externally_audited=false`；不得再用 Gate A receipt 代替运行能力。

内部测试失败、能力缺失或证据不完整时仍必须 fail closed，`implementation_ready=false`。删除外部 Gate 不会自动把任何当前 stub、测试桥或合同改成 ready。

### 5. 敏感目录只在用户明确授权后测试

普通目录和敏感/受保护目录共用 ADR-008 的 session root grant：用户在当前 task/revision/purpose 下批准 root，Core 持有 live handle，真实 open/read/create/write/fsync 判定权限。权限不足时只打开本地动作等待用户授权或重新选择；不得自动执行 `sudo`、`chmod`、`chown` 或 ACL 修改。

自动测试只创建非敏感或“敏感形态”的生成语料。真实敏感/受保护目录案例必须由产品负责人手动选择并授权；测试工具不得枚举或输出其绝对路径、文件名、内容或秘密，也不得把这些值写入 evidence。

### 6. Credential 由用户在本机安全提供

Apple signing、notarization、Google OAuth 或其他 credential 只通过 Core-private 本地 surface、操作系统 credential store 或受控本机发布环境提供。Agent、Skill、CLI argv/env、仓库 fixture、测试日志和聊天内容均不得接收 credential 原文。

Local-provider 同机 E2E 不依赖 Google OAuth、Apple notarization或第二台 Mac，因此这些 credential 不阻塞第一阶段实现。

## 被拒绝的方案

### 继续等待外部 R/Q/S 后再接产品链

这与当前单人项目、开源核心算法和先完成同机产品 E2E 的目标不匹配。外部审计可以以后进行，但不再是当前 runtime authority。

### 把现有固定语料 C2 暴露成隐藏 CLI

它使用 test-only authority、固定密钥和 synthetic journal，无法证明正式 task、用户授权、恢复因素或崩溃语义；暴露它会形成 QA bypass，因此拒绝。

### 使用真实敏感目录做无人值守自动化

它会扩大数据暴露面并绕过用户授权。敏感目录必须由用户在本机动作中明确选择，自动证据保持脱敏。

## 后果

- 当前实现不再被外部 Gate A/R/Q/S 阻止，但仍被真实未实现的 IPC、task/journal、native approval、production key authority、provider binding 和 target writer 阻止；这些必须逐项实现并通过案例。
- 项目承担“没有第三方审计”的风险，并以公开源码、独立向量实现、可复现测试和透明声明降低风险。
- 同机产品 E2E 可以先于物理双 Mac、Google Drive 和安装器签名完成。
- 历史 Gate 文档不会被冒充为当前要求；active docs/contracts/code 必须按本 ADR 分批删除或标记 superseded。
