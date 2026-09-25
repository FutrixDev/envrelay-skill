# ADR-004：v0.1 采用独立协议库、固定 IPC 客户端与 bin-only Trusted Core

## 状态

Accepted；Trusted Core 共置与三层可见性边界继续有效。第 5 节的 Gate A/B、外部 `R/Q/S` 和 installed final T 前置已由 [ADR-009](009-open-source-assurance-and-local-e2e.md) supersede。

## 日期

2026-07-14

2026-07-16补充：冻结无参数human recovery facade；不改变bin-only Core边界。

2026-07-19补充：[ADR-007](007-v01-scope-convergence-skill-cli-core.md) 已把产品交付物收敛为 Skill + CLI + Core，[ADR-008](008-session-scoped-user-approved-root-grants.md) 已把 source/target authority 收敛为进程内 session root grant。此前规划的产品内 system-attestor release-import client、stable cross-process root identity 与相应 wire namespace 不再属于 v0.1 产品合同；QA-lab 可保留私有数据面工具，但不得进入本 ADR 的产品依赖 DAG 或公开 client surface。

## 背景

v0.1 必须同时满足三件事：

- 只有完整认证 MVJ1/MVJC、trust-state、manifest graph 和本地文件系统事实后，Core 才能推进任务或写入恢复目标；
- Codex、Claude Code、CLI 和普通 UI 只能看见 Task ID、阶段、计数、稳定错误码等脱敏数据；
- macOS、Windows 与 Ubuntu 上的新 CLI 进程都能通过 Task ID 重新连接同一个每用户任务，而不是在每次调用中复制 Core 权限。

旧方案把公开 `MigrationClient`、私有 `MigrationCore` 和未来的 service 入口都放在 `migration-domain` library 中。这在 Rust 中不能按安全目标接线：binary target 是另一个 crate，不能访问 library 的 private 或 `pub(crate)` Core；若改成公开 `run_service`，任意下游又可以在自己的进程中启动 secret-bearing Core。Rust 没有 friend crate，`#[doc(hidden)]`、sealed trait、feature allowlist 和命名约定都不能修复这个可见性问题。

同样，private provider wrapper 若不依赖 Google Drive crate，就无法实际调用它；而让 CLI 或任何 Agent orchestration 直接依赖 storage、scanner 或 Google Drive，又会结构性地扩大 Agent 可触及的路径、provider 和 byte surface。

## 决策

### 1. 三层边界

v0.1 固定为三层：

```text
migration-domain          无 authority 的 closed protocol、ID、错误和脱敏 DTO
migration-client          固定 endpoint 的短生命周期 IPC client 与 opaque watch
migration-core-service    bin-only、签名、每用户单实例的 Trusted Core
```

`migration-domain` 不再拥有 secret、provider、journal parser、state reducer、target writer 或 service 入口。它只定义：

- canonical ID、closed enum、稳定错误策略；
- Agent 允许发送的 closed request DTO；
- Core 允许返回的 system probe、task selector/status/event/report 等脱敏 response DTO；
- 每个 DTO 的有界、跨字段验证。

这些 response DTO 本身不授予任何权限，也不会被 Core 的 request union 接受，因此可以有 public validating constructor 和安全 Serde 实现，供 service 构造、client 解码后再次验证。能构造一份 `TaskReportView` 不等于能改变任务状态。真正必须保持不可构造、不可 Clone、不可 Serde 的是 `MigrationClient`、`TaskWatch` 和 service 内部 authority。

`migration-client` 是 CLI、UI 和 Skill 唯一可依赖的控制库。Core控制类型`MigrationClient`提供无参数`connect_system()`，由库内部选择固定每用户endpoint；除TaskId驱动的safe操作外，它提供唯一无参数high-level `run_or_resume_local_recovery()`，在client-private模块内完成durable outbox与多步bounded IPC，只返回`TaskReportView`。raw recovery operation、continuation TaskId与surface token不公开。client 在返回 DTO 前执行 frame、contract、direction、peer/code-identity 和语义验证，且不可 Clone/Debug/Serde。v0.1 不提供连接另一产品 attestor 的公开 client。

