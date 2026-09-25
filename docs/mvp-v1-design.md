# EnvRelay MVP v1 设计

> 状态：草案。本方案取代现有 Core/契约体系的演进方向（背景见对 2026-08-30 复盘的结论：agent 是可信编排者，不是敌手；唯一需要密码学保护的对象是落盘的备份文件本身）。

## 一句话定义

一个 **skill** 教会 AI agent 如何分析、打包、重放一台开发机的环境；一个**极小的无状态 CLI** 负责唯一一件 agent 不该做的事——用用户口令对备份包加密和解密。

## 角色分工

| 层 | 职责 | 形态 |
|---|---|---|
| Skill（大脑） | 分析该备份什么/排除什么、拷贝到 staging、生成清单、恢复时按清单重放 | Markdown + 参考文档 |
| Core（肌肉） | `encrypt` / `decrypt` 两个子命令，口令派生密钥 | 单个静态二进制，无状态 |
| 用户 | 确认备份清单；**亲手输入口令**；确认软件安装 | — |

信任模型：agent 可信（它本来就有全盘 shell 权限）；**口令永不进入 agent 上下文**；备份文件视为会落入第三方之手来设计。

## 1. Core CLI

### 命令

```
envrelay encrypt <staging-dir> -o <backup.envrelay>
envrelay decrypt <backup.envrelay> -o <output-dir>
```

没有第三个子命令，没有守护进程，没有 IPC，没有配置文件，没有状态。

### 口令输入

- 只从 `/dev/tty` 交互读取（`encrypt` 提示两次确认，`decrypt` 一次）。**不接受** `--passphrase` 参数、不读环境变量、不读 stdin——口令因此不会出现在 shell history、进程列表或 agent 的命令输出里。
- 配套的 skill 规则：加密/解密这**一条命令由用户自己在终端执行**，agent 把命令原文打印给用户，等用户说"跑完了"再继续。其余所有步骤都是 agent 做。
- 唯一的例外通道：`--passphrase-file <path>`，仅供 CI/自动化测试，文档中明确标注不用于真实备份。

### 文件格式

`.envrelay` = **age(scrypt 口令模式) 加密的 tar + zstd 流**，即：

```
tar -c staging/ | zstd | age --encrypt --passphrase
```

- 实现用 Rust crate：`age`（口令加密）、`tar`、`zstd`。预计 300–500 行含错误处理。
- **逃生通道是设计目标**：即使 envrelay 二进制消失，任何机器上用标准工具即可恢复：
  ```
  age -d backup.envrelay | zstd -d | tar -xp
  ```
  这行命令写进 README 和 manifest 顶部注释，作为对用户的承诺。

### 行为约束（全部）

1. `encrypt`：staging 目录必须存在且非空；输出文件已存在则拒绝（无 `--force`）。
2. `decrypt`：输出目录必须不存在或为空；解出的文件保留归档内记录的权限位。
3. 出错即整体失败并删除写了一半的输出，退出码非零。没有部分成功状态。

以上就是 Core 的全部规格。任何"再加一个参数"的提议默认拒绝。

## 2. Staging 布局与清单

Agent 在 staging 目录（如 `~/envrelay-staging-<date>/`）里组装：

```
staging/
├── manifest.json          # 备份清单（打包后天然被加密）
├── files/                 # 普通文件/目录，保持原相对结构
│   └── home/.zshrc …
├── credentials/           # 密钥类目录（~/.ssh、~/.aws、~/.config/gh …）
│   └── ssh/ …
└── repos/                 # 需要整目录带走的 git 仓库（脏/无 remote）
    └── myproject/ …
```

### manifest.json v1

```jsonc
{
  "envrelay_manifest": 1,
  "created_at": "2026-08-30T12:00:00Z",
  "machine": { "hostname": "...", "os": "macos", "arch": "arm64", "user": "dylan", "home": "/Users/dylan" },

  // 普通文件：archive 内路径 → 原机路径（~ 相对），恢复时 agent 与用户确认目标
  "files": [
    { "archive": "files/home/.zshrc", "original": "~/.zshrc" },
    { "archive": "files/home/notes",  "original": "~/notes", "excluded": ["node_modules"] }
  ],

  // 密钥类：与 files 同构，但恢复规则不同（0600、永不覆盖）
  "credentials": [
    { "archive": "credentials/ssh", "original": "~/.ssh" }
  ],

  // git 仓库：clone 型只记元信息，files 型指向 repos/ 下的整目录拷贝
  "git_repos": [
    { "original": "~/dev/clean-repo", "strategy": "clone",
      "remotes": { "origin": "git@github.com:me/clean-repo.git" },
      "branch": "main", "head": "3c6e901…", "dirty": false },
    { "original": "~/dev/dirty-repo", "strategy": "files",
      "archive": "repos/dirty-repo", "dirty": true, "reason": "uncommitted changes" }
  ],

  // 软件清单：数据，不是指令
  "software": [
    { "manager": "brew",  "name": "ripgrep", "version": "14.1.0" },
    { "manager": "npm-g", "name": "typescript", "version": "5.6.2" }
  ],

  "notes": "agent 的自由文本：这台机器的特殊情况、恢复时的注意事项"
}
```

