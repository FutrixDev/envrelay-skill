#!/bin/sh
# Assemble the release assets that install.sh downloads.
#
#   .github/scripts/package-release.sh BINS OUT
#
# BINS holds one directory per platform, bin-<asset>/envrelay, the way the
# release workflow's build jobs upload them. OUT must be new or empty; it gets
#
#   envrelay-<asset>.tar.gz  per platform: envrelay-<asset>/ with the binary,
#                            both licenses and the README
#   envrelay-skill.tar.gz    the skill directory, as envrelay/
#   install.sh               the installer, the same file the skill carries
#   SHA256SUMS               checksums of all of the above
#
# tests/installer.sh builds its fake release with this script too, so the
# installer is always tested against the layout it meets in a real release.
set -eu

[ $# -eq 2 ] || {
	echo "usage: $0 BINS OUT" >&2
	exit 2
}
bins=$1
out=$2
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
mkdir -p "$out"
out=$(cd "$out" && pwd)
[ -z "$(ls -A "$out")" ] || {
	echo "$out is not empty" >&2
	exit 1
}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Keep macOS tar from adding ._ files; keep GNU tar from recording the builder.
export COPYFILE_DISABLE=1
if tar --version 2>/dev/null | grep -q 'GNU tar'; then
	set -- --owner=0 --group=0 --numeric-owner
else
	set --
fi

found=0
for dir in "$bins"/bin-*; do
	[ -f "$dir/envrelay" ] || continue
	name=envrelay-${dir##*/bin-}
	mkdir "$work/$name"
	cp "$dir/envrelay" "$work/$name/envrelay"
	chmod 755 "$work/$name/envrelay"
	cp "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/README.md" "$work/$name/"
	(cd "$work" && tar "$@" -czf "$out/$name.tar.gz" "$name")
	found=$((found + 1))
done
[ "$found" -gt 0 ] || {
	echo "no bin-*/envrelay in $bins" >&2
	exit 1
}

# The skill's files that git knows about: what is committed, plus anything new
# and not ignored, so __pycache__ and editor droppings never ship. In a clean
# checkout that is exactly the committed tree.
(
	cd "$root"
	git ls-files --cached --others --exclude-standard -- skills/envrelay |
		while IFS= read -r f; do
			if [ -f "$f" ]; then printf '%s\n' "${f#skills/}"; fi
		done
) >"$work/skill-files"
grep -qx 'envrelay/SKILL.md' "$work/skill-files" || {
	echo "skills/envrelay/SKILL.md is missing" >&2
	exit 1
}
(cd "$root/skills" && tar "$@" -czf "$out/envrelay-skill.tar.gz" -T "$work/skill-files")

cp "$root/skills/envrelay/install.sh" "$out/install.sh"
chmod 755 "$out/install.sh"

cd "$out"
if command -v sha256sum >/dev/null 2>&1; then
	sha256sum -- *.tar.gz install.sh >SHA256SUMS
else
	shasum -a 256 -- *.tar.gz install.sh >SHA256SUMS
fi
cat SHA256SUMS
