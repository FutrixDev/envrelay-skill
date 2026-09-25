# ADR-014：产品更名为 EnvRelay，序列化身份保持 `migration-vault` 冻结命名空间

## 状态

Accepted

## 日期

2026-07-24

## 背景

产品对外名称从 "Migration Vault" 改为 **EnvRelay**，可执行文件名从 `migration-vault` 改为 `envrelay`。

本仓库把大量标识符钉死在摘要里：contract 标识符是 closed schema 的 `const`，wire discriminator 出现在 JCS 摘要覆盖的 fixture 中，HKDF/MAC domain separator 和容器 magic 直接参与密钥派生与完整性校验。因此“改名”不是一次字符串替换：同一个 `migration-vault` 字面量，在不同位置分别属于**显示名**、**产品表面**和**序列化身份**三类，改动代价相差数量级。

在没有明确判据时，全局替换会静默改变已冻结的密文格式与合同摘要，让旧 vault 无法解密、让已签发的 fixture 失效；而完全不替换，则产品名与可执行名长期不一致。本 ADR 固定这条分界线。

## 决策

### 1. 三层分界与判据

判据：**某标识符若同时（a）是 closed schema 的 `const`/`enum`/必需对象键，且（b）出现在受摘要固定的 fixture 中，则属于 Tier C，永久冻结。**

| 层 | 含义 | 处理 |
|---|---|---|
| Tier A 显示名 | 面向用户与文档的产品名 | `Migration Vault` → `EnvRelay` |
| Tier B 产品表面 | 可执行名、运行时服务标签、开发期环境变量 | `migration-vault` → `envrelay`，`dev.migration-vault.*` → `dev.envrelay.*` |
| Tier C 序列化身份 | contract 标识符、wire discriminator、crypto domain separator、容器 magic、安装清单角色与路径、release 标识符与 origin | 永久冻结，保留 `migration-vault` 字面量 |

### 2. Tier B 实际改动

- cargo `[[bin]] name` 与 `CARGO_BIN_EXE_*` 派生的全部测试入口改为 `envrelay`；boundary 脚本与其固定的 cargo metadata 结构同步。
- 运行时 fixed-IPC socket 命名空间 `dev.migration-vault.v02.{uid}` → `dev.envrelay.v02.{uid}`。该命名空间不出现在任何 schema 中，只由 client 与 core-service 双方在运行时构造，因此可以自由改名。
- macOS 打包身份 `dev.envrelay.core.v0-2`、bundle 显示名 `EnvRelay Core`、app 目录 `EnvRelay Core.app`；签名脚本的 `expected_identifier` 必须与 plist 成对修改。
- 开发期环境变量 `MIGRATION_VAULT_DISPOSABLE_E2E_USER` → `ENVRELAY_DISPOSABLE_E2E_USER`。

### 3. Tier C 冻结清单

以下字面量**不得**改名，任何自动替换脚本必须排除：

- 全部 `migration-vault-*/vN` contract 标识符，包含本次新增的 `migration-vault-cli-argv-grammar/v6`；
- 全部 `migration-vault/...` crypto domain separator（HKDF info、MAC domain、tree digest 前缀等）；
- 容器 magic `MVJ1`/`MVJC`/`MVO`/`MVSE`/`MVTA`/`MVTL`/`MVP1`/`MVSR`/`MVL1` 与 `.mvb` 扩展名；
- `MV_*` 错误码；
- installer manifest 的组件逻辑名与安装路径（`migration-vault`、`bin/migration-vault`、`/opt/migration-vault/0.1.0/...`、`Application Support/Migration Vault/release/`、`/var/lib/migration-vault/release/`）；
- release 标识符 `dev.migration-vault.*.v0-1`、systemd unit `migration-vault-core.socket`/`.service`、发布 origin `https://releases.migration-vault.dev`；
- 凭据库 canonical item：`dev.migration-vault.trust-state.v1`、`MigrationVault/TrustState/v1`；
- `MIGRATION_VAULT_MCP_BEARER` 等作为 schema `const` 的环境变量名；
- `contracts/*.schema.json` 的 `title` 与 containment `const` 文本。

Tier C 的 `migration-vault` 自此是一个**与产品名脱钩的历史命名空间**。它不再表示产品叫什么，只表示“这段字节属于 v0.1 起冻结的序列化格式”。读到这些字面量的人不应推断产品名。

### 4. argv grammar 升 v6，不改 v1–v5

可执行名是 grammar 的一部分，因此新增 `docs/cli-argv-grammar-v0.6.json` + `contracts/cli-argv-grammar-v6.schema.json`，而不是就地编辑 v5。v6 与 v5 的唯一差异是 `executable` 字段：同样 23 个 variant、14 个 command class、同样的 path shape、response profile 与 mutation 标志。v1–v5 保持冻结历史，其摘要继续有效。

grammar 的 contract 标识符本身仍读作 `migration-vault-cli-argv-grammar/v6` —— 见第 3 条，标识符不随产品名改动。

## 已知后果

### A. 打包路径与可执行名暂时不一致

installer manifest 仍声明 `bin/migration-vault`，而 cargo 现在产出 `envrelay`。这是可接受的，因为：

1. 没有任何交叉校验把 grammar 的 `executable` 与 manifest 的 `cli_path`/角色名绑定；两者是独立合同。
2. 相关 release platform 目前全部是 `"status": "unsupported"`，且 `implementation_ready=false`，没有任何路径被真实安装。

**在首个 production 安装包冻结之前必须消解这一分歧**：届时要么把 manifest 的路径升版到 `bin/envrelay`（需要 installer-manifest v2 与全部受影响 fixture 重签），要么在安装时显式产出 `bin/migration-vault` 作为可执行名。选哪一条由发布合同的成本决定，但不能带着分歧发布。

### B. Skill 参考文件名维持 `references/cli-v0.1.md`

该路径是 canonical-skill-tree-manifest **v1 与 v2 两版** schema 的 `const`，被 `include_bytes!` 编译进 `migration-contracts`，并作为 `logical_id`/`package_path`/`install_locator`/`uninstall_locator` 出现在 6 份 installer payload fixture 中。改名需要 manifest v3 加全部 fixture 重签，超出本次范围，因此文件名保留、内容升到 v6。文件名中的 `v0.1` 指 Skill 树的编排版本，不指 grammar 版本。

### C. Skill 树摘要必须随文本重算

`SKILL.md`、`references/*.md` 的字节被 v2 manifest 逐个钉住，且 manifest 自身的 JCS 摘要又被 `migration-contracts` 的测试钉住。改这些文本必须同时重算 entry 摘要、`tree_digest` 与该 pinned 常量，否则 `current_canonical_skill_tree_binds_exact_v02_source_bytes` 失败。`evals/cases.yaml` 在 `excluded_qa_source_paths` 中，改动它不触发重算。

## 影响

- 冻结的 vault 与已签发 fixture 不受更名影响，密钥派生与解密路径逐字节不变。
- 文档中 `migration-vault` 与 `EnvRelay` 并存是设计结果而非疏漏；Tier C 出现处应按历史命名空间理解。
- 后续任何“统一命名”的清理提案，必须先给出 contract 升版与 fixture 重签方案，否则不予采纳。