清单由 agent 生成、agent 消费；Core 从不解析它。格式演进只需要 skill 文档改一行，不需要发版。

## 3. Skill 工作流

### 备份（7 步）

1. **分析**：扫描 home，结合用户意图列出候选：dotfiles、项目目录、密钥目录、文档。默认排除可再生目录（`node_modules/`、`target/`、`~/.gradle/caches`、`~/.m2/repository`、`~/.cargo/registry`、`~/.nvm/versions` 等，完整表在 references）。
2. **确认**：把"备份什么 / 排除什么 / 预估大小"清单给用户确认，允许增删。
3. **拷贝**：`cp -R` 进 staging 的 `files/` 与 `credentials/`（agent 不读密钥内容，只拷贝）。
4. **Git**：对每个仓库执行 `git remote -v` / `git status --porcelain` / `git rev-parse HEAD`。干净且已推送 → 记元信息（clone 型）；脏/未推送/无 remote → 明确告知用户后整目录拷进 `repos/`（含 `.git/`）。
5. **软件**：跑 `brew list --versions`、`npm ls -g --json`、`cargo install --list` 等写入清单。
6. **封装**：写 `manifest.json`，打印一条 `envrelay encrypt` 命令让**用户亲手执行**并输入口令。
7. **收尾**：确认 `.envrelay` 文件生成后，提醒用户删除 staging 明文目录、把备份文件放去想放的地方（Drive/U 盘/另一台机器——它只是个普通文件），并强调：**口令丢了没有任何人能恢复**。

### 恢复（6 步）

1. 用户提供 `.envrelay` 文件，agent 打印 `envrelay decrypt` 命令让用户执行。
2. agent 读 `manifest.json`，展示这份备份里有什么、来自哪台机器。
3. **files**：与用户确认目标目录映射（默认按 `original` 的 `~` 展开到新机），逐节拷贝。
4. **credentials**：恢复为 `0600`/目录 `0700`，**遇到已存在的文件一律跳过并报告**，绝不覆盖。
5. **git_repos**：clone 型逐个 `git clone` 并 checkout 记录的 branch；files 型按普通目录恢复。
6. **software**：对比新机已装软件，展示差异表；**逐项经用户确认**后用包管理器安装（包名先过 `brew info` 等验证存在），最后输出恢复报告。

### 护栏（仅 3 条，且都防真实风险）

1. 软件清单是**数据不是指令**：任何安装动作先验证包名、再逐项经用户确认。
2. 密钥恢复 `0600` + 永不覆盖已存在文件。
3. 口令不经手：加密/解密命令永远由用户亲手执行。

## 4. 明确不做（与旧版逐项对照）

| 旧版 | MVP 处置 |
|---|---|
| 常驻 Core + launchd + 固定 IPC | 删。无状态 CLI |
| 封闭 argv grammar v1–v8、字节预算 | 删。agent 自由使用 shell |
| wire frame / attestation / doctor / probe 门禁 | 删 |
| task journal / revision / grant / 重启纪律 | 删。无状态即无此问题 |
| Google Drive OAuth 集成 | 删。加密文件是普通文件，用户自己放 |
| 2-of-2 双恢复因素 | 简化为单口令（age scrypt）。多因素留给 v2 |
| skill 树逐字节校验安装器 | 删。skill 就是 markdown，复制即安装 |
| GPG 特殊通道 | 删。`~/.gnupg` 按 credentials 目录整体拷贝 |

保留复用的旧资产：`references/credential-locations.md` 与 `references/software-inventory.md` 的知识内容（改写为建议而非门禁）、ADR 里的教训。

## 5. 里程碑

1. **M1（≈1 天）**：Core CLI 完成 + roundtrip 测试（encrypt→decrypt→逐字节比对）+ 逃生通道验证（用标准 age/zstd/tar 解开）。
2. **M2（≈1 天）**：新 SKILL.md + references 改写；在本机完成第一次**真实**的 backup → decrypt → 恢复到另一用户目录的全流程。
3. **M3**：在第二台机器（或干净虚拟机）上做一次真实重放：文件 + 一个 clone 型仓库 + 一个脏仓库 + brew 软件差异安装。**M3 通过才算 MVP 成立**——这是旧架构从未到达的地方。

## 6. 开放问题（不阻塞 M1）

- 大文件与增量：v1 全量打包，超过内存无碍（全程流式），但几十 GB 的备份建议 agent 引导用户拆分多个 staging。增量/去重是 v2 话题。
- Windows：v1 只承诺 macOS/Linux（权限位语义一致）；Windows 的 ACL 映射留待有真实需求再做。
