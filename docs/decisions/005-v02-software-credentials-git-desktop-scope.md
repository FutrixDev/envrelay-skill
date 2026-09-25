# ADR-005：冻结 v0.2 交付范围——软件恢复、开发凭据恢复、Git 认证恢复与桌面端

## 状态

Accepted；范围和“先完成普通文件数据面再扩展软件/凭据/Git/桌面”的依赖顺序继续有效。Gate A/B、外部独立审计及签名 release-authority 前置已由 [ADR-009](009-open-source-assurance-and-local-e2e.md) supersede。

SW 包（AC-05）的执行方式与 CRED 包（AC-06）中 SSH/Gradle/Maven 的承载方式已由 [ADR-017](017-skill-orchestrated-software-inventory-and-file-plane-credentials.md)（2026-08-26）改写：软件恢复改为 Skill 清单模式（Core 不扫描、不封装、不安装软件），凭据以既有文件平面为主承载，devenv 凭据通道只保留 GPG。四能力包的范围本身与依赖顺序不受影响。

本 ADR 不允许把软件、凭据、Git 或桌面能力“顺手”实现进当前 local-backup vertical slice；但这些能力后续开工不再等待外部 Gate A/B 或独立审计，而以 ADR-009 的开源可复现内部 assurance 和对应功能案例为门。

## 日期

2026-07-17（产品负责人于当日明确指示推进以下四项能力）

## 背景

原始需求（[mg.md](../../mg.md)）包含普通文件、开发密钥、软件与 Git 的备份恢复。[ADR-003](003-ai-executable-v01-scope.md) 把 v0.1 冻结为"普通文件/目录 + Google Drive + 双 Agent 自动化"，并规定：任何新增范围都必须先更新该 ADR 或创建后继 ADR。本文件就是该后继 ADR。

四项能力的设计资产已经存在但只是"长期参考"：软件分级与原生迁移协作在 [ADR-002](002-software-compatibility-and-native-handoff.md)，Agent 决策边界在 [ADR-001](001-agent-first-autopilot.md)，软件/凭据/Git 适配细则在 [platform-adapters.md](../platform-adapters.md) §4–§10，桌面端在 [desktop-product-spec.md](../desktop-product-spec.md)，需求编号在 [implementation-blueprint.md](../implementation-blueprint.md) §4。缺失的是：一份把它们提升为**已决定交付范围**的 ADR、可执行的实施计划、以及与 v0.1 治理（Gate、boundary、closed contracts）不冲突的开工顺序。

当前事实（详见 [implementation-status-v0.1.md](../implementation-status-v0.1.md)）：v0.1 仍是 Gate A 前 fail-closed 骨架，四项 AC `0/4`，真实加密/IPC/provider/scanner 均未实现。四项 v0.2 能力全部构建在 v0.1 数据面（2-of-2 加密 vault、journal、provider、target writer）之上，因此**任何 v0.2 数据面实现在 v0.1 完成前都无从谈起**；能先行的只有规范、合同形状、registry 数据准备与安全评审。

## 决策

### 1. v0.2 交付四个能力包

| 包 | 新 AC | 范围权威 | 一句话边界 |
|---|---|---|---|
| SW 软件恢复 | AC-05 | ADR-002 + [platform-adapters](../platform-adapters.md) §4–§7 | 按五级兼容分级恢复软件；v0.2 manager 闭集固定为 Homebrew（formula+cask）、winget、apt/dpkg；Flatpak/Snap/Store/MAS 仅 Inventory 或 Tier 2 |
| CRED 开发凭据恢复 | AC-06 | [platform-adapters](../platform-adapters.md) §8 | SSH、GPG、Gradle/Maven 四个 adapter 闭集；Secret 级条目在独立安全审计完成前只能 Beta |
| GITAUTH Git 认证与仓库恢复 | AC-07 | [platform-adapters](../platform-adapters.md) §9 + 本 ADR §3 | 恢复 Git 身份/签名/认证配置的可移植部分，并基于已恢复凭据重新 clone；不迁移 credential helper 存储的 token 本体 |
| DESK 桌面端 | AC-08 | [desktop-product-spec.md](../desktop-product-spec.md) + 本 ADR §4 | Tauri 2 + React 本地可信面（Autopilot policy、Action Center、因素输入、恢复演练）；helper 进程安全模型见 §4 |

