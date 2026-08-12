#!/usr/bin/env bash
# Every place the version is written down has to agree with the manifest.
#
# The number lives in five files and nothing but somebody's memory kept them in
# step, which is how the README sat at `v0.2.0` through the whole of v0.3.0 and
# was only noticed a release later. This is that memory, run in CI.
#
#   ./scripts/check-versions.sh
#
# Exits non-zero naming every file that disagrees. `scripts/release.sh` writes
# all five, so a release cut with it passes this by construction; the check is
# for the hand edit, the half-finished bump, and the file added later that
# nobody remembered to teach.
set -euo pipefail

cd "$(dirname "$0")/.."

# The same expression the release workflow uses to name the tag: every crate
# takes its version from `[workspace.package]`, whose `version` is the first
# one in the root manifest.
manifest=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

vscode=$(grep -m1 '"version"' editors/vscode/package.json | cut -d'"' -f4)

# The topmost *released* heading. `[Unreleased]` sits above it and is skipped
# by requiring a digit, so this is the version the changelog last closed.
changelog=$(grep -m1 -E '^## \[[0-9]' CHANGELOG.md | sed -E 's/^## \[([^]]+)\].*/\1/')

status_line() { # file -> the version its status sentence claims
  grep -m1 -oE '`v[0-9]+\.[0-9]+\.[0-9]+` is the current release' "$1" \
    | sed -E 's/^`v([^`]+)`.*/\1/'
}
readme=$(status_line README.md || true)
roadmap=$(status_line docs/roadmap.md || true)

fail=0
report() { # label, found
  if [ "$2" = "$manifest" ]; then
    printf '  ok    %-28s %s\n' "$1" "$2"
  else
    printf '  WRONG %-28s %s (expected %s)\n' "$1" "${2:-<not found>}" "$manifest"
    fail=1
  fi
}

echo "workspace version: $manifest"
report "editors/vscode/package.json" "$vscode"
report "CHANGELOG.md heading" "$changelog"
report "README.md status" "$readme"
report "docs/roadmap.md status" "$roadmap"

# A changelog with no `[Unreleased]` heading breaks the preview site, which
# renders that section as what `/next/` has over the release.
if ! grep -q '^## \[Unreleased\]' CHANGELOG.md; then
  echo "  WRONG CHANGELOG.md              no [Unreleased] heading"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "Versions disagree. ./scripts/release.sh <version> writes all of them;" >&2
  echo "if this is a hand edit, fix the files named above." >&2
  exit 1
fi

echo "✓ every version agrees"
