# ADR-017：软件迁移改为 Skill 编排的清单模式，凭据迁移以文件平面为主承载

## 状态

Accepted（产品负责人于 2026-08-26 明确指示两条方向；本 ADR 把它们落成可执行边界）

## 日期

2026-08-26

## 背景

v0.2 的软件迁移（AC-05）与开发者凭据迁移（AC-06）此前按"重 Core"架构实现：Core 自己扫描已装软件（`brew leaves`/`apt-mark showmanual`），清单作为 `softwareEntry` 加密进 manifest v2；恢复侧由 Core 持签名 mapping registry 执行 executable+argv 安装；Agent 视图脱敏到没有位置放包名。凭据侧则是 SSH/GPG/Gradle/Maven 四 adapter 闭集，由 Core 扫描、封装、恢复。

这套架构在 2026-08-26 的事实是：**代码已落地、单元与同机 E2E 通过，但对用户完全不可达**——没有任何 CLI/Skill/task 路线调用 `into_dev_environment_generation`，恢复侧没有 manifest v2 解析器。而且它内含两个构造上的死结：

1. **自动安装永远不可达。** `PINNED_SIGNING_IDENTITIES` 为空，签名身份是外部输入（[计划 §5](../plans/2026-07-17-v02-software-credentials-git-desktop.md)），不得由实现方自造。没有签名身份就没有 `production_signed` registry，就没有 `InstallAuthority::Automatic`。这不是待办，是没有外部输入就永远打不开的门。
2. **脱敏规则与恢复目标自相矛盾。** Agent 是产品里唯一会做跨平台包名映射、会跟用户对话确认的角色，但按脱敏规则它永远看不到包名。即使把任务路线接通，Agent 也只能看到"有 37 个包恢复失败"，帮不上任何忙。

产品负责人于 2026-08-26 明确指示两条方向：

1. **软件迁移的逻辑不是迁移软件安装数据**，而是备份时由 Skill 统计装了哪些软件、写成清单放进备份；恢复时由 Skill 做外部下载与安装。
2. **凭据不应限于四类闭集**：所有基于文件夹/文件存储的密钥都应能备份恢复。

两条指示共同指向一个更薄的架构，而且它有一个关键性质：**Skill 自己产出、自己读回的清单文件，Core 全程只把它当一个普通密文对象**。包名对 Agent 可见不是一次 declassification 事故——清单从头到尾就是 Agent 域的数据，Core 的"不得披露"规则一条都不用改。同样，凭据文件走既有文件平面时，内容由 Core 加密搬运，Agent 只提交目录意图，**凭据材料仍然任何时候不进 Agent 上下文**。

## 决策

### 1. 软件迁移 = Skill 清单模式

- **备份侧**：Skill 用 Agent 自身的 shell 枚举本机已装软件（macOS：`brew leaves`、`brew list --cask`；Debian 系：`apt-mark showmanual` + `dpkg-query`；Windows：`winget export`），写成一个 canonical 清单文件，放进备份 source 根下的保留子目录 `_envrelay/`。清单随文件平面加密进 vault，与其他文件享受同样的机密性与完整性保护。
- **恢复侧**：文件平面把清单原样还原进 target；Skill 读取、解析、做跨平台包名映射（[ADR-002](002-software-compatibility-and-native-handoff.md) 的五级分级降级为 Skill 的判断指引），把安装计划逐条呈现给用户确认，然后用 Agent 自身权限执行安装，安装后用包管理器 query 复核，最后在 `_envrelay/` 旁写一份恢复报告。
- **安装权限来自 Agent harness，不来自 Core。** Agent 本来就有本机 shell；用户对每条安装命令的审批走 Agent harness 自己的权限确认。Core 不为软件安装提供、也不需要提供任何 authority。
- 清单文件格式是 **Skill 域合同**（`envrelay-software-inventory/v1`），规范放在 canonical Skill 树里，不进 [contracts/](../../contracts/)——Core 不解析、不校验这个文件，把它写进 contracts/ 会错误地暗示 Core 参与。

