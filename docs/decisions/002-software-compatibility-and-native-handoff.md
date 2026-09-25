# ADR-002：不兼容软件分级处理，并与系统原生迁移工具协作

## 状态

Accepted

## 局部取代

**恢复执行部分**——registry 覆盖下的自动安装、executable+argv 安装路径、Action Center 安装门与签名 mapping registry 分发——已由 [ADR-017](017-skill-orchestrated-software-inventory-and-file-plane-credentials.md)（2026-08-26）取代：软件恢复改为 Skill 编排的清单模式，安装由 Agent 在用户逐条确认下自行执行，Core 不再持有安装 authority 或 mapping registry。本 ADR 的**五级兼容分级、原生迁移工具协作与"显示名模糊匹配不得生成安装步骤"**降级为 Skill 侧判断指引，继续有效。

## 日期

2026-07-13

## 背景

跨 macOS、Windows、Linux 迁移时，软件可能不存在目标平台版本、包名不同、配置格式不同、依赖兼容层，或根本没有可信替代品。按相似名称自动安装会造成包名劫持、错误软件和工作流破坏。

同平台迁移也已有强系统能力。Apple 官方说明 Migration Assistant 可在 Mac 之间迁移文档、应用、用户账户和设置，设置助理还会自动转移 Keychain；不兼容应用仍可能无法迁移或使用。Migration Vault 若把自己定位成更好的 Mac 克隆工具，会在系统权限、Keychain 深度、免费分发和原生体验上处于明显劣势。

## 决策

### 软件兼容性分级

canonical mapping registry 为每个源软件和目标平台记录以下一种关系：

1. `same_product`：同一产品有目标平台原生包，例如 VS Code。
2. `official_port`：同一厂商提供正式目标平台版本，但配置可能需要转换。
3. `functional_alternative`：功能相近但不是同一产品，例如 iTerm2 到 Windows Terminal。
4. `compatibility_layer`：只能通过 WSL、虚拟机、容器、Rosetta 或其他兼容层使用。
5. `preserve_data_only`：没有经过审核的目标软件，只保留数据、配置和来源说明。

只有 registry 中经过真实验证的 `same_product` 和 `official_port` 能在 Autonomy Policy 覆盖下自动安装。`functional_alternative` 会由 Agent 汇总推荐，但需要一次本地选择；`compatibility_layer` 涉及系统功能、性能和安全边界，始终需要本地安全门；`preserve_data_only` 不猜测替代品、不执行未知下载。

每个 mapping 必须记录目标 package ID/source、架构、配置策略、数据格式兼容性、许可证状态、验证方法、mapping 版本和证据等级。显示名模糊匹配只能生成候选，不能生成安装步骤。

内置 mapping registry 随签名应用发布；未来若支持在线 mapping pack，必须独立签名、固定版本、校验发布者并防止回滚。任务开始后固定 registry version，更新不能在执行中改变 plan。

功能替代/兼容层的本地选择可以保存为窄偏好，绑定源 canonical ID、目标平台、目标 package/source、mapping version 和权限范围；相同版本的后续任务可直接视为“本地门禁已完成”。package/source/权限/兼容方式或 mapping version 任一变化时必须重新选择。这样只在第一次真正改变用户工作流时打断一次，而不是每次迁移重复询问。

### 原生迁移工具协作

- **Mac→Mac 且两台设备/Time Machine 可用**：默认建议先用 Apple Migration Assistant 完成账户、应用、设置和 Keychain 基线迁移；Migration Vault 在迁移前保存开发环境语义清单，在迁移后执行缓存清理、Homebrew/运行时重建、Git/SSH/GPG/构建工具检查、缺失软件补装和验证修复。
- **Windows→Windows**：若目标版本、账户类型和设备厂商存在经过验证的 Microsoft/OEM 原生迁移路径，则用它完成其明确支持的基线范围，再由 Migration Vault 重建开发环境；没有可靠能力证据时不假定“系统已经迁完”。
- **跨平台、异步换机、旧机即将交付或希望保留可审计加密快照**：Migration Vault 执行完整备份和恢复。

产品不读取或操控 Apple Migration Assistant 的内部状态，也不声称能验证其全部系统迁移结果；它只记录用户选择的 handoff、等待用户在本地完成，然后重新扫描目标机并比较迁移前 inventory。

## 相对 Apple Migration Assistant 的优势边界

Migration Vault 不宣称在通用 Mac→Mac 搬家上优于 Apple。它的差异是：

- macOS、Windows、Linux 之间的跨平台恢复；
- 识别软件和开发配置的语义，而不是只搬运应用/文件；
- 对 Homebrew、winget、apt、语言运行时、Git、SSH/GPG 和构建工具进行重建与实际验证；
- Agent 在策略内处理缓存、软件映射、失败重试和修复，减少用户判断；
- 使用用户自有存储保存端到端加密、可延迟恢复的快照；
- 公开格式和 CLI，厂商停止服务后仍可恢复；
- 给出逐项 `verified / restored-unverified / reauth-required / manual / failed` 结果。

Mac→Mac 用户购买的价值因此是“开发环境迁移体检与自动修复”，而不是重复购买一个系统已有的文件搬家工具。

## 备选方案

### 所有软件都尝试寻找近似替代

- 优点：自动化比例看起来更高。
- 缺点：改变用户工作流并放大供应链风险。
- 结论：拒绝。

### 完全不处理不兼容软件

- 优点：实现简单。
- 缺点：跨平台迁移的核心价值消失。
- 结论：保留 `preserve_data_only` 兜底，但提供受控推荐和兼容层方案。

### 与 Migration Assistant 正面竞争

- 优点：统一宣传“一套工具完成全部迁移”。
- 缺点：系统工具在 Mac 账户、Keychain、系统设置和原生权限上更强且免费。
- 结论：采用 native handoff + post-migration repair。

## 实现与验收影响

- 源 manifest 只记录 canonical identity、来源和 identity evidence；软件 registry 与目标相关 restore plan 记录 compatibility kind、configuration/data strategy、evidence grade 和 native handoff。兼容性不得在未知目标平台时提前固化。
- E2E 至少覆盖同产品、官方端口、功能替代、兼容层和无替代五类。
- Mac→Mac 发布验收同时覆盖纯 Migration Vault 和 Migration Assistant handoff 后的 inventory diff/repair。
- 官方依据：
  - [Apple：使用“迁移助理”转移到新 Mac](https://support.apple.com/en-us/102613)
  - [Apple：将钥匙串拷贝到另一台 Mac](https://support.apple.com/zh-cn/guide/keychain-access/kyca1121/mac)
  - [Apple：从 Windows PC 转移到 Mac](https://support.apple.com/en-us/102565)
