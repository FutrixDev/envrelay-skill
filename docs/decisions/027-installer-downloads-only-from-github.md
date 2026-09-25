# ADR-027: The installer downloads only from EnvRelay's release on GitHub

Date: 2026-09-25
Status: accepted
Amends: ADR-024 (one command installs the binary and the skill), ADR-026
(frontmatter and manifests that awesome-copilot accepts)

## Context

ADR-024 gave `install.sh` an environment variable that changed where it
downloads from: `ENVRELAY_DOWNLOAD_URL` replaced the GitHub release with a
mirror, or with a local directory holding the release's files. Only the
installer's `--help` and the skill's frontmatter mentioned it, and only two
things ever set it: the release workflow's *verify* jobs, which install a
release before it is published, and `tests/installer.sh`. No mirror exists.

ClawHub's security audit rated v1.0.0 and v1.0.1 "Review" because of it. Its
one finding (T03, high) is that the variable accepted plain http, while
`SHA256SUMS` came from the same place as the files it checks. Whoever could set
the variable, or answer that http request, chose the binary, and the installer
runs a new binary once (`--version`) before it puts it in place.

The owner's rule: a variable that only tests use does not belong in what is
released.

## Decision

- **`install.sh` downloads only from
  `https://github.com/FutrixDev/envrelay-skill/releases`**: the release that
  matches the skill beside it, the one `--version` names, or the latest.
  Nothing changes that: no variable or option, no http, no `file://` and no
  local path. curl stays held to HTTPS and TLS 1.2.
- **The tests replace curl instead.** `tests/stubs/curl`, first on `PATH`,
  answers the installer's downloads from a release packaged on the spot. It
  refuses a call that is not held to HTTPS and TLS 1.2, or that asks for
  anything but this repository's release. `tests/installer.sh` and the
  *verify* jobs both use it, so they run the installer users get, calling curl
  the way it does for them.
- **The frontmatter's `clawdis` block declares one optional environment
  variable**, `ENVRELAY_BIN_DIR`.
- This ships as v1.0.2.

## Consequences

- **A mirror or a local copy of a release no longer works.** Anyone who set
  `ENVRELAY_DOWNLOAD_URL` now downloads from GitHub. It shipped only in v1.0.0
  and v1.0.1, both released on 2026-09-25. A machine that cannot reach GitHub
  cannot use the installer.
- **No test takes a path that users do not.** Every install the tests make
  asks for the same URLs a user's does.
- **The stub has to follow the installer.** A change to how `install.sh` calls
  curl fails the tests until the stub accepts it, which is the point: the
  flags that hold curl to HTTPS are checked on every run.
- **wget stays untested**, as before: the stub stands in for curl, and every
  runner has curl.
- **The checksums still come from the release they check.** They catch a
  corrupted or mixed-up download, not a forged release. What the installer
  trusts is HTTPS to GitHub; build provenance (`gh attestation verify`) is how
  anyone checks that a file came from this repository's release workflow.

## Alternatives rejected

- **Keep the variable, https only.** The finding would shrink, but a variable
  that only tests use would still ship.
- **Sign `SHA256SUMS` and pin the key in `install.sh`.** It would make a mirror
  safe, but no one needs a mirror, and a signing key is one more secret to
  keep.
- **Take `--passphrase-file` out of the binary too**, the other thing only
  tests use. The owner declined: the skill is the core of the product, and it
  is the agent's own judgment, guided by SKILL.md's first rule, that keeps the
  passphrase out of its hands.