`migration-core-service` 只有 binary target，不提供可被其他 crate 链接的 library，也不公开 `run_service`。`MigrationCore`、scheduler、state reducer、provider binding、credential/root handle、scanner、crypto、journal、trust-state、manifest graph 和 target writer都属于该 binary crate root 的 private module。服务启动时先验证当前服务二进制身份、安装来源、当前用户 peer、每用户单实例锁与固定 endpoint；任一证明失败都不能打开凭据库或任务仓库。

### 2. 精确依赖 DAG

workspace 依赖方向冻结为：

```text
migration-contracts       -> no workspace dependency
migration-domain          -> no workspace dependency
migration-client          -> migration-domain
migration-storage         -> no domain/client/core dependency
migration-google-drive    -> migration-storage
migration-core-service    -> migration-domain
                           + migration-contracts
                           + migration-google-drive
                           + migration-storage
migration-cli             -> migration-client only
```

`migration-domain` 只保留 Core request/response 的 closed、zero-authority namespace；v0.1 产品合同不再包含 attestor release-import wire、caller-selected handle API 或 `migration-agent-attestor` 产品依赖。CLI 仍只能依赖 `migration-client`。

v0.1 不存在独立 `migration-engine` 或 `migration-local-ui` package。CLI 直接编排短生命周期 `MigrationClient` 调用，真实 scheduler 与可信本地交互 adapter 都是 `migration-core-service` binary 的 private module。若未来平台限制要求 helper process，必须先新增 ADR，并使用 service 启动、签名身份校验、一次性 current-user-only private broker；它不得复用 Agent IPC。

`migration-google-drive` 是受审计的低层 OAuth/HTTP 数据面依赖，不签发 task、journal、publication、listing completion 或 restore authority。token handle、Drive file ID、session、locator、sealed bytes 和 raw response 只在 service binary 的 private wrapper 与该低层依赖之间短暂流动；CLI、UI 与 Skill 不能增加对它或 storage 的 direct dependency。

现有 `migration-scan` 只保留为 fixture/兼容性测试实现，不能产生 production scan authority。production scanner、用户批准记录与 session-scoped live root handle 必须在 service binary 内实现；Core 重启后 grant 与旧 scan/plan 失效，不建立 stable cross-process root identity。QA 可以依赖 fixture scanner，CLI 与 client 不可以。

任何新增 workspace member、feature、target、direct dependency 或 reverse edge 都必须让边界检查失败，并经过新的 trusted-core 复审。

### 3. 跨进程协议是定向的

IPC request 与 response 使用不同的 closed union：

- Agent/Core request只允许 probe、active、create、status、watch、action-open、run、retry、cancel、report；human recovery raw union仅由client-private高层facade使用；v0.1 产品协议没有 attestor release-import union；
- response 只允许对应命令的唯一 payload kind，不能用 generic `{}` acknowledgement 兜底；
- Task status/report/event 等 response DTO 永远不能反向作为 request；
- request ID 统一为 26 字符 canonical id128。读请求只用它做关联；写请求同时把它作为持久幂等键。

所有 mutation（包括 `action-open`）必须先把 exact request 写入当前用户独占 outbox并 fsync，再发送。service 的 durable dedupe 记录绑定 `request_id + operation + task_id/null + expected_revision/null + provider/null + request digest + exact result`。同 ID 同 tuple 返回原结果；同 ID 不同 tuple 返回 non-retryable `MV_STATE_REQUEST_ID_CONFLICT`，不得执行、覆盖或复用旧结果。create 响应丢失时只能重发原 ID，不能创建第二个 task。

`action-open` 使用 typed request，绑定 task ID、action ID、expected action revision 和 request ID。重复请求只能 focus 已有的同一可信 surface，不能重复启动另一个窗口或 TTY。surface 仍由 Core 根据已验证宿主能力选择，Agent 不能指定。

### 4. Watch 不是 authority

