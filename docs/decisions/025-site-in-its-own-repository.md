# ADR-025: The site moves to a repository of its own

Date: 2026-09-25
Status: accepted
Amends: ADR-024 (one command installs the binary and the skill)

## Context

Until now one repository, FutrixDev/envrelay, held everything: the CLI, the
skill, the release workflow, and envrelay.com, whose pages, Cloudflare Worker
and deploy script ADR-024 describes. The Worker answers `/install.sh` with a
redirect to the latest release, and records each download in a D1 database.

ADR-024 needed that repository public before its first release. Once it was,
the owner asked whether anyone could now read the download records, since the
Worker's code and the database's schema were public. No one could: the
database is reached only through the Worker's binding or a maintainer's
wrangler sign-in, and the repository held no account ID, database ID or token.
The owner still wanted the two kept apart. The site is the owner's own
operation: its Cloudflare account, its deploys and what it records. The skill
and the CLI are what users install and what registries read.

No release had been published from that repository yet, so nothing a user had
installed pointed at it.

## Decision

- **FutrixDev/envrelay becomes private and keeps the site**: its pages, the
  Worker, the deploy and the Worker's tests. It deploys as before, by hand.
- **FutrixDev/envrelay-skill, public, holds everything else**: the CLI, the
  skill and its installer, the release workflow, the plugin marketplace files,
  the tests and the docs, every ADR included. It starts from a fresh history,
  a single commit with the tree as it was when the site moved out.
- **envrelay.com/install.sh leads to this repository's latest release.** The
  one-line command does not change.
- **The smoke test moves here**, as `.github/workflows/smoke.yml`, since it
  follows this repository's release workflow. It needs nothing from the site's
  repository: it compares what envrelay.com serves with the release's
  `install.sh`, byte for byte, then installs through the one-liner.
- **The version is written in three places here**: `Cargo.toml`, SKILL.md's
  `metadata.version` and `.claude-plugin/plugin.json`, which
  `check-versions.sh` compares. The homepage keeps its `envrelay-version` meta,
  which a maintainer sets in the site's repository once a release is out, and
  deploys.
- **Names stay.** The skill, the plugin and its marketplace are still called
  `envrelay`, so `envrelay@envrelay` and ClawHub's `@futrixdev/envrelay` are
  unchanged; only the repository in install commands and registry entries is
  new.

## Consequences

- **The Worker's code is no longer public, only its behaviour.** What matters
  to a user can still be checked from outside: where `/install.sh` leads (a
  HEAD request shows it and is not recorded), and that both hostnames serve
  the release's `install.sh` exactly, which *smoke* checks after every
  release. The READMEs keep saying what each download records.
- **Nothing checks the homepage's version any more.** Setting it is a release
  step (README, "Releasing"); until a maintainer does, the badge and the demo
  name the previous release, as they already did between a release and the
  next deploy.
- **Build provenance names this repository**: `gh attestation verify <file>
  --repo FutrixDev/envrelay-skill`.
- **The old history stays private**, with its pull requests and issues. The
  reasoning they held is in the ADRs, which came across unchanged apart from
  ADR-024's amendments.
- **This repository's age starts on 2026-09-25**, with its first commit, for
  the lists that count it (`docs/publishing.md`, section 4).

## Alternatives rejected

- **Keep one public repository.** Nothing in it gave the records away, but the
  owner wants the site's operation apart from what users install.
- **Keep the old repository public and move the site out of it.** Its whole
  history would stay public, the site's included, along with the commits from
  before ADR-021's reset. A fresh start publishes only what the project is now.
- **Carry over the moved files' history** (`git filter-repo`). It would
  republish those same old commits, for no reader's benefit.
- **Read the homepage's version from the latest release at deploy time.** The
  site could not be deployed before this repository's first release, and
  every deploy would depend on GitHub's API.
