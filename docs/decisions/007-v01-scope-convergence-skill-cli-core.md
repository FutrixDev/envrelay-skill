# ADR-007：v0.1 交付物收敛为 Skill + CLI + Core，移除自有 Agent 宿主监督栈

## 状态

Accepted；第 5 节中“Gate A/B 与售前独立门治理不变”的条款已由 [ADR-009](009-open-source-assurance-and-local-e2e.md) supersede。Skill + CLI + Core 交付边界继续有效。

## 日期

2026-07-17

## 背景

v0.1 原架构在数据面之外还包含一整套产品自有的 Agent 宿主监督栈：system-owned supervisor/attestor、由 supervisor 管理的 MCP bridge、trusted launcher、三平台 native containment、逐命令 host attestation、enrollment/control channel，以及对应的 8 类 escape、42 项 control transcript、六 runtime bundle 等证据要求。[ADR-006](006-review-timing-and-test-infrastructure.md) 曾把其中"Windows Agent 内核隔离投入"登记为最晚 Task 13D 前的三选一决定。

产品负责人于 2026-07-17 决定：本产品 v0.1 只交付 Skill + CLI + Core；Claude Code / Codex 由用户自行安装，宿主运行环境不属于本产品交付物，宿主监督/证明/隔离不由本产品承担。

## 决策

### 1. v0.1 交付物

- **canonical Skill**（`skills/secure-computer-migration/`）；
- **`migration-vault` CLI**；
- **`migration-core-service`**（加密、上传、解密、恢复，以及配合 Skill 的任务操作）；
- 三平台安装器 `.pkg/.msi/.deb` 保留，但基础 binary 从 4 个收缩为 2 个（CLI、Core service）。

从交付物中移除：`migration-vault-agent-attestor`、`migration-vault-agent-bridge`、trusted launcher、三平台 native containment 组件、enrollment/control channel、逐命令 host attestation。

### 2. 安全模型如实降级并重述

产品不监督、不证明、不隔离 Agent 宿主；Codex/Claude Code 一律视为**不可信调用方**。安全边界收敛为既有 zero-secret 三层 Rust 边界：CLI/Skill 只暴露脱敏、fail-closed 的命令面；短语、恢复密钥、OAuth token、绝对 root path、raw directory handle 与文件内容只在 Core 与可信本地交互中处理。文件 source/target 选择服从 [ADR-008](008-session-scoped-user-approved-root-grants.md) 的窄例外：用户先在当前 task/revision 本地批准 root，Agent 此后只能提交 opaque `root_grant_id` 下的 canonical relative selector；Core 重启即使 grant 失效。该边界已有 trybuild、no-link、public-API 快照与 boundary 脚本背书，且不因本 ADR 放松。

对外表述必须同步降级：不得再宣称逐命令 attestation、宿主 containment 或 escape 证据类能力；相关"已冻结"合同按第 4 节收缩。

### 3. AC-04 重述为功能性验收

同一份 canonical Skill 在用户自装的 Codex 与 Claude Code 上能驱动 CLI 完成 doctor→创建→继续→监控→报告的任务生命周期（QA 环境、synthetic 数据）；验收证据为功能性 transcript 与 CLI/Core 侧记录，不再要求签名宿主 attestation 链。doctor 仍是硬门：`implementation_ready=false` 或 capability 缺失时 Skill 必须安全停止。

### 4. 合同层收缩（排队中的独立任务单元，按 规范→Schema→oracle→fixture 顺序执行）

影响面盘点（2026-07-17）：约 38 个 contract Schema、14 篇 docs、`migration-contracts` 语义 oracle 与约 80 个 fixture 涉及 attestor/bridge/containment/enrollment/launcher。收缩要点：

- `agent-host-attestation-v0.1.md`、`agent-host-platform-containment-v0.1.md`、`enrollment-wire-contract-v0.1.md` 与 `agent-host-wire-contract-v0.1.md` 中监督栈部分标注 superseded；
- `release-support-v1`：基础 binary 4→2，注册 artifacts 同步收缩；
- `release-candidate-v1`：11-member set（3 installer + 6 host runtime bundle）收缩，六 runtime bundle 结构移除；
- E0 Agent 证据分支收缩为功能性 transcript；8 类 escape、42 项 control row、supply-chain host 行随监督栈移除；
- QA 根 purpose 清单同步收缩（agent host attestation/qualification 类 purpose 移除）；
- [外部输入账本](../external-release-inputs-v0.1.md)：Apple ES/NE content-filter entitlement、Windows Hardware Dev Center driver/catalog 签名、Microsoft minifilter altitude 三行随监督栈删除；Codex/Claude 两行收缩为版本 pin + 官方文档 receipt + QA 模型 key；
- **注意**：system attestor 在数据面 QA 中的角色（Dq 私有通道、existing-vault append receipt、network-deny receipt、trusted-time receipt）不随本 ADR 自动删除——它转为 QA-lab 工具属性（不进安装器、不属交付物），具体形态在 Task 14 合同修订时重新定义。

在该任务单元完成前，README、implementation-status 与相关合同中的旧监督栈表述以本 ADR 为准；状态文件 Task 12/13/15 行保持既有事实描述，不得预先改绿。

### 5. 不变项

2-of-2 密钥、加密格式与向量纪律、Google Drive provider、手动下载后离线恢复、3×3 数据面验收、Gate A/B 与售前独立门治理、zero-secret 边界与 fail-closed 纪律全部不变。本 ADR 不解锁任何 Gate，不产生任何发布 authority。

### 6. 关联决定：修订 ADR-006 开发期测试基础设施表（Windows 行）

- Windows 开发期形态从"推迟"改为：**GitHub Actions hosted x64 runner**（私有仓库），只跑 cargo/合同/Go 向量测试；费用触及免费额度时再评估 self-hosted。它不能产生任何验收证据（runner label 不构成 runtime tuple 或路线证据）。
- 负责人另有一台 Windows 宿主机，保留为未来 Milestone A/Task 14 的候选原生 runner（届时须核验其是否在 Windows 11 25H2 x64 家族内）；Ubuntu Desktop 虚拟机（GNOME Wayland）计划运行于该宿主机 Hyper-V，替代 ADR-006 预期的独立采购。

## 后果

- 外部输入账本 17 行中 3 行（Apple ES/NE entitlement、Windows 驱动签名、minifilter altitude）不再阻塞 v0.1，2 行（Codex/Claude host 输入）显著收缩；
- Task 12/13/15 范围显著缩小；ADR-006 登记的"Windows Agent 内核隔离三选一"以本 ADR 关闭（选择：全平台一致地移除自有监督栈，而非仅 Windows 降级）；
- 风险如实记录：产品不再提供宿主侧防线，Agent 误用只能由 CLI/Core fail-closed 边界拦截，宿主本身的行为由用户对 Codex/Claude 的自选信任承担；
- root grant、权限失败、本地重新授权、重启重规划与首次 provider/target mutation 后终止旧 attempt 的语义以 ADR-008 为准；宿主拥有当前 OS 用户权限不等于获得本次迁移的 root grant；
- 合同收缩手术完成前，仓库处于"ADR 已定、合同未同步"的过渡态，任何人不得以旧合同的监督栈条款为由扩大实现，也不得以新范围为由提前宣称 Task 12/13/15 已简化完成。
