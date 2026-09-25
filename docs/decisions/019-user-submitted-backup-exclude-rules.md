# ADR-019：用户提交的备份排除规则（`--exclude`）

## 状态

Accepted。

## 日期

2026-08-28

## 背景

代码仓库备份撞上了 ADR-018 模型的一个硬缺口：一个 `--source` root 必被全量扫描。`node_modules/`、`target/` 这类可再生构建目录动辄数 GiB、文件数以万计，把它们封存进 generation 既浪费加密与存储，又几乎必然触发预算 fail closed；同时 `.git/` 必须留在备份里（ADR-017 Option B 的前提）。此前 Skill 的答案是"用户自己先 clean"——不可靠，而且构建目录在备份期间往往还在变动，一旦被扫描就会挂进 closure 校验，任何 churn 都会让后续阶段 fail closed。

既有安全边界必须原样保留：封闭 CLI 文法、zero-authority 客户端、fixed IPC 的 `deny_unknown_fields`、per-root 的 scan→policy→plan→execution 证明链，以及"路径只是 Agent intent，授权在 Core"的原则。排除规则同样只能是用户确认后提交的 intent，不能变成 Agent 私自替用户删目录或伪造裁剪副本的许可。

## 决策

### 1. 封闭文法 v8：一个新 variant

- CLI 文法升级到 `migration-vault-cli-argv-grammar/v8`（0.8.0）：仅新增 `action_open_source_vault_exclude` 一个 variant——在 Local backup 的 source+vault 形状上，`--exclude {exclude_path}` 这一 token 对可重复 1..=256 次，全部在最后一个 `--source` 之后、`--vault` 之前。其余 23 个 variant 逐字段与 v7 相等，v1-v7 保持冻结历史。
- 排除只存在于 (Local, BackupSetup) 这一个 provider/action 形状；Google Direct/Portable 与一切 restore 形状携带 excludes 一律拒绝。CLI 文法解析器、client 构造器（`backup_sources_excludes_and_vault`）、Core 的 fixed IPC 解码器与 `action_paths_match` 矩阵各自独立执行同一守卫。
- 数量与字节守卫与 source 同构但相互独立：`MAX_BACKUP_EXCLUDES = 256`、`MAX_BACKUP_EXCLUDES_TOTAL_PATH_BYTES = 24 * 1024`，去重，逐条走同一 canonical agent path 校验。
- wire 上新增 `excludes: Option<Vec<String>>`；旧 Core 因 `deny_unknown_fields` 直接拒绝携带 excludes 的请求，fail closed，不会静默做一次未排除的全量备份。

### 2. 每条规则严格绑定恰好一个 source

- 每个排除路径必须是恰好一个已提交 source 的严格词法后代：等于 source 本身、落在所有 source 之外、或（防御性地）能同时落进两个嵌套 source 而产生歧义，都 fail closed。
- local gate 在 approval 时用 `resolve_source_excludes` 把绝对路径从零重新解析成 per-source canonical 相对路径集合（`canonical_relative_path`：`%`→`%25`、`\`→`%5C`、Windows 盘符 `:`→`%3A`，拒绝空/`.`/`..`/绝对路径）——即使解码器有缺陷，规则也不可能被走私到它点名的 source 之外。
- `BackupSetupAuthority::Local`、`ReadyLocalBackupSession` 与 `ReadyBackupGenerationAuthority` 携带与 `sources` 索引对齐的 `source_excludes: Vec<BTreeSet<String>>`；Google/Portable 通道恒为空集合，扫描阶段永远看不到绝对排除路径。

### 3. 扫描器：枚举但不下钻、不哈希、不封存

- `ScanEntryKind` 新增 `ExcludedRegularFile`（code 3）与 `ExcludedDirectory`（code 4）。被排除条目仍被枚举、仍计入 `expected_names` 与各项预算，但记录为 `logical_bytes=0`、`content_blake3=None`，目录不下钻、文件不打开也不可 pump。
- excluded 计数进入 `WalkState`/`VerifiedScanTotals`，并被 snapshot digest 与 final-source closure digest 一并哈希；被排除的名字换成 symlink 仍然 fail closed。
- 被排除目录不保留 `DirectoryClosure`，其下内容在备份期间自由变动不会触发任何后续阶段的 closure 校验失败——这正是排除存在的意义：构建目录可以边备份边重编译。

### 4. 计划与 manifest 证据

- 每条被排除条目在 plan 里是一个真实 item：`decision="exclude"`、`reason_code="EXCLUDED_BY_USER_RULE"`、`decision_source="local_user"`、`evidence_grade="verified"`、`estimated_bytes=0`；exclusions 聚合按 `(code, category="files", count)` 与 excluded items 精确相等（ADR 前身格式规则原样适用）。
- manifest 的 `total_entries` 只计未排除条目；`scope.exclusion_rule_ids = ["EXCLUDED_BY_USER_RULE"]` 当且仅当本次 generation 真有排除，v0.1 与 dev-environment 两条通道一致。
- 一个 source 被排除到一个条目不剩，冻结以 `ScanLimitExceeded` fail closed——一个空 root 的 generation 不该被造出来。
- QA 的 `real_mac_stage` 桥不提交排除规则，因此它的扫描里出现 excluded record 视为 scanner 故障，在 inventory 构造处 fail closed，不向下传递。

### 5. Skill：排除是提交的规则，不是文件系统操作

- Skill 报告可见的可再生目录及其体量，由用户确认排除清单后作为 `--exclude` 提交；`.git/` 永远不进排除清单。
- 依旧禁止：替用户删除/移动构建目录、把裁剪副本拷进 staging 树伪造排除、未经确认默默应用排除。

## 验收案例

- 扫描器：被排除目录只枚举不下钻（内部 symlink 证明无 descent），其下内容 churn 后 `verify_source_closure` 仍通过；被排除文件不可 pump 且相邻包含文件不受影响；排除名下的 symlink fail closed；`canonical_relative_path` 编码与拒绝集与扫描编码一致（scanner::tests 四例）。
- E2E：两 source 各带排除/不带排除冻结为一个 generation，inventory/对象池只含未排除条目，plan 含 `EXCLUDED_BY_USER_RULE` item 与 exclusions 聚合，handoff 计数一致（`an_excluded_directory_leaves_plan_evidence_but_no_sealed_bytes`）；全部被排除的 source 冻结 fail closed（`a_source_with_everything_excluded_fails_the_freeze_closed`）。
- local gate：excludes 只被 (Local, BackupSetup) 接受，超上限拒绝；`resolve_source_excludes` 正确按 source 分组并 canonical 编码，等于 source、落在所有 source 外、重复、嵌套歧义、非 Local provider 全部拒绝。
- CLI/client：接受 1..=256 次 `--exclude`（在最后一个 `--source` 之后、`--vault` 之前），拒绝超数量上限、超 24 KiB 字节预算、重复路径、错位、以及任何其他形状上的 `--exclude`。
- 文法 v8 契约测试从冻结的 v7 文档机械推导期望：除新增 variant 外任何字段漂移都失败。

## 影响

- `migration-client` 公共 API 新增 `backup_sources_excludes_and_vault`、`excludes()` 与两个上限常量，public-api 快照需再生。
- 旧 Core 无法解出携带 excludes 的请求（fail closed）；不带排除的提交与既有 generation 逐字节不变，`GENERATION_DIGEST_DOMAIN` 不变。
- canonical Skill 树（SKILL.md 与 references）、eval cases、running-a-backup 指南随 v8 同步；skill-tree manifest v0.2 与输入 SHA-256 清单按依赖顺序再生。
