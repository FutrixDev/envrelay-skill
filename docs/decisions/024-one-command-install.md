# ADR-024: One command installs the binary and the skill

Date: 2026-09-24
Status: accepted
Extends: ADR-022 (deterministic mechanics scripts)
Amended by: ADR-025 (the site moves to a repository of its own)

## Context

Until now, using EnvRelay meant building it: clone the repository, have Rust
1.97, `cargo install --path .`, then link `skills/envrelay` by hand into each
agent's skills directory. On the machine this project is developed on, even
that failed (rustup there is a keg-only Homebrew install with no
`~/.cargo/bin`). The restore side was worse: the repository was private and had
no releases, so a new machine needed a GitHub login and a Rust toolchain before
it could decrypt anything, or `age` and `zstd` and the escape hatch. And
nothing in SKILL.md checked whether `envrelay` was installed at all; an old
binary from before ADR-021 answered with a JSON contract that no longer exists.

The goal is that one command makes the skill usable. Listing what the skill
actually depends on, piece by piece:

- **The `envrelay` binary** is the only thing that has to be installed. It is
  the passphrase layer (ADR-021), and nothing else can stand in for it.
- **python3 3.9+ and git**, for the scripts (ADR-022). An existing development
  machine has both. A new Mac gets both from Apple's Command Line Tools, which
  only Apple's own dialog installs, with one click from the user. A new Linux
  machine gets them from its package manager, which needs sudo.
- **Package managers** (Homebrew, npm, cargo…) matter only when the user wants
  their software reinstalled, and the restore procedure deals with them in its
  own step. **`age` and `zstd`** matter only for the escape hatch.
- **curl or wget, tar, and sha256sum or shasum** ship with every supported OS.
- **Rust, a GitHub account and Node** were never needs of the product, only
  costs of how it was distributed.
- **The coding agent itself** is the user's, and is there before EnvRelay is.

