# EnvRelay

[English](README.md) | 简体中文

**一个 skill 教会 AI agent 如何分析、打包、重放一台开发机的环境；一个极小的无状态 CLI 负责唯一一件 agent 不该做的事——用你的口令给备份包加密和解密。**

换电脑时最花时间的不是拷文件，是回想：装过什么、配过什么、哪个仓库还有没推的改动、哪些密钥不能丢。这件事本质上是分析和判断，正好是 agent 擅长的。而口令绝不能进 agent 的上下文——所以有了这个二进制。

| 层 | 职责 | 形态 |
|---|---|---|
| Skill（大脑） | 决定备份什么、排除什么、拷进 staging、写清单、恢复时按清单重放 | Markdown + 几个确定性脚本 |
| Core（肌肉） | `encrypt` / `decrypt` / `inspect` / `verify` 四条命令 | 单个二进制，无状态 |
| 你 | 确认清单；**亲手输入口令**；确认软件安装 | — |

> **状态：MVP v1。** Core 与 skill 都已就位，本机往返可用；规格见 [`docs/mvp-v1-design.md`](docs/mvp-v1-design.md)，本次重写的理由见 [ADR-021](docs/decisions/021-mvp-v1-reset.md)。在你自己完整跑过一次之前，别拿唯一一份重要数据当第一个实验对象。

---

## 安装

macOS 或 Linux 上：

```bash
curl -fsSL https://envrelay.com/install.sh | sh
```

