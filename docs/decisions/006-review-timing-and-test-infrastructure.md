# ADR-006：评审时点收敛为售前硬门，并冻结开发期测试基础设施

## 状态

Partially superseded by [ADR-009](009-open-source-assurance-and-local-e2e.md)：开发期测试基础设施部分继续有效；售前独立门、Gate A `R/Q/S` 以及其对真实数据/Beta/销售的硬阻塞要求已取消。

## 日期

2026-07-17

## 背景

v0.1 治理原设计为"开发期两级独立门"：readiness 独立复审清零 → Gate A（仓库外 R/Q/S verifier）→ 实现 → Gate B 独立 evidence 复核 → 发布。对单人项目，独立评审人在开发期不可得，导致 v0.1 在 readiness 复审处停滞。产品负责人决定调整评审时点；同时冻结开发期测试基础设施的实际形态，避免为不可用的理想环境空转。

## 决策

### 1. 独立评审与安全审计收敛为一个"售前独立门"

以下各项从开发期前置调整为**公开 Beta/销售前的一次性 pre-release review bundle**，在此之前由产品负责人 self-review 承担同等检查职责：

- v0.1 readiness 独立复审（原 Gate A 前置）；
- Gate A 的仓库外 R/Q/S authority verifier 闭包；
- Gate B 的独立 evidence 复核（B/E 签署）；
- 桌面 helper 安全模型独立评审（V2-0.4，[草案](../drafts/v0.2/desktop-helper-security-model.md)已备好评审请求）。

**保留不变的**：

- 顺序不变：先向量、后实现；先 synthetic、后真实数据。self-review 必须按原 checklist 逐项执行并留下记录（review 记录写明 `self_reviewed=true`，不得伪装为独立复核）；
- 机器独立性不变：candidate vectors 仍必须由独立第二语言生成器（Go 层已存在）产生，Rust 实现不得自证；
- **真实用户数据、公开 Beta、销售，三者继续被独立门硬性阻塞**：pre-release bundle 未由未参与实现的人完成前，产品只能处于 engineering preview，不得处理任何非 synthetic 数据或对外宣称安全性已批准；
- 各状态文件（security-review、implementation-status、release-support）不因本 ADR 改写既有 `NOT_READY`/`CLOSED`/`unsupported` 值；状态推进仍按"事实发生后同步"的既有规则执行，仅"推进所需的复核人"在开发期按本节替换为 self-review。

**如实记录的代价**：外部视角后置意味着格式/边界类错误可能在实现大量展开后才被发现，返工成本显著升高；self-review 无法消除作者盲区。负责人接受该风险以换取开发期进度；售前独立门是该风险的最终兜底。

### 2. 开发期测试基础设施冻结

| 平台 | 开发期形态 | 能产生什么 | 明确不能产生什么 |
|---|---|---|---|
| macOS | 负责人本机（Darwin 25.x / Apple Silicon，落在 Tier 1 家族内） | 全部开发/CI；后续 tuple 采集与路线测试的候选 runner | — |
| Ubuntu | k3s 容器（ubuntu:24.04 基础镜像 + 仓库固定工具链） | 全部 cargo/合同/oracle/fixture/Go 向量 CI | runtime tuple、desktop session、Secret Service preflight、3×3 路线等一切验收证据（容器无 logind/GNOME Wayland/持久凭据库，probe 合同 fail closed 属设计行为） |
| Windows | 推迟 | — | — |

进入 Gate B 数据面证据阶段前，须补一台 Ubuntu Desktop 虚拟机（GNOME Wayland 会话）；Windows 虚拟机同期补齐。虚拟机满足"真实 VM/设备"要求，容器不满足。

### 3. 推迟项（不是取消，登记最晚时点）

- **Windows Agent 内核隔离投入**（Hardware Dev Center、驱动/catalog 签名、minifilter altitude）：推迟，`agent_host_matrix` 的 Windows 行继续保持 `unsupported`。负责人后续在三个方向中选择其一并另立 ADR：(a) v0.1 Agent 支持收窄为 macOS/Ubuntu（需同步修订 AC-04 三平台表述与 candidate 六 bundle 结构）；(b) Windows 采用弱化的纯用户态监督并如实降级该行承诺；(c) 维持内核设计并按期申请。该决定最晚在 Task 13D 开工前做出；
- **Codex/Claude 管理侧输入**：推迟。到期动作已收敛为：两个 CLI 的精确版本与二进制哈希 pin、两个 QA 专用模型 API key、官方文档原件抓取 receipt、Codex workspace 管理权；无企业 MDM 要求。最晚 Task 13F/15 前；
- **双根签名基础设施**：推迟到首个 QA authorization/发布对象签发前（与原最晚硬门一致）。单人规模的最小实现口径记录为：两把用途隔离的独立密钥（硬件 key 或云 KMS）+ 仓库外人工确认签名流程 + 发布源上的撤销清单；
- **Google OAuth**：开发期使用负责人个人 Google Cloud 项目（testing consent + 负责人账号为测试用户，注意 testing 模式 refresh token 7 天过期）；发布前将 consent screen 发布为 production。用户数据路径不经任何中间组织：token 由 Google 直发用户本机，密文由用户本机直连用户自己的 Drive（`drive.file` 最小 scope），该架构事实不因商业授权服务的存在而改变。

## 后果

- v0.1 从 readiness 复审处解除停滞：负责人 self-review 完成并留档后，可按原顺序推进 candidate vectors 与后续实现；
- 所有"已批准/已验证"类对外表述在售前独立门完成前继续禁止；
- [external-release-inputs-v0.1.md](../external-release-inputs-v0.1.md) 中受影响行的"最晚硬门"语义不变，本 ADR 只调整"谁在开发期承担检查"与"何时必须转为独立"；
- 后续若团队扩充获得可用的独立评审人，可随时提前恢复原两级独立门，无需新 ADR。