So one command can do everything except three things: the Command Line Tools
click (Apple's), a sudo package install on Linux (the user's password), and
the passphrase (the user's, by design).

## Decision

**Releases.** Pushing a tag `vX.Y.Z` runs `.github/workflows/release.yml`:

1. *preflight*: the repository is public, and the versions agree with the tag;
2. *build*: one universal macOS binary (deployment target 11.0, the oldest
   macOS on Apple silicon) and static musl binaries for Linux on x86_64 and
   aarch64, each built on its own architecture;
3. *package*: `.github/scripts/package-release.sh` makes a tarball per
   platform, `envrelay-skill.tar.gz` from the files git tracks under
   `skills/envrelay`, a copy of `install.sh`, and `SHA256SUMS` over all of them;
4. *verify*: the packaged release is installed into an empty `HOME` on each of
   the three platforms, before anything is published;
5. *publish*: build provenance for every asset, then `gh release create
   --verify-tag`.

Run by hand without a tag, the workflow stops after *verify*. That is how a
release is tried before its tag exists.

The repository has to be public: the installer downloads without signing in,
and the arm64 Linux runners and the attestations need a public repository too.

**The installer, `skills/envrelay/install.sh`.** POSIX sh, so it runs wherever
`curl … | sh` does.

- **No sudo, ever.** It refuses to run under sudo and installs into the home
  directory: the binary into `~/.local/bin` (or `--bin-dir`,
  `ENVRELAY_BIN_DIR`), the skill into `~/.agents/skills/envrelay`.
- **Everything is downloaded and checked before anything changes.** Each file
  must match `SHA256SUMS`; curl is held to HTTPS and TLS 1.2. The new binary is
  run once from where it will live, since `/tmp` is noexec on some systems.
- **One real copy of the skill, links for the rest.** `~/.agents/skills` is
  read by Codex, Cursor, Gemini CLI, GitHub Copilot, OpenCode, OpenClaw and
  most other agents. Claude Code gets a symlink in
  `${CLAUDE_CONFIG_DIR:-~/.claude}/skills`, and Kiro and Cline get one when
  their directories exist.
- **It only ever replaces its own work.** A marker file, `.envrelay-installer`,
  records that the skill directory is the installer's. A directory or link that
  anything else made (`npx skills`, a copy, the user's own link) is left alone
  and reported. The new skill is staged beside `skills/`, so no agent sees a
  half-made copy, and an interrupted update puts the previous one back.
  `--uninstall` removes only what the installer made; it never touches a backup
  and keeps the PATH line, which other programs may rely on.
- **PATH.** If `~/.local/bin` is not on `PATH`, one commented line is appended
  to the rc file of the user's shell (zsh, bash — the login-shell file on macOS
  — fish, else `~/.profile`); `--no-modify-path` opts out. An `envrelay`
  earlier on `PATH` is reported, since that is how a stale binary hides.
- **python3 and git are checked, not installed.** On a Mac without the Command
  Line Tools it opens Apple's installer (`xcode-select --install`), which the
  user clicks through; on Linux it prints the distribution's install command
  for the user to run.
- `--dry-run` downloads and changes nothing, and says what would happen.
  `ENVRELAY_DOWNLOAD_URL` swaps the GitHub release for a mirror or a local
  directory, which is how CI and `tests/installer.sh` install a release that
  has not been published.

**The installer is the one exception to ADR-022's "a script never installs".**
It lives at the skill's root, not in `scripts/`, and SKILL.md says it is not
one of the scripts. The skill runs it only through the new "Before anything"
step: it shows the user the `--dry-run` plan, says what `envrelay` is, and runs
`install.sh --bin-only` only on a yes. Installing the passphrase layer is the
user's decision, like every other install the skill proposes.

**One version, written in three places.** `Cargo.toml` (what `envrelay
--version` prints), `metadata.version` in SKILL.md (which release the skill's
installer fetches) and `.claude-plugin/plugin.json` (what Claude Code compares
to offer an update). `.github/scripts/check-versions.sh` fails CI and the
release preflight unless they agree. It asks the installer which release it
would fetch rather than reading SKILL.md a second way, so the check and the
user's path share one parser. The homepage's badge and demo name the version
too, from a meta tag that the site's repository sets after each release
(ADR-025).

That is what makes a skill installed from anywhere safe. Run from inside an
installed skill, `install.sh` fetches the release matching the `SKILL.md`
beside it, so a registry copy that lags behind still gets the binary it was
written for. The reader copes with `gh skill install`, which re-serialises the
frontmatter (sorted keys, four-space indent, quotes dropped, `github-*` keys
added) and writes every file without its execute bit, which is why SKILL.md
says `sh …/install.sh`. If no version can be read, the installer falls back to
the latest release.

**Frontmatter.**

- `allowed-tools: Bash(python3 ${CLAUDE_SKILL_DIR}/scripts/*)` pre-approves the
  scripts and nothing else. The installer and `envrelay` stay behind a prompt.
- `compatibility` names what the skill needs to run.
- `metadata.openclaw` declares the required binaries (python3, git), the
  operating systems (darwin, linux), the two optional environment variables
  the installer reads, and the homepage. OpenClaw decides from these whether
  the skill can load, and ClawHub's review compares them with what the code
  does. The Agent Skills spec expects `metadata` to map strings to strings;
  OpenClaw documents this nested shape, and no validator we have found
  rejects it.
- There is no `license` field. ClawHub releases every skill it publishes under
  MIT-0 and asks for no conflicting license terms in SKILL.md, so the field
  would be wrong there. The repository's MIT OR Apache-2.0 covers the source,
  the binary and the skill as it lives here; the owner accepted MIT-0 for the
  copy on ClawHub.

**envrelay.com.** The one-liner is `curl -fsSL https://envrelay.com/install.sh
| sh`: the project's own domain, not a GitHub URL. Since ADR-025 the site, its
Worker and its deploy live in a private repository of their own; every path
below but `smoke.yml` is that repository's. A Cloudflare Worker serves the
site, on the account that already holds the domain's DNS, and
`wrangler.jsonc` makes envrelay.com and www.envrelay.com its custom domains,
whose DNS records and certificates the first deploy creates. Cloudflare serves
the site's files itself, free and without limit; the Worker's code,
`worker/index.js`, runs only for a request that no file answers, such as
`/install.sh`:

- The homepage. It explains the skill in ten languages, and its animated
  terminal plays a backup and a restore the way they really happen: the user
  asks the agent in their own language, the agent answers in it, and the
  `envrelay` commands and their output stay as they are. Nothing on it loads
  from a third party. Its badge and the demo's installer output name the
  version in the page's `envrelay-version` meta.
- Each language is a page of its own, so that search engines index all ten:
  English at `/`, the others at `/ja/`, `/zh-hans/` and so on. Before every
  deploy, `.github/scripts/build-site.py` writes them from `site/index.html`
  and `site/i18n.json`. Each page is whole in its language without scripts,
  with its own title, description and canonical link, links to every other
  language's page (`hreflang`), and structured data that names the software
  and its version. A sitemap lists them all, and a shared link shows the same
  picture in every language (`site/og.png`, rendered from
  `.github/scripts/og-image.html`). A visitor's first arrival at `/` goes on
  to the page in the language they last chose, or else in the first of their
  browser's languages the site has. Coming back to `/`, reloading it, or
  following a link to it from another of the site's pages with no choice
  remembered stays on English, and the language switcher goes to the chosen
  page and remembers it.
- `/install.sh`, which the Worker answers with a 302 to
  `https://github.com/FutrixDev/envrelay-skill/releases/latest/download/install.sh`.
  The installer, the binaries, the skill and their checksums all stay on
  GitHub's release, and the one-liner runs the copy that the release's own
  *verify* jobs installed with. The Worker records each GET of it in a D1
  database (`worker/migrations`): when, from which IP address, the country and
  network Cloudflare places that address in, and the client's user agent, as
  the README says. A failed write is logged, without the address, and the
  download goes ahead: the record is a side effect, never a condition.

A maintainer deploys it by hand, from main, with `npm run deploy`
(`.github/scripts/deploy-site.sh`): it stops unless the checkout is main as
GitHub has it, with nothing changed, applies the database's migrations and runs
`wrangler deploy`, which builds the site first. The only credential is
wrangler's sign-in on the maintainer's machine; nothing about the deploy is
stored in GitHub. A release needs no deploy for the one-liner, since the
redirect already leads to it, but the homepage shows the new version only once
it is deployed. `package.json` and `package-lock.json` pin wrangler and every
package it depends on, each with its checksum, and `.npmrc` stops npm from
running their install scripts.

In the site's repository, a pull request gets the Worker's tests (`npm test`)
and a dry run of the deploy, with the same wrangler; the dry run builds the
site, so it fails wherever the build does. In this repository, after every
release run that published one (a `workflow_run` of the release workflow),
`.github/workflows/smoke.yml` waits until both hostnames lead to the new
`install.sh`, checks that plain http gets nothing but a redirect, and installs
with the one-liner into an empty `HOME` on macOS and Linux. That is the path a
user takes, end to end. Its requests carry `envrelay-smoke` as their user
agent, which the site's download counts leave out.

**Plain http serves nothing.** Cloudflare answers port 80 on every proxied
hostname, and a zone cannot close it, only decide what comes back. Unless the
zone says otherwise, the Worker answers there too, and would serve the site,
and the redirect to the installer, in plaintext. The zone's Always Use HTTPS
setting answers every http request with a redirect to the same URL on https,
and *smoke* fails on anything else.

**Distribution.** The one-liner is the primary way to install, and the only one
that brings the binary too. Every other channel installs the skill alone, and
the skill installs the binary on first use. The channels are English-language
open skill communities, at the owner's direction: skills.sh, `gh skill`,
Anthropic's community plugin marketplace, ClawHub, Tessl, MCP Market,
Smithery, GitHub's awesome-copilot, the directories that crawl GitHub, and the
awesome lists. `docs/publishing.md` is the checklist, with what each one asks
for and why some are left out. The repository is also a plugin marketplace of
its own (`.claude-plugin/`), which Claude Code, GitHub Copilot CLI and VS Code
read.

`gh skill publish` is used only as `--dry-run`, plus the `agent-skills` repo
topic it would otherwise add. Without `--dry-run` it creates a GitHub release
itself, from the current branch and without binaries, which would collide
with the release workflow.

## Consequences

- **The repository must be public**; the release preflight fails otherwise.
- **Releasing is a tag push.** Nothing is uploaded by hand, and every release
  has been installed on all three platforms before it is published, then from
  envrelay.com on macOS and Linux after. Publishing the skill to registries is
  a separate step, which `docs/publishing.md` lists; a registry that re-syncs
  from GitHub needs no step at all.
- **CI tests the installer on every change**: `tests/installer.sh` under sh,
  dash, bash and zsh against a release packaged from the checkout, plus
  shellcheck and the version check. In the site's repository, the site's build
  does the same for the homepage's languages: every string in every one, and
  no translation that drops a placeholder, a `<code>` or a tag.
- **The homepage can go live before the first release; the one-liner
  cannot.** Until this repository has a release, `/install.sh` leads to a 404,
  so `curl -f` fails and sh runs nothing.
- **What is live is what was last deployed.** A merge to the site's main
  changes nothing on the site until a maintainer deploys, and the deploy takes
  only main as GitHub has it, so the site is always some commit of main, not
  necessarily the latest. A new version reaches the homepage with the deploy
  that follows its release (README, "Releasing"); until then the badge and the
  demo name the version before it.
- **The installer and the assets it downloads each come from the latest
  release at the moment they are fetched.** They differ only for an install
  that straddles a publication, so asset names and layout are a contract with
  the previous installer, not just the current one.
- **envrelay.com depends on Cloudflare as well as GitHub.** The Worker, the
  database, the DNS records and the certificates are Cloudflare's. Whoever
  can sign in to the account, or holds a maintainer's wrangler sign-in, can
  change where the one-liner leads. Always Use HTTPS is a setting in
  Cloudflare's dashboard that no review sees; *smoke* notices if it is off.
  The site's repository has the setup.
- **Only requests that no file answers run the Worker**: `/install.sh`, and
  any path the site does not have. On the Workers free plan they count
  against its daily request limit, past which they fail until the next day;
  the pages do not.
- **Every download of `install.sh` records an IP address**, which is personal
  data under the GDPR and laws like it. The homepage, in every language, and
  the README say so. How long the records are kept is not decided yet, and
  nothing deletes them.
- **The macOS binary is not signed or notarised.** Files that curl downloads
  carry no quarantine attribute, so Gatekeeper does not stop them; a copy
  downloaded through a browser would be stopped. Signing waits until someone
  needs that path.
- **On Linux, a missing python3 or git is the user's to install.** The
  installer never asks for a password, so it names the command instead.
- **Releases can be made immutable.** With assets to upload, `gh release
  create` makes a draft, uploads to it and only then publishes, and GitHub
  enforces immutability from publication on. A published release then cannot
  be patched: a broken one is followed by the next patch version.
- **Not verified when this was written**: how long ClawHub's minimum account
  age is, and how long each crawling directory takes to pick the skill up.

## Alternatives rejected

- **Ship the binary in a Claude Code plugin, or in the skill folder.** A
  plugin's files land in the plugin cache, not on the `PATH` of the terminal
  where the user types the passphrase, and only Claude Code would get them.
  Binaries for three platforms in the skill folder would ride along to every
  registry and every agent's skills directory, where no one can read them the
  way they can read the scripts.
- **Download the binary from a SessionStart hook.** Code that fetches and
  installs software without asking is exactly what rule 4 tells the skill to
  warn the user about.
- **Fold the scripts into the binary.** ADR-022: the binary grows only for
  things that need the passphrase.
- **An npm package.** It would make Node a requirement of a product that needs
  none.
- **Homebrew.** A fine extra channel later, but it is not one command on a new
  Mac without Homebrew, or on Linux.
- **Install to `/usr/local/bin` with sudo.** It needs a password the agent
  cannot and must not type, and a directory in the user's home needs none.
- **Point `/install.sh` at main's copy instead of the release's.** The copy on
  main may already expect assets the latest release does not have. The
  release's copy is the one its own *verify* jobs installed with.
- **Copy the release's `install.sh` onto the site from a GitHub Actions
  workflow**, as this decision did before, first to GitHub Pages and then to
  the Worker. It took a redeploy after every release, until which the site
  still served the previous installer, and the homepage could not go live
  before the first release; deploying the Worker from GitHub also took a
  Cloudflare token stored there, in an environment only main could deploy to.
- **A redirect without code**, from a `_redirects` file among the site's
  files or a redirect rule in Cloudflare's dashboard. Neither counts against
  the Worker's request limit, but neither can record the download, and the
  dashboard's rule is configuration no review sees.
- **Cloudflare's Workers Builds**, which deploys every push to main. It keeps
  the token out of GitHub, but it needs Cloudflare's GitHub app on the
  repository, and a token that Cloudflare makes, by default able to edit every
  Worker, KV namespace and R2 bucket on the account; its branch, preview
  builds and token are settings in Cloudflare's dashboard that no review
  sees. The owner decides when the site changes, and a deploy by hand takes no
  app, no stored token and no setting in the dashboard.
- **One page that shows each language in place**, as the homepage did before.
  A search engine indexes a page in the language it finds it in, so nine of
  the ten could not be indexed, and a shared link previewed in English
  whatever the language of the person sharing it.
- **Turn wrangler's usage telemetry off with `send_metrics` in
  `wrangler.jsonc`.** Deploy ignores it for the events it sends while it
  looks for a framework to configure. The dry run, on GitHub's machines, turns
  telemetry off with `WRANGLER_SEND_METRICS`, and the site's README has the
  command that turns it off on a maintainer's.
- **Host the binaries on envrelay.com too.** It would put megabytes of binaries
  on the site for no gain: the release already has them, with provenance, and
  the checksums the installer trusts have to come from the release either way.
- **GitHub Pages**, which the first version of this decision used. It needs no
  stored secret and no action but GitHub's own. Everything else about it was
  more work: a TXT record to verify the domain for the organization, nine
  DNS-only records pointing at GitHub, a certificate issued on GitHub's
  schedule and only then enforced, and a ten-minute cache after every deploy.
  The domain's DNS is at Cloudflare anyway.
- **Refusing plain http outright**, with a WAF rule that answers 403. Neither
  it nor the redirect serves anything over http, and neither protects someone
  who types `http://`: an attacker who can rewrite that request can answer it
  in Cloudflare's place either way. Refusing would only break the link.