不需要 sudo。envrelay.com 会把你转到[最新 release](https://github.com/FutrixDev/envrelay-skill/releases/latest) 里的安装脚本，脚本再从同一个 release 下载适合你这台机器的 `envrelay` 二进制和 skill，任何一个文件和 release 里的 `SHA256SUMS` 对不上就拒绝安装。然后它会：

- 把 `envrelay` 放进 `~/.local/bin`，如果这个目录还不在 `PATH` 里，就在你的 shell 配置文件里加上；
- 把 skill 放进 `~/.agents/skills/envrelay`——Codex、Cursor、Gemini CLI、GitHub Copilot、OpenCode 等大多数 agent 都从这里找 skill——再为 Claude Code 链接一份到 `~/.claude/skills`（用 Kiro 或 Cline 的话，也链接到 `~/.kiro/skills`、`~/.cline/skills`）；
- 检查 skill 的脚本要用的 python3 3.9+ 和 git 在不在。

然后开一个新的 coding agent 会话，对它说：

> 用 EnvRelay 把这台机器的开发环境备份下来。

到了新机器上，跑同一条命令，把备份文件拷过去，再说：

> 从 backup.envrelay 恢复我的开发环境。

想先看看脚本的话，[这里是源码](skills/envrelay/install.sh)；也可以让它只说明会做什么，不下载、不改动任何东西：

```bash
curl -fsSL https://envrelay.com/install.sh | sh -s -- --dry-run
```

选项写在 `sh -s --` 后面：

| 选项 | 作用 |
|---|---|
| `--version X.Y.Z` | 安装指定版本，而不是最新版 |
| `--bin-dir DIR` | 把 `envrelay` 放进 `DIR` 而不是 `~/.local/bin`（也可以设 `ENVRELAY_BIN_DIR`） |
| `--bin-only` | 只装二进制，不装 skill |
| `--no-modify-path` | 不改任何 shell 配置文件 |
| `--dry-run` | 只说明会做什么，不下载、不改动 |
| `--uninstall` | 移除安装器装上的东西。备份永远不碰 |

再跑一次就是更新。它只替换自己装的东西：别的工具放在那里的 skill 目录或链接（`npx skills` 装的、你自己拷的）原样不动，并告诉你一声。每个 release 文件都带构建来源证明，envrelay.com 指向的 `install.sh` 也一样，可以用 `gh attestation verify <文件> --repo FutrixDev/envrelay-skill` 核对。

每次从 envrelay.com 下载 `install.sh` 都会留下记录：时间、IP 地址、该地址所属的国家和网络，以及客户端的 User-Agent。

### 需要什么

- **macOS 11 及以上，或 Linux**，x86_64 或 arm64。Linux 版是静态链接的，任何发行版都能跑。Windows 请用 WSL。
- **python3 3.9+ 和 git**，skill 的脚本要用。Mac 上这两样都来自苹果的 Command Line Tools：缺的话，安装器会打开苹果的安装程序，你点一下“安装”即可。Linux 上它会告诉你该跑哪条包管理器命令。
- **一个能读 Agent Skills 的 coding agent**：Claude Code、Codex、Cursor、Gemini CLI、GitHub Copilot、OpenCode 等等。

就这些。你平时用的包管理器（Homebrew、npm、cargo 等）只在你想让 agent 在新机器上把软件装回来时才用得上；`age` 和 `zstd` 只有走[逃生通道](#逃生通道)时才需要。

### 其他安装方式

下面这些方式只装 skill。第一次用的时候，skill 会发现 `envrelay` 还没装，告诉你它是什么，经你同意后运行它自带的安装器（`install.sh --bin-only`：同样校验 checksum，装的是和 skill 版本对应的 release）。

任何 agent，通过 [skills.sh](https://skills.sh)：

```bash
npx skills add FutrixDev/envrelay-skill -g
```

Claude Code，作为插件：

```
/plugin marketplace add FutrixDev/envrelay-skill
/plugin install envrelay@envrelay
```

GitHub Copilot CLI 读的是同一个插件市场：

```bash
copilot plugin marketplace add FutrixDev/envrelay-skill
copilot plugin install envrelay@envrelay
```

GitHub CLI 2.90 及以上（`--agent` 也接受 `codex`、`cursor`、`gemini-cli`、`github-copilot` 等）：

```bash
gh skill install FutrixDev/envrelay-skill envrelay --agent claude-code --scope user
```

[ClawHub](https://clawhub.ai)，给 OpenClaw 用。OpenClaw 本来就读 `~/.agents/skills`，所以上面那条一键安装命令已经覆盖了它；想从 ClawHub 装的话：

```bash
openclaw skills install @futrixdev/envrelay --global
```

从源码装，需要 Rust 1.97 以上。`cargo install` 把 `envrelay` 装进 `~/.cargo/bin`，两个软链让 Claude Code 和所有读 `~/.agents/skills` 的 agent 都能看到 skill：

```bash
git clone https://github.com/FutrixDev/envrelay-skill
cd envrelay-skill
cargo install --path . --locked
mkdir -p ~/.agents/skills ~/.claude/skills
ln -s "$PWD/skills/envrelay" ~/.agents/skills/envrelay
ln -s "$PWD/skills/envrelay" ~/.claude/skills/envrelay
```

---

## 四条命令

```bash
envrelay encrypt <staging-dir> -o <backup.envrelay>
envrelay decrypt <backup.envrelay> -o <output-dir>
envrelay inspect <backup.envrelay>   # 列出内容、打印 manifest，不写盘
envrelay verify  <backup.envrelay>   # 从头读到尾：口令正确、每个字节完好，不写盘
```

没有第五条命令，没有守护进程，没有配置文件，没有状态。`inspect` 和 `verify` 是只读的：恢复前先确认"这个文件是不是我要的、还能不能打开"，而不必真的解到盘上。

```bash
# agent 把要带走的东西整理进 ~/envrelay-staging-20260830/ 之后，你亲手跑：
envrelay encrypt ~/envrelay-staging-20260830 -o ~/backup.envrelay
# 提示输入口令两次

# 新机器上：
envrelay decrypt ~/backup.envrelay -o ~/envrelay-restore
# 提示输入口令一次，然后让 agent 接管恢复
```

规则很少，但都是硬的：

- **口令只从 `/dev/tty` 读**，没有 `--passphrase` 参数、不读环境变量、不读 stdin。所以口令不会出现在 shell history、`ps` 输出，或者 agent 的命令回显里。（`--passphrase-file` 存在，但只给本项目的自动化测试用，帮助文本里也是这么写的。）
- **`encrypt` 不覆盖已存在的文件**，没有 `--force`。
- **`decrypt` 只写进不存在或空的目录**，解出来的文件保留归档里记录的权限位。
- **出错就整体失败**：写了一半的输出会被删掉，退出码非零。没有"部分成功"这种状态。

加密后记得删掉 staging 目录——那是同一份数据的明文副本。

---

## 逃生通道

`.envrelay` 文件就是 **age（scrypt 口令模式）加密的 `tar + zstd` 流**。这不是实现细节，是承诺：即使这个二进制从地球上消失，任何一台装了标准工具的机器都能把备份打开。

```bash
age -d backup.envrelay | zstd -d | tar -xp
```

仓库里有一条测试专门验证这一点——它绕开所有 envrelay 代码，直接用 age、zstd、tar 三个格式的参考实现库把备份读回来。

备份文件是**普通文件**。Google Drive、U 盘、另一台机器、同时放三个地方——加密不关心它在哪，这正是重点。

---

## ⚠️ 口令丢了，数据就没了

没有密钥托管，没有找回流程，没有后门，没有任何人能帮你——包括我们。scrypt 从你的口令派生密钥，除此之外这个文件里没有第二把钥匙。

把口令存进密码管理器，现在就存。

---

## 备份里有什么

Agent 在 staging 目录里组装出这样一棵树，然后你加密它：

```
staging/
├── manifest.json          # 备份清单，打包后天然被一起加密
├── files/                 # 普通文件/目录，保持原相对结构
│   └── home/.zshrc …
├── credentials/           # 密钥类目录（~/.ssh、~/.aws、~/.config/gh …）
│   └── ssh/ …
└── repos/                 # 需要整目录带走的 git 仓库（脏的/没 remote 的）
    └── myproject/ …
```

`manifest.json` 由 agent 写、由 agent 读，Core 从不解析它。格式见 [`skills/envrelay/references/manifest.md`](skills/envrelay/references/manifest.md)。

Skill 的判断逻辑分散在几份参考文档里，都是人也能读的：

| 文档 | 内容 |
|---|---|
| [`SKILL.md`](skills/envrelay/SKILL.md) | 备份 8 步、恢复 7 步、四条硬规则 |
| [`references/what-to-carry.md`](skills/envrelay/references/what-to-carry.md) | 该带走的东西去哪里找（dotfile、工具配置、工作目录、凭据目录、macOS 保护的应用数据），以及怎样放进 staging |
| [`references/exclude-list.md`](skills/envrelay/references/exclude-list.md) | 哪些目录不该备份（`node_modules/`、`target/`、各类缓存），以及"配置随身走、依赖留原地重建"的原则 |
| [`references/credential-locations.md`](skills/envrelay/references/credential-locations.md) | 密钥目录清单与恢复规则（0600、永不覆盖） |
| [`references/software-inventory.md`](skills/envrelay/references/software-inventory.md) | 各包管理器的枚举与安装命令 |
| [`references/git-repos.md`](skills/envrelay/references/git-repos.md) | 仓库五态判定与三种备份策略 |
| [`references/ai-agents.md`](skills/envrelay/references/ai-agents.md) | AI coding agent（Claude Code、Codex、Cursor 等）的五个平面：agent 本体、配置、扩展、工作状态（会话与记忆）、凭据；哪些随身走、哪些按清单重装、哪些留下 |
| [`references/manifest.md`](skills/envrelay/references/manifest.md) | manifest.json v3 格式与恢复状态词汇表 |

判断留在 skill 里，纯机械的部分沉淀成了 [`skills/envrelay/scripts/`](skills/envrelay/scripts/) 下的六个单文件 Python 脚本（staging 拷贝、仓库分类、软件清单、AI agent 清点、恢复台账、校验和比对）——脚本只干活不做主，划分标准见 [ADR-022](docs/decisions/022-deterministic-mechanics-scripts.md)；AI agent 状态为什么自成一类见 [ADR-023](docs/decisions/023-ai-agent-state.md)。

---

## 信任模型

agent 可信——它运行在你自己的 shell 里，本来就有全盘读写权限，用一个封闭 CLI 去限制它防不住任何真实攻击者。**唯一需要密码学保护的对象是落盘之后可能流到第三方手里的备份文件本身**，所以边界只有一条：口令永不进入 agent 上下文，加解密命令由你亲手执行。

这套判断推翻了本项目此前的整个架构，理由写在 [ADR-021](docs/decisions/021-mvp-v1-reset.md) 里。

**平台**：macOS 与 Linux（权限位语义一致）。Windows 的 ACL 映射等有真实需求再说。

---

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
sh tests/installer.sh
sh .github/scripts/check-versions.sh
```

`tests/installer.sh` 用当前 checkout 打出来的 release 跑安装器，每个场景一个一次性的 `HOME`，所以要先 build release。安装器只从 GitHub 下载，所以由一个替身 curl（[`tests/stubs/curl`](tests/stubs/curl)）代替 GitHub 提供这个 release。`SH=dash sh tests/installer.sh` 换一个 shell 跑安装器；CI 在 sh、dash、bash、zsh 下各跑一遍。`check-versions.sh` 检查所有写了版本号的地方是否一致。

## 发版

1. 在 `Cargo.toml`、[`skills/envrelay/SKILL.md`](skills/envrelay/SKILL.md) 的 `metadata.version`、[`.claude-plugin/plugin.json`](.claude-plugin/plugin.json)、[`plugin.json`](plugin.json) 四处改成新版本号，再跑一次 `cargo build` 让 `Cargo.lock` 跟上。四处不一致时 CI 会失败。
2. 合并之后，马上给 GitHub 上 main 的合并提交打 tag 并推送（release 出来之前，从 main 装的 skill 会去找一个还不存在的 release），例如：

   ```bash
   git fetch origin
   git tag v1.0.1 origin/main
   git push origin v1.0.1
   ```

   [release workflow](.github/workflows/release.yml) 会为 macOS（一个通用二进制）和 Linux（静态链接，x86_64 与 arm64）构建 envrelay，在每个平台上安装打好的包验证一遍，再带着构建来源证明发布。此后 envrelay.com 上的 `install.sh` 就指向这个新 release，[smoke workflow](.github/workflows/smoke.yml) 随后在 macOS 和 Linux 上用那条一行命令从网站安装一遍。
3. release workflow 的 *publish* job 通过之后，由维护者把首页上的版本号改成新版本，重新部署 envrelay.com（网站在单独的仓库里，见 [ADR-025](docs/decisions/025-site-in-its-own-repository.md)），首页从这时起才写新版本号。`install.sh` 不用部署，它总是指向最新 release。
4. 把 skill 发布到各个 registry：见 [`docs/publishing.md`](docs/publishing.md)。

---

## 许可

MIT OR Apache-2.0。见 [LICENSE-MIT](LICENSE-MIT) 与 [LICENSE-APACHE](LICENSE-APACHE)。
