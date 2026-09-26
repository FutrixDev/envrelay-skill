# ADR-028: The git inventory runs nothing a repository's config names

Date: 2026-09-26
Status: accepted
Amends: ADR-022 (deterministic mechanics in scripts)

## Context

`git_classify.py` walks the home directory and runs read-only git commands in
every repository it finds: `status`, `remote`, `stash list`, `rev-parse`,
`symbolic-ref`, `rev-list`. Read-only is not the same as running nothing. A
repository's own `.git/config` can name programs that git starts during those
reads:

- `core.fsmonitor`, which `status` asks which files changed;
- a filter driver's `clean` or `process` command, which `status` pipes a file
  through when its timestamp changed but its size did not;
- `gpg.program`, which `stash list` calls for every signed stash once
  `log.showSignature` is on;
- the same settings in a checked-out submodule, where `status` runs a second
  `git status`.

A repository that came from someone else — a cloned exercise, an unpacked
archive, a directory another tool wrote — is data the scan finds, not something
the user chose to trust. The skill runs the scan before the user has looked at
any of them.

ClawHub re-scanned v1.0.2 on 2026-09-25 at 15:43 UTC and moved its audit from
Pass to Review. Its one substantive finding (A.I.G T09, high) is this: the
inventory can run code a scanned repository configures.

## Decision

- **Every git command in `git_classify.py` runs with the repository's helpers
  switched off**: `core.fsmonitor` empty (not `false`, which git before 2.36
  runs as a command), `core.hooksPath` set to `/dev/null`, and
  `log.showSignature` off.
- **Filter drivers that only the repository defines are switched off for
  `status`**: `clean`, `smudge` and `process` empty, `required` false. The
  script lists the repository's config with its scopes, and those of its
  checked-out submodules, and switches off each driver defined outside the
  system, global and command scopes. A driver the user installed — git lfs
  puts itself in the global config — keeps running.
- **The settings travel in `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n`/
  `GIT_CONFIG_VALUE_n`, not `-c`.** `-c` splits at the first `=`, and a
  driver's name may contain one. The environment also reaches the `git status`
  that runs inside each submodule.
- **The script needs git 2.31 or newer**, the first with `GIT_CONFIG_COUNT`.
  With an older git every repository is `unknown`, with the version in the
  detail; no git command runs in the repository first. The skill's
  `compatibility` field says so, and `install.sh`, which checks for what the
  scripts need, names an older git and the version it found.
- **`tests/git_classify.py` proves each case both ways**: the script leaves the
  program unrun, and plain git making the same read runs it. CI runs it on
  Linux under Python 3.9 and on macOS.
- This ships as v1.0.3.

## Consequences

- **A file a repository-only filter would have cleaned can count as
  uncommitted.** That errs towards carrying the repository whole, the safe
  direction: at worst a clean repository gets `files` instead of `clone`.
- **Users with git older than 2.31 lose the inventory.** macOS's command line
  tools shipped 2.24 and 2.30 in the Xcode 11 and 12 years; Debian 11 has
  2.30. The installer tells them at install time, not at the first backup.
- **The `git` on `PATH` is trusted.** It is the user's own environment, not
  something the scanned repository controls.

## Alternatives rejected

- **Only switch off `core.fsmonitor`.** It is the obvious one, but filters and
  signature checks run from the same commands.
- **Switch off every filter driver, the user's too.** Simpler, but every git
  lfs repository would read as fully uncommitted and be carried as files.
- **Stop running `git status` and read the index directly.** It would take a
  reimplementation of git's change detection, and still have to decide what a
  filter would have done.
