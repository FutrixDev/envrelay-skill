# ADR-026: Frontmatter and manifests that awesome-copilot accepts

Date: 2026-09-25
Status: accepted
Amends: ADR-024 (one command installs the binary and the skill), ADR-025 (the
site moves to a repository of its own)

## Context

github/awesome-copilot lists a plugin that lives in its own repository once an
issue form names a release tag, its commit and a version. Their intake
(`eng/external-plugin-quality-gates.mjs`, which pins `@microsoft/vally` 0.12.0
and Ajv 8.20.0) checks out that commit, then:

- reads the plugin's manifest with `git show` from `.github/plugin/plugin.json`,
  `.plugin/plugin.json` or `plugin.json` at the root, the first that exists,
  and stops if there is none;
- requires the manifest's `version` to be the one submitted;
- runs `vally lint` over the skills directories the manifest names, or over the
  whole repository when it names none;
- installs the plugin with Copilot CLI from a marketplace it makes on the spot,
  and checks that the manifest arrived;
- checks the manifest against the Agent Plugins 1.0 specification, whose top
  level allows `$schema`, `name`, `version`, `description`, `author`,
  `homepage`, `repository`, `license`, `keywords` and `extensions`.

v1.0.0 fails at the first step: its one manifest is
`.claude-plugin/plugin.json`, which Claude Code reads and the intake does not.
It would fail the lint too. vally refuses a SKILL.md `metadata` value that is
not a string ("Metadata values must be strings. Non-string values found for
key(s): openclaw"), and ADR-024 put the declarations for OpenClaw and ClawHub
in a nested `metadata.openclaw`.

Each reader of those declarations looks in a place of its own:

- ClawHub (`convex/lib/skills/index.ts`) takes `metadata.clawdbot`,
  `metadata.clawdis` or `metadata.openclaw` if it is an object, and otherwise a
  top-level `clawdis` block.
- OpenClaw (`packages/markdown-core/src/frontmatter.ts` and
  `src/shared/frontmatter.ts`) turns `metadata` into JSON text, parses it back
  and takes an `openclaw` or `clawdbot` object from it. It reads no other
  field for these.
- vally, `gh skill` and `claude plugin validate --strict` accept extra
  top-level fields. `skills-ref`, the Agent Skills reference validator,
  refuses them.

No one frontmatter satisfies all of these.

## Decision

- **The declarations move to a top-level `clawdis` block**, unchanged: the
  required binaries, the operating systems, the two optional environment
  variables and the homepage. `metadata` keeps only `version`.
- **`plugin.json` at the root is the plugin's Agent Plugins manifest**: the
  Claude Code one plus the `$schema` that opts into Agent Plugins 1.0. Copilot
  CLI and VS Code read it with that format's semantics, and it is where
  awesome-copilot finds it. Claude Code reads only
  `.claude-plugin/plugin.json`, where it ignores `$schema`, so that file
  stays.
- **`check-versions.sh` compares the two manifests** field for field, apart
  from `$schema`. The version is now written in four places.
- **v1.0.1 is the release to submit.**

## Consequences

- vally passes over the whole repository, and the manifest has no field the
  specification check warns about.
- ClawHub reads the same values from v1.0.1 as from v1.0.0.
- **OpenClaw no longer sees the declarations.** It used them to decide whether
  the skill can load; now it loads the skill on Windows too, or where python3
  or git is missing, and knows no homepage for it. The `compatibility` field
  still names macOS or Linux, python3 and git, and the skill's first step,
  "Before anything: the tools", checks for python3 and git before a backup or
  a restore starts.
- `skills-ref validate` fails on `clawdis`. No channel we submit to is known to
  run it.
- Another manifest in `.github/plugin/` or `.plugin/` would take over from the
  root one at awesome-copilot's intake, and Copilot CLI's documented lookup
  checks `.plugin/plugin.json` first as well (`docs/publishing.md`, section 2).

## Alternatives rejected

- **Keep `metadata.openclaw` and leave out awesome-copilot.** OpenClaw would
  keep hiding the skill where it cannot run, but the owner chose the listing:
  awesome-copilot is the marketplace that Copilot CLI and VS Code ship with.
- **Declare nothing.** ClawHub's review would see an installer that downloads
  a binary, and nothing declared to compare it with.
- **`metadata.openclaw` as a JSON string.** vally would accept it, but ClawHub
  and OpenClaw both take only an object there.
- **A symlink from the root to `.claude-plugin/plugin.json`.** `git show`
  returns the link's target path, not the manifest.
- **The manifest in `.github/plugin/` or `.plugin/`.** Those are the older
  locations; Agent Plugins 1.0, which Copilot CLI, VS Code and Cursor load,
  puts the manifest at the plugin's root.
- **The root manifest alone.** Claude Code reads only
  `.claude-plugin/plugin.json`.
