# ADR-022: A third layer for deterministic mechanics

Date: 2026-08-31
Status: accepted
Extends: ADR-021 (MVP v1 reset)

## Context

The first real backup+restore test (issue #17) failed in eight places, and not
one of them was in the Rust core or in the skill's judgement calls. Every
failure was the agent *improvising a mechanical procedure as shell one-liners*:
`cp` following a dangling symlink and sinking a directory copy, per-package
registry queries turning a 200-package diff into 200 round-trips, `git`
metadata read from a repo whose HEAD did not resolve, restore progress kept in
the agent's head and lost when the session was.

ADR-021 gave us two layers: Core (holds the passphrase, nothing else) and the
skill (holds the judgement, all of it). The test showed those two layers had
been quietly sharing a third job — deterministic mechanics — and that
improvising it fresh each run is where the defects live.

## Decision

Add a scripts layer: `skills/envrelay/scripts/`, Python 3.9+ standard library
only, one file per job, JSON on stdout.

The assignment rule for any new behaviour is now:

1. **Does it need the passphrase?** → Core. This is the *only* criterion for
   growing the binary. `inspect` and `verify` qualify (they open the encrypted
   stream); nothing else proposed in issue #17 does.
2. **Is it deterministic mechanics — the same inputs always warranting the
   same outputs, no decision in the middle?** → a script.
3. **Everything else — anything with a decision in it** → the skill, in prose.

The scripts are subordinate to the skill by construction:

- A script never decides, installs, uninstalls, or deletes user data. It
  enumerates, copies, classifies, records, and reports.
- A script's output is input *to the skill's judgement*, never a command list.
  `git_classify.py` reports counts; choosing `clone` vs `files` vs `metadata`
  stays in SKILL.md where the user can be consulted.
- Per-item problems (an unreadable file, a manager that errored) are data in
  the JSON, not process failures. Exit codes are for "could not run at all".
- Zero dependencies and single files, so "run the script" never becomes its
  own installation step on a machine that is mid-migration.

Core stays passphrase-only, and keeps its existing promise that it **never
parses the manifest**. That is why `verify` checks stream integrity (the age
MAC already proves every byte) but does not cross-check per-file checksums —
that comparison happens on plaintext trees, outside the passphrase boundary,
in `verify_tree.py` against the manifest's `checksums` map.

## Consequences

- The skill directs; scripts execute; Core encrypts. Three layers, one
  criterion each, and issue #17's failure classes each land in exactly one:
  copy robustness in `stage_copy.py`, repo-state classification in
  `git_classify.py`, batch inventory and the app/bundle-id identity in
  `sw_inventory.py`, restore resumability in `restore_ledger.py`, tree
  verification in `verify_tree.py`, and read-only backup confidence in
  `envrelay inspect` / `envrelay verify`.
- Scripts travel with the skill, not with the binary. Updating a procedure is
  a docs-and-scripts change, no release.
- Windows remains out of scope (ADR-021); the scripts use POSIX facts
  (modes, symlinks) deliberately rather than abstracting them away.
- Cloud upload remains out of scope. The `.sha256` sidecar written at backup
  time is the whole of the "did the cloud round-trip corrupt it" answer for
  now.
