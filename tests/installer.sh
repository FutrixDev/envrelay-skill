#!/bin/sh
# End-to-end tests for skills/envrelay/install.sh. The release it installs is
# packaged on the spot by .github/scripts/package-release.sh, the same script
# the release workflow runs, and every scenario gets a throwaway HOME.
#
#   tests/installer.sh [BINARY]          BINARY: target/release/envrelay
#   SH=dash tests/installer.sh           run the installer under another shell
#
# Nothing outside a temporary directory is read or written.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
binary=${1:-$root/target/release/envrelay}
[ -x "$binary" ] || {
	echo "no $binary: run cargo build --release first" >&2
	exit 2
}
SH=$(command -v "${SH:-sh}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# One binary stands in for every platform, so the installer finds its asset
# wherever the test runs.
for asset in universal-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
	mkdir -p "$work/bins/bin-$asset"
	cp "$binary" "$work/bins/bin-$asset/envrelay"
done
sh "$root/.github/scripts/package-release.sh" "$work/bins" "$work/release" >/dev/null
version=$("$binary" --version)

passed=0 failed=0
check() {
	what=$1
	shift
	if "$@"; then
		passed=$((passed + 1))
	else
		failed=$((failed + 1))
		echo "FAIL [$scenario] $what" >&2
		sed 's/^/    | /' "$out" >&2
	fi
}

# scenario NAME: a fresh HOME and the paths the checks look at.
scenario() {
	scenario=$1
	h=$work/$1
	mkdir "$h"
	bin=$h/.local/bin/envrelay
	skill=$h/.agents/skills/envrelay
	link=$h/.claude/skills/envrelay
	out=$work/$1.out
	: >"$out"
	url=$work/release
	test_path=/usr/bin:/bin
	test_shell=/bin/zsh
}

# run_installer [ARGS...]: run the release's install.sh as a user whose PATH
# lacks ~/.local/bin. Sets $status; the output is in $out.
run_installer() {
	set +e
	env HOME="$h" SHELL="$test_shell" PATH="$test_path" ZDOTDIR= XDG_CONFIG_HOME= \
		CLAUDE_CONFIG_DIR= ENVRELAY_BIN_DIR= SUDO_USER= ENVRELAY_DOWNLOAD_URL="$url" \
		"$SH" "${installer:-$work/release/install.sh}" "$@" >"$out" 2>&1
	status=$?
	set -e
}

# run_piped SCRIPT [ARGS...]: the way the one-liner runs it, from stdin.
run_piped() {
	script=$1
	shift
	set +e
	env HOME="$h" SHELL="$test_shell" PATH="$test_path" ZDOTDIR= XDG_CONFIG_HOME= \
		CLAUDE_CONFIG_DIR= ENVRELAY_BIN_DIR= SUDO_USER= ENVRELAY_DOWNLOAD_URL="$url" \
		"$SH" -s -- "$@" <"$script" >"$out" 2>&1
	status=$?
	set -e
}

status_is() { [ "$status" = "$1" ]; }
failed_run() { [ "$status" != 0 ]; }
said() { grep -Fq -- "$1" "$out"; }
absent() { [ ! -e "$1" ] && [ ! -L "$1" ]; }
home_empty() { [ -z "$(ls -A "$h")" ]; }
links_to_skill() { [ -L "$1" ] && [ "$(cd -P "$1" && pwd)" = "$(cd -P "$skill" && pwd)" ]; }
path_line_once() { [ "$(grep -Fxc "export PATH=\"$2:\$PATH\"" "$1" 2>/dev/null)" = 1 ]; }
no_leftovers() { [ -z "$(find "$h" -name '.envrelay-skill.*' -o -name '.envrelay.new.*')" ]; }

scenario fresh
run_piped "$work/release/install.sh"
check "exits 0" status_is 0
check "installs the binary" [ -x "$bin" ]
check "the binary runs" [ "$("$bin" --version)" = "$version" ]
check "installs the skill" [ -f "$skill/SKILL.md" ]
check "the skill carries the installer" [ -x "$skill/install.sh" ]
check "marks the skill as its own" [ -f "$skill/.envrelay-installer" ]
check "ships no bytecode" absent "$skill/scripts/__pycache__"
check "links the skill for Claude Code" links_to_skill "$link"
check "creates nothing for absent agents" absent "$h/.kiro"
check "adds the PATH line" path_line_once "$h/.zshrc" "$h/.local/bin"
check "leaves no staging behind" no_leftovers
check "says what to do next" said "Next: start a new session"

run_installer
check "runs again" status_is 0
check "adds the PATH line only once" path_line_once "$h/.zshrc" "$h/.local/bin"
check "keeps the link" links_to_skill "$link"
check "reports the link as in place" said "already points at it"

echo stale >"$skill/scripts/gone_in_this_release.py"
run_installer
check "upgrades" status_is 0
check "replaces the whole skill" absent "$skill/scripts/gone_in_this_release.py"
check "leaves no staging behind after an upgrade" no_leftovers

# Inside an installed skill it asks for the release that matches the skill.
url=
installer=$skill/install.sh
run_installer --dry-run --bin-only
check "asks for the skill's own release" said "/releases/download/v${version#envrelay }"
run_installer --dry-run --bin-only --version v9.8.7
check "lets --version override it" said "/releases/download/v9.8.7"
installer=

scenario kiro-cline
mkdir "$h/.kiro" "$h/.cline"
run_installer
check "exits 0" status_is 0
check "links the skill for Kiro" links_to_skill "$h/.kiro/skills/envrelay"
check "links the skill for Cline" links_to_skill "$h/.cline/skills/envrelay"

scenario own-copy
mkdir -p "$link"
echo mine >"$link/SKILL.md"
run_installer
check "exits 0" status_is 0
check "leaves a copy the user made" [ "$(cat "$link/SKILL.md")" = mine ]
check "does not turn it into a link" [ ! -L "$link" ]
check "still installs the skill" [ -f "$skill/SKILL.md" ]
check "says what it left" said "left $link as it is"

scenario foreign-link
mkdir -p "$h/.claude/skills" "$h/elsewhere"
ln -s "$h/elsewhere" "$link"
run_installer
check "exits 0" status_is 0
check "leaves a link to somewhere else" [ "$(readlink "$link")" = "$h/elsewhere" ]

scenario npx-skills
mkdir -p "$skill" "$h/.claude/skills"
echo theirs >"$skill/SKILL.md"
ln -s ../../.agents/skills/envrelay "$link"
run_installer
check "exits 0" status_is 0
check "leaves a skill another tool installed" [ "$(cat "$skill/SKILL.md")" = theirs ]
check "does not claim it" absent "$skill/.envrelay-installer"
check "leaves that tool's relative link" [ "$(readlink "$link")" = ../../.agents/skills/envrelay ]
check "still installs the binary" [ -x "$bin" ]
check "says what it left" said "left $skill as it is"

scenario linked-skill
mkdir -p "$h/clone/skills/envrelay" "$h/.agents/skills"
echo clone >"$h/clone/skills/envrelay/SKILL.md"
ln -s "$h/clone/skills/envrelay" "$skill"
run_installer
check "exits 0" status_is 0
check "leaves a skill linked to a clone" [ -L "$skill" ]
check "leaves the clone alone" [ "$(cat "$h/clone/skills/envrelay/SKILL.md")" = clone ]

# gh skill install copies the skill without executable bits and rewrites its
# frontmatter: sorted keys, four-space indentation, metadata of its own.
scenario gh-skill
mkdir -p "$link"
cp "$root/skills/envrelay/install.sh" "$link/install.sh"
chmod 644 "$link/install.sh"
cat >"$link/SKILL.md" <<'EOF'
---
allowed-tools: Bash(python3 ${CLAUDE_SKILL_DIR}/scripts/*)
clawdis:
    requires:
        bins:
            - python3
description: Use when backing up or restoring a development environment.
metadata:
    github-path: skills/envrelay
    github-ref: v9.8.6
    github-repo: https://github.com/FutrixDev/envrelay-skill
    github-tree-sha: 4b825dc642cb6eb9a060e54bf8d69288fbee4904
    version: 9.8.6
name: envrelay
---
EOF
url=
installer=$link/install.sh
run_installer --dry-run --bin-only
check "exits 0" status_is 0
check "reads the version from rewritten frontmatter" said "/releases/download/v9.8.6"
installer=

scenario bin-only
run_installer --bin-only
check "exits 0" status_is 0
check "installs the binary" [ -x "$bin" ]
check "installs no skill" absent "$h/.agents"
check "makes no agent directory" absent "$h/.claude"

scenario no-modify-path
run_installer --no-modify-path
check "exits 0" status_is 0
check "writes no rc file" absent "$h/.zshrc"
check "says PATH lacks the directory" said "is not on your PATH"

scenario on-path
test_path=$h/.local/bin:/usr/bin:/bin
run_installer
check "exits 0" status_is 0
check "writes no rc file when PATH has the directory" absent "$h/.zshrc"

scenario bash
test_shell=/bin/bash
: >"$h/.profile"
run_installer
check "exits 0" status_is 0
if [ "$(uname -s)" = Darwin ]; then
	check "uses the login file bash reads on macOS" path_line_once "$h/.profile" "$h/.local/bin"
	check "creates no .bash_profile beside it" absent "$h/.bash_profile"
else
	check "uses .bashrc on Linux" path_line_once "$h/.bashrc" "$h/.local/bin"
fi

scenario fish
test_shell=/usr/bin/fish
run_installer
check "exits 0" status_is 0
check "writes a fish conf.d file" grep -Fq "set -gx PATH \"$h/.local/bin\" \$PATH" "$h/.config/fish/conf.d/envrelay.fish"

scenario bin-dir
# shellcheck disable=SC2088 # the installer expands a quoted ~ itself
run_installer --bin-dir '~/tools/bin'
check "exits 0" status_is 0
check "installs where it is told" [ -x "$h/tools/bin/envrelay" ]
check "puts that directory on PATH" path_line_once "$h/.zshrc" "$h/tools/bin"
check "leaves the default directory alone" absent "$h/.local"

scenario dry-run
run_installer --dry-run
check "exits 0" status_is 0
check "changes nothing" home_empty
check "says it is a dry run" said "Dry run"
check "shows where the skill would go" said "would install the skill in $skill"
check "shows the link it would make" said "would link $link"
check "shows the PATH change" said "would add $h/.local/bin to PATH in $h/.zshrc"

scenario uninstall
mkdir -p "$h/.kiro" "$h/.cline/skills" "$h/elsewhere"
ln -s "$h/elsewhere" "$h/.cline/skills/envrelay"
run_installer
run_installer --uninstall --dry-run
check "previews the uninstall" status_is 0
check "keeps the binary in a dry run" [ -x "$bin" ]
check "keeps the skill in a dry run" [ -f "$skill/SKILL.md" ]
run_installer --uninstall
check "uninstalls" status_is 0
check "removes the binary" absent "$bin"
check "removes the skill" absent "$skill"
check "removes the Claude Code link" absent "$link"
check "removes the Kiro link" absent "$h/.kiro/skills/envrelay"
check "keeps a link it did not make" [ "$(readlink "$h/.cline/skills/envrelay")" = "$h/elsewhere" ]
check "keeps the PATH line and says where" said "kept the PATH line in $h/.zshrc"

scenario uninstall-theirs
mkdir -p "$skill"
echo theirs >"$skill/SKILL.md"
run_installer --uninstall
check "exits 0" status_is 0
check "keeps a skill another tool installed" [ "$(cat "$skill/SKILL.md")" = theirs ]

scenario tampered-binary
cp -R "$work/release" "$work/tampered-binary.release"
for f in "$work/tampered-binary.release"/envrelay-*-*.tar.gz; do printf x >>"$f"; done
url=$work/tampered-binary.release
run_installer
check "fails" failed_run
check "names the mismatch" said "does not match SHA256SUMS"
check "installs nothing" home_empty

scenario tampered-skill
cp -R "$work/release" "$work/tampered-skill.release"
printf x >>"$work/tampered-skill.release/envrelay-skill.tar.gz"
url=$work/tampered-skill.release
run_installer
check "fails" failed_run
check "installs nothing, not even the binary" home_empty

scenario truncated
size=$(wc -c <"$work/release/install.sh")
for pct in 10 25 50 75 90 99; do
	head -c $((size * pct / 100)) "$work/release/install.sh" >"$work/cut.sh"
	run_piped "$work/cut.sh"
	check "a download cut at $pct% installs nothing" home_empty
done

scenario bad-input
run_installer --frobnicate
check "refuses an unknown option" failed_run
check "names it" said "unknown option: --frobnicate"
run_installer --version '1.0;rm'
check "refuses a malformed version" failed_run
check "installs nothing" home_empty

echo "installer tests ($SH): $passed passed, $failed failed"
[ "$failed" = 0 ]
