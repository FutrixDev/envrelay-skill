#!/bin/sh
# Check that every place a release version is written agrees.
#
#   .github/scripts/check-versions.sh [TAG]
#
# Cargo.toml's version is what `envrelay --version` prints. The skill's
# (metadata.version in SKILL.md) picks the release its install.sh fetches, so a
# skill installed from anywhere gets the binary it was written for. The Claude
# Code plugin's (.claude-plugin/plugin.json) is what /plugin compares to decide
# whether there is an update. plugin.json at the root is the same manifest for
# Copilot CLI, VS Code and awesome-copilot, in the Agent Plugins format
# (ADR-026), so it must match the Claude Code one field for field. With TAG, the
# tag must be v<version> as well. envrelay.com names the version too, in its
# own repository (ADR-025).
set -eu

root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
cd "$root"

failed=0
fail() {
	echo "$1" >&2
	failed=1
}

version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$version" ] || {
	echo "no version in Cargo.toml" >&2
	exit 1
}

# Ask the installer which release it would fetch instead of reading SKILL.md a
# second way: this is the path a user's agent takes.
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
plan=$(HOME="$home" ENVRELAY_DOWNLOAD_URL='' ENVRELAY_BIN_DIR='' \
	sh skills/envrelay/install.sh --dry-run --bin-only --no-modify-path 2>&1) || {
	printf '%s\n' "$plan" >&2
	exit 1
}
url=$(printf '%s\n' "$plan" | sed -n 's/^==> Would download for .* from //p')
case $url in
*/releases/download/v"$version") ;;
*) fail "skills/envrelay/install.sh would fetch ${url:-nothing}, not release v$version (metadata.version in SKILL.md)" ;;
esac

plugin=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' .claude-plugin/plugin.json)
[ "$plugin" = "$version" ] || fail ".claude-plugin/plugin.json has version $plugin, Cargo.toml has $version"

# The Agent Plugins manifest is the Claude Code one plus the $schema that opts
# it into that format.
agent=$(python3 - plugin.json .claude-plugin/plugin.json <<'EOF'
import json, sys
schema = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json"
agent, claude = (json.load(open(path)) for path in sys.argv[1:])
if agent.pop("$schema", None) != schema:
    print(f'plugin.json must have "$schema": "{schema}"')
differ = sorted(k for k in agent.keys() | claude.keys() if agent.get(k) != claude.get(k))
if differ:
    print("plugin.json and .claude-plugin/plugin.json differ in " + ", ".join(differ))
EOF
)
[ -z "$agent" ] || fail "$agent"

if [ $# -gt 0 ] && [ "$1" != "v$version" ]; then
	fail "tag $1 does not match Cargo.toml's version: expected v$version"
fi

[ "$failed" = 0 ] || exit 1
echo "versions agree: $version"
