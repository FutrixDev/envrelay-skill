# ADR-018：多 source 备份合入同一 vault 与同一 generation

## 状态

Accepted。

## 日期

2026-08-28

## 背景

在 ADR-013/016 的模型下，一次 local backup 只接受一个 backup source：用户如果想把「几个文件夹 + 工具配置 + 密钥」一起备份，只能逐个建任务、逐个生成 vault，恢复时也要逐个 install。这与产品目标（用户选好目录后全程不用管，恢复机上按 manifest 一次装回）直接冲突。

同时，既有的安全边界必须原样保留：封闭 CLI 文法、zero-authority 客户端、fixed IPC 的 `deny_unknown_fields`、per-root 的 scan→policy→plan→execution 证明链、grant broker 的 slot 模型，以及 sealed metadata 不泄露真实路径的约束。

## 决策

### 1. N 个 source → 一个 vault → 一个 generation

- 一次 local backup 接受 1..=256 个互不重复、互不嵌套的 source 目录（`MAX_BACKUP_SOURCES = 256`），全部写入同一个 local vault，冻结为**一个** encrypted generation。
- 数量上限之外还有一个总字节守卫：所有 source 路径的 UTF-8 字节总和不得超过 24 KiB（`MAX_BACKUP_SOURCES_TOTAL_PATH_BYTES = 24 * 1024`），超出即 fail closed。单靠数量上限无法保证一次合法提交能作为命令行被 spawn——Windows 的命令行上限是 32,767 个 UTF-16 code unit，而 256 个各 4,096 字节的路径远超此数；24 KiB 路径字节加上 256 对 `--source` token 与固定 argv 在任何受支持平台上都留有余量（UTF-8 字节数恒不小于同文本的 UTF-16 code unit 数，故该总和是命令行开销的上界）。CLI 文法解析器、client 构造器与两端 fixed IPC 解码器各自独立执行同一守卫。
- generation 的 manifest 携带 N 个 root：每个 source 对应一个 `root_id`（由 metadata key 派生，pairwise distinct，全零拒绝），manifest entry 以 `root_id` 限定 item ref 与路径。
- 加密对象池跨 root 去重：相同内容的文件只封存一个 object，occurrence 预算与总 plaintext 预算按全局累加校验（`LOCAL_BACKUP_MAX_ENTRIES` / `LOCAL_BACKUP_MAX_LOGICAL_BYTES` 等对 N 个 root 的**总和**生效）。
- `plan.rs` / `execution.rs` 保持单快照职责不变；multi-root 逻辑集中在 `production_generation.rs`：逐 root 走完整的 policy→plan→execution 证明，再对每 root 的 final-source closure 做逐对匹配 + 总和匹配的双重绑定（`validate_final_source_binding`）。
- final-source closure digest 域升级为 `envrelay/final-source-closure/v0.3`：绑定首个 proof 的 task/revision、root 数量（u64）以及每个 root 的 digest/totals 块；所有 proof 必须共享同一 task/revision。`GENERATION_DIGEST_DOMAIN` 保持 v0.2 不变。

### 2. 封闭文法 v7 与 wire 兼容

- CLI 文法升级到 `migration-vault-cli-argv-grammar/v7`（0.7.0）：仅 `action_open_source` 与 `action_open_source_vault` 两个 variant 获得 `repeat` 声明——`--source {source_path}` 这一 token 对可重复 1..=256 次，其余 21 个 variant 与 v6 逐字段相等，由语义校验器机械推导并强制。
- IPC wire 上单 source 保持历史单数 `source` 字段逐字节不变；两个及以上使用 `sources` 数组（2..=256，去重，且受同一 24 KiB 总字节守卫约束）。`source` 与 `sources` 互斥；旧 Core 因 `deny_unknown_fields` 直接拒绝 `sources`，fail closed，不会静默只备份其中一个目录。
- backup plan 的 `scope.root_refs` 按 contract 要求严格排序后写入；staging 顺序（用户选择顺序）不进入 sealed 文档。

### 3. Grant broker：BackupSource 变为多槽

