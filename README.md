# EnvRelay

English | [简体中文](README.zh-CN.md)

**An Agent Skill teaches your coding agent to analyse, pack and replay a development machine's environment. A tiny, stateless CLI does the one job the agent must not: encrypting and decrypting the backup with your passphrase.**

When you move to a new computer, the slow part is not copying files. It is remembering: what you installed, what you configured, which repository still has unpushed changes, which keys you cannot afford to lose. That is analysis and judgement, which is what agents are good at. A passphrase, though, must never enter an agent's context, and that is what the binary is for.

| Layer | Job | Form |
|---|---|---|
| Skill (the brain) | Decides what to back up and what to leave out, copies it into staging, writes the manifest, and replays it on restore | Markdown and a few deterministic scripts |
| Core (the muscle) | Four commands: `encrypt` / `decrypt` / `inspect` / `verify` | One binary, no state |
| You | Approve the list; **type the passphrase yourself**; approve software installs | — |

> **Status: MVP v1.** Core and the skill are both in place, and a round trip on one machine works. The spec is [`docs/mvp-v1-design.md`](docs/mvp-v1-design.md) (in Chinese), and [ADR-021](docs/decisions/021-mvp-v1-reset.md) explains this rewrite. Until you have run a full round trip yourself, don't make your only copy of anything important the first experiment.

---

## Install

On macOS or Linux:

```bash
curl -fsSL https://envrelay.com/install.sh | sh
```

