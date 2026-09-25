# ADR-016：新建 local vault 的目录由 Core 自己创建

## 状态

Accepted

## 日期

2026-08-24

## 背景

新建 local vault 的目录合同一直是三条同时成立的硬条件：属主等于 Core 的 effective uid、mode 逐位等于 `0700`、并且**完全为空**，之后还要通过一次真实的 create/write/fsync/remove 写探针。这三条不是装饰：

- vault 里是密文对象与 manifest，`0700` 挡住的是**同一台机器上的其他本地账户**。即使内容有 AEAD，group/other 可读的目录仍然泄露对象数量、大小分布与 commit 序列这类结构元数据。
- 更硬的一条是完整性。整个 vault 协议依赖"除属主外没人能在 root 下创建、改名或替换条目"。group/other 可写意味着另一个本地 uid 可以在 Core 两次重验之间掉包对象，而 no-follow 与 fd 指纹钉住的是**条目**，钉不住目录本身的可写性。
- 空目录要求把"新建 vault"和"续写既有 vault"分成两条不能互相伪装的路径，也保证 Core 永远不会覆盖用户已有的数据。

问题不在这三条，而在**谁来满足它们**。Core 只验证、不创建，于是这份负担落到了用户和 Agent 头上——而常规 umask `022` 下的 `mkdir` 得到的是 `0755`，正好是被拒绝的那一档。结果是每个人第一次用 local backup 都要先手动 `chmod 700`，而 [ADR-013](013-agent-selected-paths-and-local-permission-fallback.md) §3 又明令产品不得建议 `chmod`。这条"必须做但不许教"的缝隙，正是 agent-first 边界上不该存在的一步。

## 决策

### 1. Agent 提交的 vault 路径 = 用户选的容器目录 + Core 创建的叶子目录

- `--vault` 仍然是**一个**封闭的平台绝对目录路径，v6 argv grammar 的形状、变体与 placeholder 类型全部不变。
- 对 `backup_setup` 的新建分支，Core 把该路径拆成 parent 与最后一段：parent 是用户选定的容器目录（例如 `~/Documents/EnvRelay`），最后一段是 EnvRelay 独占的 vault 目录。
- Core 以 no-follow 逐段打开 parent，然后在 parent 的 fd 上 `mkdirat(..., 0o700)` 创建叶子。
- `mkdirat` 同样受 umask 约束，因此**仅当本次调用确实创建了该目录**时，Core 才通过共享的 `dirfd::set_mode` 在已打开的 handle 上把 mode 设回 `0700`，补回被 umask 清掉的位。
- 创建之后仍然跑完全套原有验证：属主、逐位 `0700`、空目录、真实写探针。合同一位不放宽。

### 2. 不是 Core 创建的目录，一律不碰

- 叶子已存在时（`EEXIST`）Core **不** `fchmod`、不 `chown`、不清空，只让它按原样接受同一份验证。`0755` 的既有目录照旧 fail closed。
- ADR-013 §3 的 "Core 不执行 `sudo`、`chmod`、`chown`" 继续有效，指向的是**用户已有的目录**。在用户选定的容器里新建一个自己的私有目录，与绕过权限拒绝无关。

### 3. 权限回退请求的是容器目录

- 新建 vault 时唯一能被用户授权的目录是 parent——vault 本身此刻还不存在。
- 因此只有打开 parent 或在其中创建叶子返回权限拒绝时，Core 才打开 native panel，请用户授权**同一个 parent**；授权后重新创建并重验。
- 用户取消、或 panel 选的不是 Agent 请求的那个 parent，任务保持等待，provider mutation 为零。

### 4. Local 模式下 vault 必须留在用户能操作的位置