`open_watch(task_id)` 不接受 cursor；`next_watch(&mut watch)` 使用 Core 固定 deadline，只返回 `Event`、`Heartbeat` 或 `Closed`。不存在 caller timeout 或 `Pending` 忙轮询。

事件是 advisory、at-least-once 的脱敏通知。多 watcher 各自拥有 service 内游标，不能竞争一个全局消费队列；断线、heartbeat、重复 event 或 `Closed` 后，客户端都以 `task_status(task_id)` 为唯一权威。raw journal cursor、record、checkpoint 和 replay stream永不跨 IPC。

### 5. Trusted Core 内部共置

service binary 内继续共置以下 private authority：

- key/KDF/AEAD 与 secret zeroization；
- strict byte parser、MVJ1/MVJC、trust-state、manifest/commit graph；
- exclusive journal replay claim 与 authoritative reducer；
- Core-owned local/Google provider binding、bounded put/listing scheduler；
- Core-owned source/target directory handle、production scanner 与 target writer。

`VerifiedJournalChain`、authenticated replay、`ApprovedBackupPlan`、`ApprovedRestorePlan`、listing receipt、target guard/completion 和 canonical append projection都不能进入 `migration-domain` 或 `migration-client`。在线事件与恢复 replay 必须在 service 内走同一个 reducer；schema success、hash、JSON 或 provider 2xx 都不能替代 authority。

### 6. Gate 顺序

Gate A 之前只允许 zero-secret、synthetic、始终 fail-closed 的 skeleton，用来证明：

- service 确实是 bin-only crate root，private Core 无 public 启动入口；
- client/protocol 不依赖 provider、storage、scanner、crypto 或 journal；
- CLI/UI 只能依赖 client；
- public API 与 compile-fail 测试没有 authority backdoor；
- 全部 Schema、semantic hooks、JSON/raw-byte fixtures 和文档一致。

只有独立 reviewer 将 readiness findings 清零并写入 `READY_FOR_CANDIDATE_VECTOR_GENERATION` 后，才允许独立生成 candidate vectors；这仍不等于 Gate A。后续 vector review `R` 必须保持 `authorization_authority=false`。只有仓库外受保护控制面生成 exact request `Q` 与 detached response `S`，再由 authority-producing verifier 重载并验证 `R/Q/S`、外部 keyring、exact key record、revocation、rollback、可信时间与 single-use replay-consumption receipt 的完整无环闭包，才可能允许 synthetic-only 的 production-shaped crypto/parser 实现。当前该 verifier 不存在，production candidate 固定 fail closed；人工签字、Markdown marker、Schema-valid `Q/S` 或非零 hash 都不能替代。真实用户数据仍必须等待 Gate B 与 installed final T 的完整验证。

## 被拒绝的方案

### 在 library 中公开 `run_service`

外部 binary 虽能调用，但任意下游也能把 Core 拉进自身进程，破坏二进制身份与进程边界。拒绝。

### library 与 binary 依靠 `pub(crate)` 或 friend 访问

Rust 的 package target 仍是不同 crate；不存在 friend visibility。拒绝。

### Agent orchestration 直接依赖 storage、scanner 或 Google Drive

这会让 Agent orchestration 层结构性地获得未来路径、provider 和 byte API，regex lint不能构成权限边界。拒绝。

### 把 output DTO 做成不可跨 crate 构造

service 将无法合法构造 client 要返回的数据；为此再公开 Core getter反而更危险。脱敏 output DTO 本就无 authority，应依靠定向 request union而不是 constructor secrecy。拒绝。

## 后果

- workspace 多一个 client library 和一个 bin-only service package，但 Rust visibility 与实际部署边界一致；
- safe protocol 可以独立 fuzz/contract-test，service 与 client都必须验证，不能由 CLI 猜字段；
- Google Drive 依赖只存在于 service 数据面，CLI/UI 无 reverse edge；
- 桌面 UI 与 CLI 使用同一 client，不再各自实现 Core；
- 未来若要拆 secret/trust/journal crate，必须整体迁移 Trusted Core crate root并新建 ADR，不能公开 capability 作为跨 crate glue。