No sudo. envrelay.com sends you to the installer in the [latest release](https://github.com/FutrixDev/envrelay-skill/releases/latest), and the installer downloads the `envrelay` binary for your machine and the skill from that same release, refusing any file that does not match the release's `SHA256SUMS`. Then it:

- puts `envrelay` in `~/.local/bin`, and adds that directory to `PATH` in your shell's rc file if it is not there yet;
- puts the skill in `~/.agents/skills/envrelay`, where Codex, Cursor, Gemini CLI, GitHub Copilot, OpenCode and most other agents look for skills, and links it into `~/.claude/skills` for Claude Code (and into `~/.kiro/skills` and `~/.cline/skills` if you use Kiro or Cline);
- checks for python3 3.9+ and git, which the skill's scripts use.

Then start a new session of your coding agent and say:

> Back up this machine with EnvRelay.

On the new machine, run the same command, bring the backup file over, and say:

> Restore my environment from backup.envrelay.

[Read the script](skills/envrelay/install.sh) first if you like, or have it print what it would do without downloading or changing anything:

```bash
curl -fsSL https://envrelay.com/install.sh | sh -s -- --dry-run
```

Options go after `sh -s --`:

| Option | Effect |
|---|---|
| `--version X.Y.Z` | Install that release instead of the latest one |
| `--bin-dir DIR` | Put `envrelay` in `DIR` instead of `~/.local/bin` (or set `ENVRELAY_BIN_DIR`) |
| `--bin-only` | Install the binary only, not the skill |
| `--no-modify-path` | Never edit a shell rc file |
| `--dry-run` | Say what would happen; download and change nothing |
| `--uninstall` | Remove what the installer put in place. Backups are never touched |

Run it again to update. It only ever replaces what it installed itself: a skill directory or link that something else put there (`npx skills`, your own copy) is left alone, and it tells you so. Every release asset, the `install.sh` that envrelay.com points to included, carries build provenance, which `gh attestation verify <file> --repo FutrixDev/envrelay-skill` checks.

Each download of `install.sh` from envrelay.com is recorded: the time, the IP address, the country and network that address belongs to, and the client's user agent.

### What it needs

- **macOS 11 or later, or Linux**, on x86_64 or arm64. The Linux binaries are static, so any distribution works. On Windows, use WSL.
- **python3 3.9+ and git**, for the skill's scripts. A Mac gets both from Apple's Command Line Tools: if they are missing, the installer opens Apple's installer and you click Install. On Linux, it tells you the package-manager command to run.
- **A coding agent that reads Agent Skills**: Claude Code, Codex, Cursor, Gemini CLI, GitHub Copilot, OpenCode and many more.

That is all. The package managers you already use (Homebrew, npm, cargo and so on) matter only when you want the agent to reinstall your software on the new machine, and `age` and `zstd` only for the [escape hatch](#the-escape-hatch).

### Other ways to install

These install the skill on its own. The first time you use it, the skill notices that `envrelay` is missing, tells you what it is, and with your OK runs the installer it carries (`install.sh --bin-only`: the same checksums, and the release that matches the skill's version).

Any agent, through [skills.sh](https://skills.sh):

```bash
npx skills add FutrixDev/envrelay-skill -g
```

Claude Code, as a plugin:

```
/plugin marketplace add FutrixDev/envrelay-skill
/plugin install envrelay@envrelay
```

GitHub Copilot CLI reads the same plugin marketplace:

```bash
copilot plugin marketplace add FutrixDev/envrelay-skill
copilot plugin install envrelay@envrelay
```

GitHub CLI 2.90 or later (`--agent` also takes `codex`, `cursor`, `gemini-cli`, `github-copilot` and others):

```bash
gh skill install FutrixDev/envrelay-skill envrelay --agent claude-code --scope user
```

[ClawHub](https://clawhub.ai), for OpenClaw. OpenClaw also reads `~/.agents/skills`, so the one-line installer above already covers it; to install from ClawHub instead:

```bash
openclaw skills install @futrixdev/envrelay --global
```

From source, with Rust 1.97 or later. `cargo install` puts `envrelay` in `~/.cargo/bin`, and the two links make the skill visible to Claude Code and to everything that reads `~/.agents/skills`:

```bash
git clone https://github.com/FutrixDev/envrelay-skill
cd envrelay-skill
cargo install --path . --locked
mkdir -p ~/.agents/skills ~/.claude/skills
ln -s "$PWD/skills/envrelay" ~/.agents/skills/envrelay
ln -s "$PWD/skills/envrelay" ~/.claude/skills/envrelay
```

---

## The four commands

```bash
envrelay encrypt <staging-dir> -o <backup.envrelay>
envrelay decrypt <backup.envrelay> -o <output-dir>
envrelay inspect <backup.envrelay>   # list the contents and print the manifest; writes nothing
envrelay verify  <backup.envrelay>   # read it end to end: right passphrase, every byte intact; writes nothing
```

There is no fifth command, no daemon, no config file and no state. `inspect` and `verify` are read-only: before a restore they answer "is this the file I want, and will it still open?" without unpacking anything to disk.

```bash
# Once the agent has gathered what to carry into ~/envrelay-staging-20260830/, you run:
envrelay encrypt ~/envrelay-staging-20260830 -o ~/backup.envrelay
# asks for the passphrase twice

# On the new machine:
envrelay decrypt ~/backup.envrelay -o ~/envrelay-restore
# asks for the passphrase once; then the agent takes over the restore
```

The rules are few, and all of them are hard:

- **The passphrase is read only from `/dev/tty`.** There is no `--passphrase` flag, and no environment variable or stdin is read, so the passphrase never shows up in shell history, in `ps` output, or in the agent's echo of a command. (`--passphrase-file` exists, but only for this project's automated tests, and its help text says so.)
- **`encrypt` never overwrites an existing file.** There is no `--force`.
- **`decrypt` writes only into a directory that does not exist or is empty**, and the files it unpacks keep the permission bits recorded in the archive.
- **Any error fails the whole command**: a half-written output is deleted and the exit code is non-zero. There is no "partly succeeded".

Delete the staging directory once the backup is encrypted: it is a plaintext copy of the same data.

---

## The escape hatch

An `.envrelay` file is an **age-encrypted (scrypt passphrase mode) `tar + zstd` stream**. That is a promise, not an implementation detail: if this binary vanished from the face of the earth, any machine with the standard tools could still open your backup.

```bash
age -d backup.envrelay | zstd -d | tar -xp
```

One test in this repository checks exactly that: it bypasses all of envrelay's code and reads a backup back with the reference libraries for the three formats, age, zstd and tar.

A backup is an **ordinary file**. Google Drive, a USB stick, another machine, all three at once: the encryption does not care where it lives, and that is the point.

---

## ⚠️ Lose the passphrase and the data is gone

No key escrow, no recovery flow, no back door, and nobody who can help you, us included. scrypt derives the key from your passphrase, and apart from that the file holds no second key.

Put the passphrase in your password manager. Do it now.

---

## What's in a backup

The agent assembles a tree like this in a staging directory, and then you encrypt it:

```
staging/
├── manifest.json          # what the backup holds; encrypted along with everything else
├── files/                 # ordinary files and directories, in their original relative layout
│   └── home/.zshrc …
├── credentials/           # key directories (~/.ssh, ~/.aws, ~/.config/gh …)
│   └── ssh/ …
└── repos/                 # git repositories carried whole (dirty ones, ones without a remote)
    └── myproject/ …
```

`manifest.json` is written by the agent and read by the agent; Core never parses it. The format is in [`skills/envrelay/references/manifest.md`](skills/envrelay/references/manifest.md).

The skill's judgement is spread over a few reference documents, all of them readable by people too:

| Document | What it covers |
|---|---|
| [`SKILL.md`](skills/envrelay/SKILL.md) | The 8 backup steps, the 7 restore steps, and the four hard rules |
| [`references/what-to-carry.md`](skills/envrelay/references/what-to-carry.md) | Where to look for what is worth carrying (dotfiles, tool config, work, credential directories, app data macOS guards), and how it goes into staging |
| [`references/exclude-list.md`](skills/envrelay/references/exclude-list.md) | Which directories not to back up (`node_modules/`, `target/`, caches of every kind), and the principle behind it: configuration travels, dependencies stay behind and are rebuilt |
| [`references/credential-locations.md`](skills/envrelay/references/credential-locations.md) | Where keys live, and the rules for restoring them (0600, never overwrite) |
| [`references/software-inventory.md`](skills/envrelay/references/software-inventory.md) | How to list each package manager's packages, and the commands that reinstall them |
| [`references/git-repos.md`](skills/envrelay/references/git-repos.md) | The five states a repository can be in, and the three ways to back one up |
| [`references/ai-agents.md`](skills/envrelay/references/ai-agents.md) | The five planes of an AI coding agent (Claude Code, Codex, Cursor and the rest): the agent itself, configuration, extensions, working state (sessions and memory), and credentials; which travel, which are reinstalled from a list, and which stay behind |
| [`references/manifest.md`](skills/envrelay/references/manifest.md) | The manifest.json v3 format and the vocabulary of restore statuses |

The judgement stays in the skill. What is purely mechanical has settled into six single-file Python scripts under [`skills/envrelay/scripts/`](skills/envrelay/scripts/): copying into staging, classifying repositories, listing software, taking stock of AI agents, the restore ledger, and comparing checksums. The scripts do the work but never decide; [ADR-022](docs/decisions/022-deterministic-mechanics-scripts.md) draws that line, and [ADR-023](docs/decisions/023-ai-agent-state.md) explains why AI agent state is a category of its own.

---

## Trust model

The agent is trusted. It runs in your own shell and already has full read and write access to your disk, so fencing it in with a sealed CLI would stop no real attacker. **The only thing that needs cryptographic protection is the backup file itself, which may end up in a third party's hands once it is written.** So there is exactly one boundary: the passphrase never enters the agent's context, and you run the encrypt and decrypt commands yourself.

That reasoning overturned this project's entire earlier architecture; [ADR-021](docs/decisions/021-mvp-v1-reset.md) explains why.

**Platforms:** macOS and Linux, which share the same permission-bit semantics. Windows, with its ACL mapping and the rest, waits for real demand.

---

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
sh tests/installer.sh
sh .github/scripts/check-versions.sh
```

`tests/installer.sh` runs the installer against a release packaged from this checkout, with a throwaway `HOME` per scenario, so it needs the release build first. `SH=dash sh tests/installer.sh` runs the installer under another shell; CI runs it under sh, dash, bash and zsh. `check-versions.sh` checks that the version is the same everywhere it is written.

## Releasing

1. Set the new version in `Cargo.toml`, in `metadata.version` in [`skills/envrelay/SKILL.md`](skills/envrelay/SKILL.md), in [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json) and in [`plugin.json`](plugin.json), and run `cargo build` so `Cargo.lock` follows. CI fails until the four agree.
2. Once that is merged, tag the merge commit on GitHub's main and push the tag right away (until the release exists, a skill installed from main asks for a release that is not there yet), for example:

   ```bash
   git fetch origin
   git tag v1.0.1 origin/main
   git push origin v1.0.1
   ```

   The [release workflow](.github/workflows/release.yml) builds envrelay for macOS (one universal binary) and Linux (static, x86_64 and arm64), installs the packaged release on each platform, and publishes it with build provenance. From then on envrelay.com's `install.sh` leads to it, and the [smoke workflow](.github/workflows/smoke.yml) installs from there with the one-line command, on macOS and Linux.
3. Once the release workflow's *publish* job is green, a maintainer sets the new version on the homepage and redeploys envrelay.com, whose site lives in a repository of its own ([ADR-025](docs/decisions/025-site-in-its-own-repository.md)): the homepage names the new version only from then on. `install.sh` needs no deploy, since it leads to the latest release.
4. Publish the skill to the registries: [`docs/publishing.md`](docs/publishing.md).

---

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