- `LocalRootPurpose::BackupSource` 的 slot 容量为 `MAX_BACKUP_SOURCES`（256）；`RestoreTarget` 与 `LocalCiphertextVault` 保持单槽。一次满容量 backup 同时持有 256 个 source lease 加一个 vault lease，因此 `MAX_ACTIVE_GRANTS` 提升到 320、`MAX_SESSION_GRANT_IDS` 提升到 65,536；Core 入站 raw JSON 守卫的节点上限相应改为 `32 + MAX_BACKUP_SOURCES`（最大合法请求是携带满 `sources` 数组的 `action_open`）。占用计数按 (task, action, purpose) 过滤，槽满后下一次 approval 返回 `MV_POLICY_LOCAL_GATE_REQUIRED`，不做隐式替换。
- source 两两不重叠（相互包含即拒绝），且逐一以真实 I/O 验证后 claim；root identity 冻结时再做一次 pairwise distinct 复核，防御 approval 与 freeze 之间的漂移。

### 4. GPG 覆盖：恰好命中一个 source

- 当 coverage 带 approved GPG root 时，该 root 必须与 N 个 source 中**恰好一个**同目录（`is_same_directory`）：命中 0 个或 ≥2 个都返回 `MV_POLICY_SCOPE_EXCEEDED`。`FilesOnly` coverage 完全跳过该检查。

### 5. Sealed metadata 中的 source 路径保持不透明

- manifest 的 `scope_roots` 只写 `root_id`、`logical_root: "custom:<id>"` 与 `source_path: "envrelay:approved-root:<id>"`。真实路径写入 sealed metadata 会活得比让它「可说出」的那次 approval 更久，违反 ADR-008/013 的授权生命周期，因此拒绝。

### 6. 恢复：multi-root 落回一个 target，路径冲突 fail closed

- 恢复把 N 个 root 的 entry 全部装入同一个 restore target。当各 root 的相对路径集合互不相交时可直接工作；一旦两个 root 含同一相对路径，以 `MV_RESTORE_NAME_DUPLICATE_SOURCE_PATH` fail closed。该错误码原样穿出 restore planner（不折叠进泛用 plan error），恢复端能看到确切原因。
- 冻结端提前拒绝：freeze 在 staging 完全部 root 的 inventory 后，对跨 root 的相对路径做全局去重检查，发现冲突以 `MV_PLAN_PATH_CONFLICT` fail closed（`two_sources_with_a_colliding_top_level_name_fail_the_freeze_closed`）。一个注定无法恢复的 generation 不该被造出来——在原件还在手边的机器上失败，好过在恢复机上失败。单 root 内的唯一性由 scanner 保证，检查只在多 root 时运行。
- **暂不**做 per-root 前缀目录：manifest proof 依赖 `try_for_each_event_in_order` 的全序 + 精确集合证明，恢复端合成不在 manifest 内的 per-root Directory item 会绕开该证明，是不健全的。前缀方案要在 manifest 生成端把前缀目录作为真实 entry 写入后才能上线，作为已设计的后续工作单独提交。

## 验收案例

- 两个 source（含相同内容文件与嵌套目录）冻结为一个 generation：manifest 有 2 个 distinct root、对象池跨 root 去重、totals 为逐 root 之和（`two_sources_freeze_into_one_generation_with_two_manifest_roots`）。
- BackupSource 槽位填满 `MAX_BACKUP_SOURCES` 个后再一次 approval 返回 local gate required，且已 claim 的 lease 全部保持 active（`backup_source_slots_fill_to_the_cap_and_the_next_approval_fails_closed`）。
- CLI 接受 1..=256 次 `--source`（可带 `--vault`），拒绝超出数量上限的一个、超出 24 KiB 总字节守卫的组合、重复路径、`--vault` 在 `--source` 之前等一切偏离封闭文法的形状。
- wire fixture：单 source 请求逐字节不变；满容量 256 source 请求穿过 raw 节点守卫与解码器成功解出；`sources` 数组长度 1、超出上限、超出字节预算、重复、与 `source` 并存、与 `target` 混用均拒绝。
- 文法 v7 契约测试从冻结的 v6 文档机械推导期望：除两个 `--source` variant 的 `repeat` 声明外任何字段漂移都失败。

## 影响

- `migration-client` 公共 API 新增 `backup_sources` / `backup_sources_and_vault`（`LocalActionPaths` 内部改为 `sources: Vec<PathBuf>`），public-api 快照需再生。
- 旧 Core 无法解出多 source 请求（fail closed）；旧 generation（单 root）继续可读，因为单数 wire 拼写与 `GENERATION_DIGEST_DOMAIN` 均未变。
- final-source closure digest 域 v0.2→v0.3：多 root 绑定不与旧域混淆。
