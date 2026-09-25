# ADR-021：推倒 v0.1/v0.2 体系，按 MVP v1 从零重写

## 状态

Accepted。取代 ADR-003、004、007、008、010、011、012、013、015、016、017、018、019 所确立的架构（其中记录的教训仍然有效，保留备查）。

## 日期

2026-08-30

## 背景

ADR-001 以来的整套设计建立在一个未被明说的假设上：**驱动备份的 agent 是潜在敌手**。为了约束它，我们先后造了常驻 Core、固定 IPC、封闭 argv 文法（v1 一路到 v8）、字节预算、wire frame、attestation、doctor/probe 门禁、task journal、grant、逐字节校验的 skill 树安装器、2-of-2 双恢复因素。

代价是：这套体系累积到九个 crate、二十条 ADR、八个版本的文法，而 ADR-021 之前它**从未完成过一次真实的跨机器重放**。每加一条防线就要同步改文法、改 wire、改 manifest、改 skill 树清单、再生哈希，一次功能改动被摊成五处协同修改，迭代速度趋近于零。

假设本身是错的。agent 运行在用户自己的 shell 里，本来就有全盘读写权限。用一个封闭 CLI 去限制一个已经能 `rm -rf ~` 的进程，防不住任何真实攻击者，只能拖慢一个可信的助手。

真正需要密码学保护的对象只有一个：**落盘之后可能流到第三方手里的备份文件**。

## 决策

1. **信任模型翻转**：agent 是可信编排者。它决定备什么、拷什么、怎么重放，用普通 shell 命令做事，不受封闭文法约束。
2. **唯一的安全边界是口令**。口令永不进入 agent 上下文：加密/解密命令由**用户亲手在终端执行**，Core 只从 `/dev/tty` 读口令（`--passphrase-file` 是唯一例外，仅供自动化测试）。
3. **Core 缩到两个子命令**：`envrelay encrypt <staging-dir> -o <file>` 与 `envrelay decrypt <file> -o <dir>`。无状态、无守护进程、无 IPC、无配置文件。仓库变成单一 crate。
4. **文件格式即逃生通道**：`.envrelay` = age(scrypt 口令模式) 加密的 `tar + zstd` 流。即使本二进制消失，`age -d backup.envrelay | zstd -d | tar -xp` 也能恢复。这是对用户的承诺，也是一条测试。
5. **清单是 agent 的私产**：`manifest.json` v1 由 skill 写、由 skill 读，Core 从不解析它。格式演进只改一行文档，不需要发版。
6. **skill 从门禁改为教学**：不再是"禁止清单 + 逐字节校验的安装器"，而是一份教 agent 把事做好的 Markdown，复制到 `~/.claude/skills/` 即安装。
7. **整体删除**：常驻 Core + launchd、封闭 argv 文法 v1–v8 与字节预算、wire frame/attestation/doctor/probe、task journal/revision/grant、Google Drive OAuth 集成、2-of-2 双恢复因素、skill 树逐字节校验安装器、GPG 特殊通道（`~/.gnupg` 按普通 credentials 目录整体拷贝）。

## 后果

- 备份文件在**离线**威胁模型下受 age(scrypt) 保护，与之前的自研封装等价；而在**本机在线**威胁模型下，我们不再假装能约束一个已经拥有 shell 的 agent。这是把一条守不住的承诺换成一条守得住的。
- 单口令取代 2-of-2：口令丢失即数据丢失，README 与 skill 都必须把这句话说在前面。多因素恢复留给 v2。
- 九个 crate 的实现、`contracts/`、`tests/`、`scripts/`、`npm/`、`packaging/` 与除 ADR 外的全部旧架构文档一并删除。历史留在 git 里，需要时可查。
- 判据也换了：不再以"文法契约测试全绿"为准，而是 ADR 之前从未到达的那一步——在第二台机器上完成一次真实重放（文件 + clone 型仓库 + 脏仓库 + 软件差异安装）。规格见 `docs/mvp-v1-design.md`。