四个包的验收细则、任务拆分与红绿顺序由 [v0.2 实施计划](../plans/2026-07-17-v02-software-credentials-git-desktop.md) 冻结；该计划是 v0.2 唯一实施顺序权威。

### 2. 不可绕过的顺序门

1. **v0.1 优先**：v0.1 四项 AC 未全部达成并经 final T 安装重验前，四个包的任何 production 数据面实现（扫描 I/O、凭据文件解析、包管理器执行、目标写入、桌面 IPC）都不得开始。v0.1 期间这四项能力不得出现在成功报告、支持矩阵或任何 release claim 中（沿用 ADR-003；[release-support-v0.1.json](../release-support-v0.1.json) 的 `unsupported` 列表已声明 software/credentials/git/desktop）。
2. **全局密码学顺序不变**：Gate A 之前禁止一切 production-shaped crypto/journal/provider/IPC 实现——对 v0.2 同样成立。v0.2 不新增任何绕过该顺序的例外。
3. **现在允许先行的工作闭集**：本 ADR、v0.2 实施计划、合同形状草案、软件兼容 registry 的数据收集与来源审计、桌面壳 helper 进程安全模型的独立评审准备。这些工作必须保持 zero-authority、零 I/O 语义，不触碰 v0.1 冻结面。
4. **合同目录准入**：v0.2 JSON Schema 进入 [contracts/](../../contracts/) 的前置条件是：(a) 当前 v0.1 在途 contracts 批次已提交且 workspace 测试/fmt/clippy/boundary 全绿；(b) 新 schema 同步注册进 `migration-contracts` runtime registry 并附 valid/invalid fixtures，保持 registry 覆盖测试全绿；(c) 不改变任何 v0.1 已冻结 schema 的语义。在此之前 v0.2 合同形状只能以草案形式存在于 v0.2 计划文档内。
5. **包间依赖**：CRED 先于 GITAUTH（clone 依赖已恢复的 SSH/GPG）；SW 与 CRED 可并行；DESK 的 Action Center 依赖 ADR-001 的 AutonomyPolicy / AgentDecisionContext / DecisionProposal 合同先冻结。
6. **workspace 变更走既有边界流程**：v0.2 新增任何 crate、feature、target 或依赖边都必须让 [check-trusted-core-boundary.sh](../../scripts/check-trusted-core-boundary.sh) 失败并经独立复核更新快照（ADR-004 既有规则）；本 ADR 不预先固化新 crate 名。

### 3. GITAUTH 的精确范围

"Git 认证方式恢复"在 v0.2 精确指以下可移植集合：

- **SSH 认证**：私钥/公钥/`config`/`known_hosts` 经 CRED 的 SSH adapter 恢复（权限位、staging 合并、`IdentityFile`/`Include`/`ProxyCommand` 重映射规则见 [platform-adapters](../platform-adapters.md) §8.2）；
- **GPG 提交签名**：signing key 经 CRED 的 GPG adapter 恢复，`user.signingkey`、`gpg.format`、`commit.gpgsign` 随 Git 配置子集恢复；
- **Git 配置清洗子集**：`user.name`、`user.email`、签名相关键、`core.sshCommand`（路径重映射后）、alias 中不含 shell 注入面的白名单子集；自由 `credential.helper` 值不逐字迁移，而是按目标平台映射到等价后端（osxkeychain → Windows manager → libsecret），映射表随软件兼容 registry 一起签名发布；
- **仓库重建**：按 [platform-adapters](../platform-adapters.md) §9 用清洗后的 remote URL 重新 clone、checkout、submodule、patch、untracked 恢复与 `git status` 核验；远端认证失败必须归类 `reauth_required`，不得把"目录存在"报告为恢复成功。

**明确不做**：credential helper 内存储的 PAT/token 本体、浏览器/IDE 的 OAuth 会话、企业 SSO 状态、硬件 FIDO/智能卡 key material。这些与 [ADR-003](003-ai-executable-v01-scope.md) 的排除项（Keychain 证书链、DPAPI/TPM、设备绑定秘密）一并保持永久排除，只登记存在并提示重新认证。

### 4. DESK 的安全边界决定

[desktop-product-spec.md](../desktop-product-spec.md) 的产品形态（窗口、路由、Action Center、平台行为、可访问性）整体采纳为 v0.2 需求基线，并叠加以下与 [ADR-004](004-v01-trusted-core-co-location.md) 一致的硬性决定：