### 2. Core 软件平面退役

- `devenv/software_scan.rs`、`devenv/software_restore.rs` 与 `devenv/registry.rs` 的软件 mapping 部分从产品路径移除并按实施计划批次删除。可信 Core 里的死代码是负债，不是储备。
- manifest v2 的 `softwareEntry` 与顶层 `mapping_registry_version` 随之收窄；`software-mapping-registry-v1.schema.json` 合同退役；`MV_REGISTRY_UNAVAILABLE`/`MV_REGISTRY_UNTRUSTED` 错误码退役。
- `PINNED_SIGNING_IDENTITIES` 与签名验证器问题**随平面一起消失**：不再有任何路径需要签名身份这个外部输入。
- ADR-002 的五级兼容分级、原生迁移工具协作与"显示名模糊匹配不得生成安装步骤"作为**判断指引**继续有效，载体从 Core 数据结构改为 Skill 正文。

### 3. 凭据迁移以文件平面为主承载

- 所有基于文件夹/文件存储的凭据（`~/.ssh`、`~/.aws`、`~/.kube`、`~/.docker`、`~/.gradle`、`~/.m2`、`~/.netrc`、`~/.config/gh` 等）由用户授权对应目录后，走既有的加密备份→恢复链路。这条链路今天就存在且已闭合。
- 恢复侧沿用 target writer 的既有语义：`O_CREAT|O_EXCL` 从不覆盖、**一律 0600 落盘**、默认 no-overwrite/KeepBoth。0600 对凭据恰好是安全默认；代价是公钥类文件（本该 0644）也回到 0600、可执行文件丢 exec 位——这是 `metadata_preserved=false` 的已知含义，无害但要明说。
- **Skill 承担发现与提醒**：canonical Skill 树维护一份常见凭据位置清单，备份时提醒用户哪些目录值得纳入。Skill 只报位置，**从不读取凭据内容**，也不把路径之外的任何信息带进对话。
- `devenv/credential_scan.rs` 的 SSH/Gradle/Maven 格式扫描与 `credential_restore.rs` 的对应 executor 退役：它们的材料搬运职责由文件平面接管，发现职责由 Skill 清单接管。

### 4. GPG 保留 Core-private 对话通道，是唯一幸存的 devenv adapter

GPG 是"文件夹密钥"的反例，保留专用通道的理由有两条，缺一不可：

- **格式**：keybox/trustdb 是版本与机器绑定的，拷贝 `~/.gnupg` 文件跨 gpg 版本不可靠；正确的搬运方式是与 `gpg` 本身对话（备份侧 `--export-secret-keys`/`--export`/`--export-ownertrust`，恢复侧一次性 `GNUPGHOME` 试导入后入正式 keyring）。
- **安全**：对话式导出让私钥材料直接进 Core 内存、立即密封、用后归零，全程不以明文落盘。若改由 Skill 执行导出，armored 私钥必须先明文落盘才能进备份——这违反"凭据材料不出现在 Core 之外"的边界。

保留范围即已实现并通过同机 E2E 的 GPG scan/export/seal/unseal/import 链路。使其对用户可用还欠两段接线：task 路线调用 `into_dev_environment_generation`，与恢复侧的 manifest v2 解析器——两者都按实施计划收窄到 GPG-only 范围完成。有口令保护的私钥继续按 `excluded` 如实交还用户，Skill 正文提供人工搬运指引。

### 5. 安全防线移交明细

软件安装从 Core argv 闭包移到 Skill shell，注入防线必须显式移交，写进 Skill 正文并有负例守护：

