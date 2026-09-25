# Publishing the skill

A tag push makes a release ([README, "Releasing"](../README.md#releasing)), and
envrelay.com's `install.sh` always leads to the latest one, which is all the
one-line installer needs. This page is the checklist for the rest: setting the
repository up once, and listing the skill where people look for agent skills.
envrelay.com, with its deploy and its download records, lives in a repository
of its own ([ADR-025](decisions/025-site-in-its-own-repository.md)).

- **English-language open skill communities only.** Chinese-language
  communities are out of scope, by the owner's decision.
- **Every step here is outward-facing**, and a maintainer does it with their own
  accounts. Nothing in CI does any of it.
- **Order matters.** Almost every registry reads the public GitHub repository or
  its latest release, so section 1 comes first.
- **Whichever channel a user comes from, they get the skill alone**, and the
  skill offers to install the `envrelay` binary the first time it is used
  ([ADR-024](decisions/024-one-command-install.md)). Only the one-liner brings
  both at once.

## 1. Once, before the first listing

1. **Describe the repository**, since directories copy the GitHub description
   and homepage. Then add topics: `agent-skills` is the one `gh skill` calls
   required for discoverability. Leave out `gemini-cli-extension` for now
   (section 5).

   ```bash
   gh repo edit FutrixDev/envrelay-skill --description "Move a development environment to a new machine: an Agent Skill plus a tiny CLI that encrypts the backup with your passphrase." --homepage https://envrelay.com/
   ```

   ```bash
   gh repo edit FutrixDev/envrelay-skill --add-topic agent-skills,claude-skills,claude-code,claude-code-plugin,codex,backup,dotfiles,developer-environment,migration
   ```

2. **Recommended hardening**, before the first tag. `gh skill publish
   --dry-run` warns about each of these when it is missing.

   Secret scanning, then push protection (it needs secret scanning first):

   ```bash
   gh repo edit FutrixDev/envrelay-skill --enable-secret-scanning
   ```

   ```bash
   gh repo edit FutrixDev/envrelay-skill --enable-secret-scanning-push-protection
   ```

   Immutable releases: once a release is published, its assets and its tag
   cannot change. The release workflow is compatible, because `gh release
   create` with assets drafts the release, uploads to the draft and only then
   publishes it. The price is that a broken release cannot be patched; the
   next patch version replaces it.

   ```bash
   gh api -X PUT repos/FutrixDev/envrelay-skill/immutable-releases
   ```

   A ruleset so that `v*` tags cannot be moved or deleted, even before their
   release exists. Creating them stays allowed.

   ```bash
   echo '{"name":"release tags","target":"tag","enforcement":"active","conditions":{"ref_name":{"include":["refs/tags/v*"],"exclude":[]}},"rules":[{"type":"update"},{"type":"deletion"}]}' | gh api -X POST repos/FutrixDev/envrelay-skill/rulesets --input -
   ```

3. **Check that envrelay.com leads here**, before the first tag. Its
   `/install.sh` must redirect to
   `https://github.com/FutrixDev/envrelay-skill/releases/latest/download/install.sh`:
   the site's repository sets that, and a deploy of it puts it live. A HEAD
   request shows the redirect and, unlike a download, is not recorded:

   ```bash
   curl -sI https://envrelay.com/install.sh
   ```

4. **Tag the first release.** Run the release workflow by hand on main first:
   without a tag it builds, packages and installs on all three platforms, and
   publishes nothing. With the ruleset from step 2, a `v*` tag cannot be moved
   or deleted, so a build that fails only after the tag is pushed costs a
   version number.

   ```bash
   gh workflow run release.yml --repo FutrixDev/envrelay-skill --ref main
   ```

   ```bash
   gh run watch --repo FutrixDev/envrelay-skill
   ```

   Once its three *verify* jobs are green, tag what is on GitHub's main, not a
   local branch:

   ```bash
   git fetch origin
   ```

   ```bash
   git tag v1.0.0 origin/main
   ```

   ```bash
   git push origin v1.0.0
   ```

   Watch the release workflow until *publish* is green. From then on,
   envrelay.com's `install.sh` leads to the new release, and the release's
   success starts `smoke.yml`: the one-liner, installing from the live site on
   macOS and Linux. Watch that run too, until *smoke* is green:

   ```bash
   gh run watch --repo FutrixDev/envrelay-skill
   ```

   *smoke* waits up to ten minutes for both hostnames to lead to the new
   release. If it gives up, check step 3 and re-run it (`gh run rerun <run-id>
   --failed --repo FutrixDev/envrelay-skill`); if it fails on plain http
   instead, Always Use HTTPS is off in Cloudflare. Then try the one-liner as a
   user would, without changing anything:

   ```bash
   curl -fsSL https://envrelay.com/install.sh | sh -s -- --dry-run
   ```

## 2. Channels with no review

### The repository's own plugin marketplace

`.claude-plugin/marketplace.json` makes the repository a plugin marketplace
with one plugin, which two manifests describe with the same fields:
`.claude-plugin/plugin.json` for Claude Code, and `plugin.json` at the root, in
the [Agent Plugins](https://agent-plugins.org/) format, for GitHub Copilot CLI,
VS Code and awesome-copilot
([ADR-026](decisions/026-frontmatter-and-manifests-for-awesome-copilot.md)). It
works as soon as the repository is public, and the README lists it.

- Claude Code: `/plugin marketplace add FutrixDev/envrelay-skill`, then
  `/plugin install envrelay@envrelay`.
- GitHub Copilot CLI reads the same marketplace: `copilot plugin marketplace add
  FutrixDev/envrelay-skill`, then `copilot plugin install envrelay@envrelay`.
- VS Code's agent plugins read it when the repository is added to the
  `chat.plugins.marketplaces` setting. Factory Droid falls back to
  `.claude-plugin/marketplace.json` as well (its commands were not checked).

Validate before every release, and release only when it reports no errors or
warnings:

```bash
claude plugin validate --strict .
```

Keep these two manifests and no others. awesome-copilot reads
`.github/plugin/plugin.json` and `.plugin/plugin.json` before the one at the
root, and Copilot CLI's documented lookup checks `.plugin/plugin.json` first as
well, so a manifest in either place (a registry's import tool may write one)
would silently take over. `check-versions.sh` fails CI unless the two agree
field for field, apart from `$schema`.

### gh skill (GitHub CLI 2.90 or later)

There is nothing to publish. `gh skill install FutrixDev/envrelay-skill envrelay`
reads the repository directly, and the `agent-skills` topic from section 1 is
what gets it found. To check the skill the way `gh` sees it, from a clean
checkout of main:

```bash
gh skill publish --dry-run
```

One warning is expected: no `license` field (on purpose, see ADR-024).

**Never run `gh skill publish` without `--dry-run`.** It would push the current
branch and create a GitHub release itself, with auto-generated notes and no
binaries, which collides with the release workflow.

### skills.sh

skills.sh lists a repository once its CLI has installed a skill from it with
telemetry on, which is the default. After the first release, one install into a
throwaway home directory does that and leaves this machine alone:

```bash
HOME="$(mktemp -d)" npx skills add FutrixDev/envrelay-skill --skill envrelay -g -a claude-code -y
```

The page is <https://skills.sh/FutrixDev/envrelay-skill>. Once it exists, the
README can carry the badge:

```markdown
[![skills.sh](https://skills.sh/b/FutrixDev/envrelay-skill)](https://skills.sh/FutrixDev/envrelay-skill)
```

### Tessl

Tessl's registry indexes public GitHub skills on its own. Users install from it
with `npx tessl i github:FutrixDev/envrelay-skill --skill envrelay`.

Claiming the listing (`tessl skill import ./skills/envrelay --workspace
<workspace>`, plus a GitHub Action with a Tessl API key as a secret) is
optional, and two things argue against doing it casually. Publishing a skill
publicly on Tessl cannot be undone. And the import generates a Tessl
`plugin.json`, while the root already has one: see where the generated one
lands before committing anything ("Keep these two manifests and no others",
above).

### Directories that crawl GitHub

claude-plugins.dev, SkillsMP and LobeHub's skill market index public
repositories on their own. There is nothing to submit. Look for the listing a
few days after the repository goes public; how long each takes was not
verified.

## 3. Reviewed submissions

### Anthropic's community plugin marketplace

Anthropic's reviewed catalog of Claude Code plugins that anyone can submit to.

1. Validate: `claude plugin validate --strict .`
2. Submit the repository at <https://clau.de/plugin-directory-submission>. An
   individual can use the Console form at
   <https://platform.claude.com/plugins/submit>.

Screening is automated. An approved plugin is pinned to a commit SHA, their CI
moves the pin as the repository changes, and the catalog syncs nightly. Pull
requests opened directly against the marketplace repository are closed
automatically.

Once it is listed, add it to "Other ways to install" in both READMEs:

```
claude plugin marketplace add anthropics/claude-plugins-community
claude plugin install envrelay@claude-community
```

### ClawHub (OpenClaw)

ClawHub publishes every skill under MIT-0, and asks that SKILL.md carry no
conflicting license terms, which is why SKILL.md has no `license` field. The
owner accepted that on 2026-09-24: the skill is public, and the copy on ClawHub
is MIT-0, while the repository stays MIT OR Apache-2.0.

To publish:

1. Install the CLI and sign in. It has to be version 0.23 or later: older
   ones have no `--categories`, which publishing needs (`clawhub -V` prints
   the version). The login is a device code confirmed in the browser. ClawHub
   accepts uploads only from GitHub accounts older than a minimum age; how old
   was not verified.

   ```bash
   npm i -g clawhub
   ```

   ```bash
   clawhub login
   ```

2. Create the publisher. If the `futrixdev` handle is taken, file an
   org/namespace claim with ClawHub instead of picking another name.

   ```bash
   clawhub publisher create futrixdev --display-name "FutrixDev"
   ```

3. Publish from a checkout of the release tag, so that the version on ClawHub
   and `metadata.version` agree, and dry-run first. The `--source-*` options
   link the listing to the tag's commit.
   [`.clawhubignore`](../skills/envrelay/.clawhubignore) leaves out Python
   bytecode, which ClawHub refuses. `git switch -` goes back to your branch
   afterwards.

   ```bash
   git checkout v1.0.0
   ```

   ```bash
   clawhub skill publish ./skills/envrelay --owner futrixdev --name EnvRelay --version 1.0.0 --changelog "First release." --categories operations,development --topics backup,restore,migration,dotfiles,developer-environment --source-repo FutrixDev/envrelay-skill --source-commit "$(git rev-parse HEAD)" --source-ref v1.0.0 --source-path skills/envrelay --dry-run
   ```

   ```bash
   clawhub skill publish ./skills/envrelay --owner futrixdev --name EnvRelay --version 1.0.0 --changelog "First release." --categories operations,development --topics backup,restore,migration,dotfiles,developer-environment --source-repo FutrixDev/envrelay-skill --source-commit "$(git rev-parse HEAD)" --source-ref v1.0.0 --source-path skills/envrelay
   ```

   For a later release, change the tag (in `git checkout` and `--source-ref`),
   `--version` and `--changelog`. The upload stays hidden while ClawHub
   reviews it: for v1.0.0, `clawhub inspect` gave the moderation reason as
   `pending.publication`; for v1.0.2, `inspect --version 1.0.2` answered
   "Version not found" and `latest` stayed at 1.0.1. v1.0.2 was audited and
   public within seven minutes of the upload.

4. Check the listing, then the version's security audit. They are separate
   verdicts: moderation decides whether the listing is public, and it can be
   public (`clean`) while the audit on its page says Review, which asks users
   to read the findings before they install.

   ```bash
   clawhub inspect @futrixdev/envrelay
   ```

   ```bash
   clawhub inspect @futrixdev/envrelay --version 1.0.0 --json
   ```

   The audit is `version.security`. v1.0.0 and v1.0.1 read `suspicious`,
   shown as Review, for the download override that ADR-027 removed; v1.0.2
   reads `clean`, shown as Pass. The latest version's audit, with any
   findings, is on
   <https://clawhub.ai/futrixdev/skills/envrelay/security-audit>.

   ```bash
   openclaw skills verify @futrixdev/envrelay
   ```

Every upload is scanned (VirusTotal, ClawScan and static analysis). A scanner
is most likely to stop at the installer, which downloads a binary: the
frontmatter's top-level `clawdis` block declares what the skill needs (ADR-026)
and SKILL.md says what the installer does, and the review compares those with
the code. If a listing is held, read the report:

```bash
clawhub scan download envrelay --version 1.0.0
```

Publishing can be automated later with ClawHub's reusable workflow,
`openclaw/clawhub/.github/workflows/skill-publish.yml@v1` (inputs `skill_path`,
`owner` and `dry_run`, and a `clawhub_token` secret). It is not set up.

### MCP Market

Submit `https://github.com/FutrixDev/envrelay-skill` at <https://mcpmarket.com/submit>.
It is reviewed before it is listed.

### Smithery

Smithery registers a skill by its git URL. Sign in at <https://smithery.ai>,
create a namespace and an API key, then register the skill. Do it early: a git
URL can be claimed by only one skill.

```bash
curl -sS -w '\nHTTP %{http_code}\n' -X PUT https://api.smithery.ai/skills/NAMESPACE/envrelay -H "Authorization: Bearer $SMITHERY_API_KEY" -H "Content-Type: application/json" -d '{"gitUrl":"https://github.com/FutrixDev/envrelay-skill/tree/main/skills/envrelay"}'
```

Replace `NAMESPACE`, and put the API key in `SMITHERY_API_KEY` first. A 409
means another skill already claimed the URL, a 422
that SKILL.md failed validation, and a 404 that the namespace or the SKILL.md
was not found. That a `tree/main/skills/envrelay` URL is accepted for a skill
in a subdirectory was not verified; on a 404, check that first. Users then
install with `smithery skill add NAMESPACE/envrelay --agent claude-code`.

### awesome-copilot

A listing in [github/awesome-copilot](https://github.com/github/awesome-copilot)
puts the plugin in the marketplace that Copilot CLI and VS Code ship with.
Submit v1.0.1 or later: v1.0.0 has no manifest where they look, and its
frontmatter fails their lint (ADR-026).

- For a plugin that lives in its own repository, use their external-plugin
  issue form; their CONTRIBUTING.md links it. Do not open a pull request that
  copies the skill into their repository: it would be relicensed MIT there.
- The form asks for the repository, a release tag with its full 40-character
  commit SHA, a semver version, the license, the author and keywords. The
  version must be the one in the tag's root `plugin.json`, which is where they
  find the manifest. The SHA:

  ```bash
  git rev-list -n 1 v1.0.1
  ```

- Their checks run `vally lint` over the whole repository (the manifest names
  no skills directory), install the plugin with Copilot CLI from a marketplace
  made on the spot, and check the manifest against the Agent Plugins
  specification. Then a maintainer approves it. Listings are reviewed again
  every six months. How a listed entry moves to a newer tag was not verified;
  see their CONTRIBUTING.md.
- `vally lint` refuses a SKILL.md `metadata` value that is not a string, which
  is why ClawHub's declarations sit in a top-level `clawdis` block. Keep them
  there.

## 4. Awesome lists

For every list:

- One neutral line. No emojis and no superlatives.
- Say what it needs: a coding agent with a terminal (Claude Code, Codex and the
  like), not Claude.ai or the API.
- Say that it asks before anything destructive, and that the passphrase never
  passes through the agent.
- Where a list refuses AI-written submissions, the maintainer writes the entry
  personally.

Right after the first release:

| List | How | Notes |
|---|---|---|
| [ComposioHQ/awesome-claude-skills](https://github.com/ComposioHQ/awesome-claude-skills) | Pull request titled `Add EnvRelay skill`, adding one line in alphabetical order: `- [EnvRelay](https://github.com/FutrixDev/envrelay-skill) - One sentence.` | Keep the alphabetical order |
| [hesreallyhim/awesome-claude-code](https://github.com/hesreallyhim/awesome-claude-code) | The "recommend a resource" issue form on the web. Pull requests and issues opened with `gh` are closed | Written by a person. Eligible 14 days after the repository's first commit (2026-09-25, so from 2026-10-09) with ongoing activity, or at 100 stars. GitHub must detect the license |
| [github/awesome-copilot](https://github.com/github/awesome-copilot) | The external-plugin form, above | Needs a release tag, v1.0.1 or later |

Once people use it:

| List | How | Notes |
|---|---|---|
| [travisvn/awesome-claude-skills](https://github.com/travisvn/awesome-claude-skills) | Pull request with a list line and a table row, stating prerequisites and license | Needs 10 stars, or the pull request is closed automatically. No AI-assisted pull requests: write it yourself |
| [VoltAgent/awesome-agent-skills](https://github.com/VoltAgent/awesome-agent-skills) | Pull request titled `Add skill: FutrixDev/envrelay-skill`, with `- **[FutrixDev/envrelay-skill](https://github.com/FutrixDev/envrelay-skill)** - ` and at most ten words | Takes no brand-new skills |
| [composio-community/awesome-codex-skills](https://github.com/composio-community/awesome-codex-skills) | Pull request | The entry needs an install line that uses Codex's skill installer |
| [BehiSecc/awesome-claude-skills](https://github.com/BehiSecc/awesome-claude-skills) | Pull request, per its README | |
| [Prat011/awesome-llm-skills](https://github.com/Prat011/awesome-llm-skills) | Pull request, per its template | Wants a real use case, tested across platforms |
| [VoltAgent/awesome-openclaw-skills](https://github.com/VoltAgent/awesome-openclaw-skills) | Pull request linking the ClawHub listing | Only after the ClawHub listing exists |
| [heilcheng/awesome-agent-skills](https://github.com/heilcheng/awesome-agent-skills) | Pull request adding `[FutrixDev/envrelay-skill](https://github.com/FutrixDev/envrelay-skill) - One sentence.` to the matching section of its README, after `npm run build` in its `website/` directory, per its CONTRIBUTING.md | Needs SKILL.md under 500 lines. Its last commit was on 2026-04-05, so a review may take a while |

Not these:

| List | Why |
|---|---|
| sickn33/antigravity-awesome-skills | Copies skills into its own repository, behind security allowlists |
| composio-community/awesome-claude-plugins | Copies the plugin folder into its own repository |
| skillmatic-ai/awesome-agent-skills | Lists resources, not skills |
| webfuse-com/awesome-claude | General Claude links; no process for skills found |
| rohitg00/awesome-claude-code-toolkit | Its rules could not be found; check them before submitting |
| quemsah/awesome-claude-plugins | Built from automated metrics; nothing to submit |

## 5. Later

- **Gemini CLI's extension gallery.** It needs a `gemini-extension.json` at the
  repository root (a fifth version to keep in step, which `check-versions.sh`
  would have to learn) and the `gemini-cli-extension` topic, and how the
  gallery treats release assets that do not follow its
  `{platform}.{arch}.{name}.{ext}` naming was not verified. Gemini CLI already
  reads `~/.agents/skills`, so the one-liner covers it meanwhile.
- **Cursor's marketplace.** It needs `.cursor-plugin/plugin.json`, a logo, a
  publisher application, and a manual review of every update. Cursor also
  reads `~/.agents/skills` meanwhile.

## 6. Not pursued

| Where | Why |
|---|---|
| OpenAI's plugin directory | Needs an OpenAI organisation, identity verification, and a partner contact before anything that runs locally is accepted |
| openai/skills | Deprecated |
| Kiro powers | Curated partners only |
| Roo Code's marketplace | MCP servers and modes, not skills |
| anthropics/claude-plugins-official | Curated by Anthropic; the community marketplace is the open one |
| The Claude.ai directory | Partners only, and the skill needs a local terminal anyway |
| playbooks.com | No longer online |
| Context7's skills | The page redirects; no submission path found |
| agentskills.io | The specification, not a registry |
| Chinese-language communities | Out of scope, by the owner's decision |

## 7. Every release

1. Bump the version in its four places (README, "Releasing"), check the
   plugin with `claude plugin validate --strict .`, and merge.
2. **Tag right after the merge.** Until the release is out, a skill installed
   from the default branch asks for a release that does not exist yet, and its
   installer stops rather than fetch a different version.
3. **Redeploy envrelay.com** once *publish* is green, from the site's
   repository, with the homepage's version set to the new one (ADR-025). The
   homepage names the new version only from then on; `/install.sh` needs no
   deploy, since it leads to the latest release.
4. **ClawHub**: publish the new version from its tag (section 3).
5. **awesome-copilot**: the entry is pinned to a tag; move it when a release
   matters.
6. **Tessl**: only if the listing was claimed, its Action.
7. Nothing to do for the one-liner (envrelay.com's `install.sh` leads to the
   latest release, and *smoke* installs from it), `gh skill`, skills.sh, the
   repository's own marketplace, Anthropic's community marketplace (their CI
   moves the pin), or the directories that crawl GitHub.
