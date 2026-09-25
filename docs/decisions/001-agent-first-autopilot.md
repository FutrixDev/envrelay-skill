# ADR-001：采用 Agent-first Autopilot 与本地策略强制边界

## 状态

Accepted

## 日期

2026-07-13

## 背景

Migration Vault 的用户价值不是展示一份迁移清单，而是尽量少打断用户地完成换机。原 Draft v1 只允许 Skill 读取分类计数和错误码，把路径、软件名称、缓存判断、计划选择和冲突处理全部留给本地 UI。这个边界安全，但 Agent 无法判断可重建缓存、选择跨平台软件、处理失败或执行修复，因此不能实现“一次授权，自动迁移”。

同时，直接把文件名、配置内容、私钥、原始路径和任意命令暴露给模型会引入隐私泄漏、提示词注入和供应链安装风险。纯 Agent 或纯本地规则都不能同时满足低介入与安全要求。

## 决策

产品采用四层架构：

1. **Skill/Agent**：理解迁移目标，消费结构化决策上下文，提交动作建议，监控、重试和解释结果。
2. **Agent Decision Context**：只提供 Schema 白名单化的语义事实，例如 adapter、公开 canonical software ID、缓存证据、大小区间、风险、可逆性和允许动作；不提供秘密值、原始文件内容、绝对用户名路径、私有包名或未清洗 Git URL。根目录选择的唯一窄例外由 [ADR-008](008-session-scoped-user-approved-root-grants.md) 规定：Agent 可以引用当前 task/revision 下用户已批准的 opaque root grant，并提交该根内的 canonical relative selector。
3. **Autonomy Policy Engine**：用户在本地一次批准迁移策略；引擎验证 Agent 建议和最终计划是该策略的子集，并拒绝越权、过期上下文、自由命令和新增高风险动作。
4. **Local Core**：在本地扫描、解析秘密、加密、上传、安装、写入、验证和持久化 journal。恢复因素、OAuth token、私钥和明文清单永不进入 Agent。

`SafeSummary` 继续承担低信息量的状态汇报；新增独立、分页、短时有效的 `AgentDecisionContext`，不把任意 metadata 塞进 `SafeSummary`。Agent 的输出使用 `AgentDecisionProposal`，通常只能引用不透明 decision ID 和该条目允许的枚举动作，不能携带 shell、绝对路径或 provider URL。文件 source/target 的选择仅允许 ADR-008 冻结的 `root_grant_id + canonical relative selector`；它不能扩大到未批准根、任意 host path 或 raw directory handle。

context token 不是权限令牌：proposal 首次被接受后任务 revision 立即递增，旧 token 失效；网络重试只允许相同 proposal 内容幂等返回，使用同一 token 改写动作必须拒绝。`plan_sha256` 和 `policy_sha256` 也只是防止 stale/confused-deputy 的内容选择器，不是 bearer credential；Core 必须同时查到本机可信 UI 写入的 approved policy record，Agent 仅拿到 hash 不能创造授权。

用户一次批准 `AutonomyPolicy` 后，策略覆盖的可逆动作不再逐项确认。Autonomy Policy 不替代 ADR-008 的 task/revision 级 root grant；Core 重启后 root grant 失效并必须重新取得。以下事项始终是本地安全门：

- 设置或输入两个恢复因素；
- 批准 source/target root，以及实际 read/create/write 权限失败后的重新授权或重新选择；
- 云盘 OAuth、第三方 MFA 和账户登录；
- UAC、sudo、Full Disk Access 等操作系统授权；
- 覆盖现有数据、启用未知仓库、安装未知来源软件；
- 功能替代软件、兼容层/虚拟化、许可证和设备绑定授权；
- 无法证明可重建且影响较大的未知数据。
- 需要用户在系统外部工具中完成并确认的 native handoff。

未知用户数据的安全默认是保留；只有本地确定性规则或已审核 adapter 能证明内容可重建时，Agent 才能在策略覆盖下排除。文件名、README、配置值和第三方输出全部作为不可信数据，不作为 Agent 指令。

## 备选方案

### 只保留 SafeSummary

- 优点：泄漏面最小。
- 缺点：Agent 只能监控，无法做缓存、软件和修复决策。
- 结论：不满足 Autopilot 产品目标。

### 把完整扫描清单交给 Agent

- 优点：上下文最丰富。
- 缺点：泄漏路径、项目和凭据元数据；文件内容可实施提示词注入；模型可生成危险命令。
- 结论：拒绝。

### 完全使用本地确定性规则

- 优点：容易复现和审计。
- 缺点：无法处理跨平台替代、失败修复和用户意图；每个例外都需要 UI。
- 结论：规则作为证据层保留，但由 Agent 在策略内编排。

## 后果

- 需要新增版本化的 Autonomy Policy、Agent Decision Context 和 Decision Proposal 契约。
- 计划必须绑定 policy hash、context revision 和 decision provenance；计划变化后重新做策略覆盖检查。
- Skill eval 必须同时测试秘密泄漏、提示词注入、越权动作、过期上下文和错误软件映射。
- 商业价值从“提供加密 UI”转向持续维护的 adapter、软件映射、自动决策、失败修复和可验证完成。
- “一键迁移”定义为一次策略授权后自动执行；操作系统和第三方身份系统强制的交互不伪装成可绕过步骤。
- source/target root 授权、Agent 相对选择器、Core 重启与副作用边界以 ADR-008 为准；Agent 的 OS 用户权限本身不能替代本次任务的用户批准。