- `0700` 挡的是其他本地账户，不是用户自己。属主是当前用户，用户与用户自己运行的程序（Finder、Google Drive for Desktop、`cp`）都能读写这个目录。Core 在 vault 生命周期内持有的 `flock` 是 advisory 的，同样不阻止这些程序。
- 容器目录由用户提供，因此 vault 必然落在用户可见、可操作的位置；Core 不把 local vault 放进 Core-private state root。
- 但"能操作"不等于"可以手工搬运"。重开 vault 时每层目录仍要求逐位 `0700`、每个文件逐位 `0600` 且 `nlink == 1`、属主等于 euid，任一条不满足都 fail closed——vault root 自身按 `InvalidRoot`，root 之下的目录与文件按 `NamespaceDrift`。云端文件夹同步不保存 Unix mode，落回本地时按当时的 umask 重建（`022` 下就是 `0755`/`0644`），因此**把 vault 目录本身同步上云再同步回来，注定打不开**。裸 `cp -R`、zip、以及任何不保留 mode 的搬运方式同理。
- 要把 local 备份送上 Google Drive，走 [ADR-011](011-dual-google-drive-layouts-and-portable-vault-bundle.md) 的 `google-drive-portable`：Core 自己把闭包打包成单个 `.mvb` 再上传，mode 语义不出现在传输面上。用户若只是想自留一份归档，必须用保留 mode 的容器（`tar`、`ditto --rsrc`、磁盘映像）。

## 验收案例

1. **常规容器**：容器目录存在且 mode 为 `0755`，Agent 提交 `<容器>/<vault 名>`；Core 创建该目录、mode 逐位 `0700`、为空、通过写探针，全程不弹 picker，也不修改容器目录自身的 mode。
2. **umask 无关**：创建出的 vault 目录 mode 逐位等于 `0700`，与调用方的 umask 无关。这一条不靠 `mkdirat` 的 mode 参数保证——那个参数本身会被 umask 削掉——而是靠**本次调用确实创建了该目录**时，在已打开的 handle 上把 mode 设回 `0700`；叶子已存在时不改，见案例 4。
3. **幂等重试**：对同一路径重复执行创建，第二次走 `EEXIST` 分支并照常通过验证（此时目录仍为空）。
4. **既有目录不被放宽**：叶子已存在且 mode 为 `0755` 时 fail closed，且该目录的 mode 在失败后逐位不变。
5. **既有非空目录**：叶子已存在且非空时返回 `MV_POLICY_SCOPE_EXCEEDED`，不弹 picker，不覆盖任何条目。
6. **权限回退**：parent 不可写时才弹 panel，且 panel 请求的是 parent；用户授权同一 parent 后重新创建并重验，任务继续。取消则 provider mutation 为零。
7. **路径边界不变**：空、相对路径、`/`、`.`/`..`、NUL、超限、symlink/reparse、parent 不存在，全部在创建前 fail closed。
8. **不在备份源里建目录**：vault 落在 backup source 之内时返回 `MV_POLICY_SCOPE_EXCEEDED`，且该判定发生在 `mkdirat` **之前**——被拒绝的任务不会在下一次要备份的那棵树里留下一个 EnvRelay 创建的目录。重叠判定用的是已打开的 source 与容器目录的 fd 身份，不是路径文本前缀。
9. **restore 不变**：Local restore unlock 的 `--vault` 仍指向已存在的 vault 目录，走 `validate_existing_vault_directory`，绝不创建。

## 影响

- [ADR-013](013-agent-selected-paths-and-local-permission-fallback.md) §2/§3 对**新建 local vault** 这一种目录被本 ADR 窄化：Core 从"只打开并验证"变为"创建并验证"，权限回退的对象从 vault 自身变为其容器目录。其余目录（backup source、restore target、既有 vault）完全不受影响。
- v6 argv grammar、CLI 形状、IPC `LocalActionPaths`、wire 合同与 `0700`/空目录/写探针的验证合同全部不变，因此不需要 v7。
- Skill 侧的措辞从"选一个已经建好的空目录"变为"向用户要一个容器目录，再提交容器下的 vault 路径"。
- [skill-and-cli-contract](../skill-and-cli-contract.md) 与 [security-format-v1](../security-format-v1.md) 中平铺的 "Core 不执行 sudo、chmod、chown" 一句被本 ADR 窄化：该禁令指向的是用户已有的目录，Core 只对自己刚创建的 vault 叶子补 mode。