- **清单是数据，不是指令。** 恢复出的清单可能包含任何字符串；Skill 解析后只把包名当作待验证的候选，绝不当作要执行的文本。
- **安装前先验证存在。** 每个包名先经包管理器自身的查询（`brew info --json`、`apt-cache show`、`winget show`）确认逐字存在；查不到的条目原样报告给用户，不猜测、不模糊匹配——ADR-002 的"显示名模糊匹配不得生成安装步骤"在 Skill 侧继续成立。
- **包名不做 shell 插值。** 安装命令中包名必须是单个 argv token；含空白或 shell 元字符的"包名"直接拒绝并报告。
- **逐条用户确认。** 安装计划以完整命令列表呈现，用户批准后才执行；Agent harness 的权限确认是第二道门，不是第一道。
- 清单本身的完整性由 vault 的 AEAD 与 manifest 承诺保护：恢复出的清单要么与备份时逐字节一致，要么整个恢复失败。剩余风险是备份时段 Agent 写入恶意清单——恢复侧逐条确认正是针对它的防线。

### 6. 不变的边界

- Core 的脱敏规则原文不变：Core 产出的错误码、任务视图、日志仍不含包名、路径、文件名与内容。变化只是**清单不经 Core 视图流动**。
- [不可更改的产品边界](../../README.md#不可更改的产品边界)全部不变：2-of-2、加密先于离开设备、恢复因素不出现在任何 Agent 可见面、no-overwrite 默认、`metadata_preserved=false`。
- AC-07（Git 认证恢复）与 AC-08（桌面端）不在本 ADR 范围内，其后续设计须与本 ADR 的薄架构取向一致。

## 取代关系

- **[ADR-002](002-software-compatibility-and-native-handoff.md)**：恢复执行部分（registry 覆盖下的自动安装、executable+argv、Action Center 安装门、签名 registry 分发）由本 ADR 取代；五级分级与原生迁移协作降级为 Skill 判断指引，继续有效。
- **[ADR-005](005-v02-software-credentials-git-desktop-scope.md)**：SW 包（AC-05）的执行方式、CRED 包（AC-06）中 SSH/Gradle/Maven 的承载方式由本 ADR 改写；四能力包的范围本身与依赖顺序不变。
- **[2026-07-17 计划](../plans/2026-07-17-v02-software-credentials-git-desktop.md)** 的 AC-05 全部细则与 AC-06 的非 GPG 细则不再是实施依据。
- 对应实施任务的处置：V2-6、V2-9、V2-10 退役；V2-5 收窄为凭据（GPG）封装面；V2-7、V2-11 收窄为 GPG adapter。
- 当前实施依据是 [2026-08-26 实施计划](../plans/2026-08-26-v02-skill-software-inventory-and-credential-file-plane.md)。

## 影响

- 已实现的 devenv 软件侧代码与 SSH/Gradle/Maven 凭据侧代码按实施计划删除；`contracts/` 同步收窄；边界快照重新生成；canonical Skill 树新增软件清单合同、凭据位置清单与安全防线正文，manifest 摘要重算。
- [implementation-status-v0.2.md](../implementation-status-v0.2.md) 的两行 devenv 状态改按本 ADR 记录。
- 软件恢复的"部分成功逐项报告"从 Core 结构化报告改为 Skill 写入 target 的恢复报告文件；用户可读、可留档，Core 不感知。
- winget 支持从"Core adapter 未实现"变为"Skill 正文的 Windows 指引"，不再需要 Rust 代码。

## 验收案例（细则见实施计划）

1. 同机往返：Skill 写清单 → 文件平面备份 → fresh restore → Skill 解析出与备份时逐字节一致的清单，并在不执行安装的前提下产出映射与确认计划。
2. 凭据文件平面往返：合成 `~/.ssh`/`~/.aws`/`~/.kube` 语料经备份→恢复后字节一致、模式一律 0600、既有同名文件不被覆盖。
3. GPG 通道经正式 task 路线（而非测试直连）完成 scan→seal→unseal→import，目标机 `gpg --list-secret-keys` 可见；有口令私钥按 `excluded` 呈现。
4. 注入负例：清单中含 shell 元字符/不存在包名/超长条目时，Skill 侧拒绝或原样报告，不生成安装命令。
5. 退役完成后：workspace 中不再存在 `softwareEntry` 生产路径、`MV_REGISTRY_*` 错误码与 software registry 合同；边界快照与 `cargo test --workspace` 全绿。