1. 桌面壳是**独立 helper 进程**，不是第二个核心：加密、journal、provider、scanner、target writer 仍只存在于 bin-only `migration-core-service`；桌面壳通过与 CLI 相同的 `migration-client` safe library 执行 TaskId 状态循环，不得链接 core-service、storage、google-drive 或获得任何 authority 类型。
2. 涉及秘密的本地动作面（恢复因素输入、二维码展示/扫描、native picker、OAuth 浏览器跳转）继续由 Core-private local-action broker 所有。桌面壳承载这些页面的前提是：service 从固定已签名安装位置启动桌面 helper、验证其代码身份，并使用 service-spawned、one-time、current-user-only 的 private broker channel 传递 surface 绑定；**绝不复用 Agent IPC**，channel 不可被 CLI/Agent/第三方进程重放。此机制在实现前必须完成独立安全评审（desktop spec 状态注记的要求由本 ADR 正式承接，评审通过前 DESK 数据面不得开工）。
3. 因素 payload 只在 Rust factor session 生命周期内存在：不进入 DOM 持久化状态、`localStorage`、日志、崩溃报告或 IPC trace；离开 route 即销毁（desktop spec §4.3/§13 的 secret canary 验收对 v0.2 生效）。
4. 桌面壳不引入 Node 后台进程；React 只渲染与调用窄命令（blueprint §6 既有边界）。

### 5. 与 v0.1 合同的关系

- v0.1 manifest/plan Schema 中已有的 credential/software/git/symlink future syntax 仍被 `V01FilesOnly` capability gate 拒绝。v0.2 通过**新版本 manifest 合同与新的 capability gate** 扩展（gate 闭集由 v0.2 计划的合同任务冻结），不放宽任何 v0.1 authority。
- v0.2 的 release 证据沿用 v0.1 的对象图形状（新一代 candidate C、独立 evidence、S/Ts/P/T）：v0.1 的 final T 不自动授权 v0.2 能力，v0.2 发布必须生成**新 candidate 并全量重采证**。
- canonical Skill 在 v0.1 期间不改；软件/凭据/Git 决策相关的 Agent 命令类扩展属于 v0.2 计划任务，且必须继续满足 ADR-001 的 Decision Context/Proposal 边界（Agent 只见枚举决策，不见路径/指纹/URL/秘密）。

## 被拒绝的方案

### 在 v0.1 骨架上立即开始实现四项能力

违反 Gate A/Gate B 顺序；v0.1 数据面不存在，凭据/软件/Git 恢复无载体；会破坏当前一切入口 fail-closed 的安全承诺。拒绝。

### 把桌面壳做成第二个秘密处理核心

在 Tauri 进程内复制加密/journal/provider 逻辑会制造两个秘密边界与行为漂移，[ADR-004](004-v01-trusted-core-co-location.md) 已拒绝，本 ADR 重申。拒绝。

### 用"备份整个 dotfiles / ~/.gnupg / ~/.ssh 目录"替代逐 adapter 评审

看似省事，实则把 socket、agent cache、硬件 stub、非可移植状态与未知秘密一并搬运，绕过敏感级别、清洗与验证框架（[platform-adapters](../platform-adapters.md) §8.5 明确禁止）。拒绝。

### 按显示名模糊匹配自动安装软件

包名劫持与供应链风险，ADR-002 已拒绝。拒绝。

### 迁移 credential helper / 浏览器中存储的 token 本体

等同于把第三方服务凭据带出其原生保护域，与威胁模型和"设备绑定秘密不迁移"的产品边界冲突。拒绝；一律 `reauth_required`。

## 后果

- 新增 [v0.2 实施计划](../plans/2026-07-17-v02-software-credentials-git-desktop.md)，作为 v0.1 计划完成后的唯一实施顺序权威；README 阅读顺序同步加入本 ADR 与该计划。
- AC 编号空间扩展为 AC-01..AC-08；v0.1 验收口径不变。
- [release-support](../release-support-v0.1.json) 与 contracts 的 v0.2 行/schema 按 §2.4 的准入条件延迟同步；在此之前四项能力在一切机器可读支持视图中保持 `unsupported`。
- 软件兼容 registry 与 credential-helper 映射表成为持续维护的签名数据资产，需要新的外部输入（数据来源审计、签名 key、registry 版本/回滚策略），登记入口在 v0.2 计划的外部输入节。
- 桌面 helper 进程机制新增一次独立安全评审义务；评审结论将决定 DESK 数据面能否开工。
