#!/usr/bin/env bash
# Cut a release: write the version everywhere it is written down, close the
# changelog, and run the gate.
#
#   ./scripts/release.sh 0.5.0             bump, then run the gate
#   ./scripts/release.sh 0.5.0 --dry-run   show the edits, change nothing
#   ./scripts/release.sh 0.5.0 --no-gate   edit only, run nothing
#
# It does not commit, push, or tag. Those are the steps where a human or an
# agent should still be looking, and the tag is made by the Release workflow
# from this manifest rather than by hand - see docs/development.md.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=""
DRY_RUN=0
GATE=1
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --no-gate) GATE=0 ;;
    -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown flag: $arg" >&2; exit 2 ;;
    *)  VERSION="$arg" ;;
  esac
done

[ -n "$VERSION" ] || { echo "usage: $0 <version> [--dry-run] [--no-gate]" >&2; exit 2; }

case "$VERSION" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must be X.Y.Z, not '$VERSION'" >&2; exit 2 ;;
esac

CURRENT=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
DATE=$(date -u +%Y-%m-%d)
FILES=(Cargo.toml Cargo.lock CHANGELOG.md README.md docs/roadmap.md editors/vscode/package.json)

# --- preflight ---------------------------------------------------------------

# A release commit should carry the bump and nothing else, and --dry-run
# restores these files afterwards, which would take an unrelated edit with it.
dirty=$(git status --porcelain -- "${FILES[@]}")
if [ -n "$dirty" ]; then
  echo "These files have uncommitted changes; commit or stash them first:" >&2
  echo "$dirty" >&2
  exit 1
fi

if [ "$VERSION" = "$CURRENT" ]; then
  echo "already at $CURRENT" >&2
  exit 1
fi
if [ "$(printf '%s\n%s\n' "$CURRENT" "$VERSION" | sort -V | tail -1)" != "$VERSION" ]; then
  echo "$VERSION is below the current $CURRENT" >&2
  exit 1
fi

# The changelog's unreleased section is the release. Nothing in it means there
# is nothing to ask anyone to upgrade for.
unreleased=$(awk '/^## \[Unreleased\]/ { on = 1; next } on && /^## \[/ { exit } on { print }' CHANGELOG.md)
if ! printf '%s' "$unreleased" | grep -q '^### '; then
  echo "CHANGELOG.md's [Unreleased] section is empty - nothing to release." >&2
  exit 1
fi

# Pre-1.0, a minor bump is what may change the markup, so anything that adds,
# changes or removes goes in one; a patch carries fixes. This is advice, not a
# rule - a judgement call that disagrees is fine, and says so out loud.
kind_expected=patch
printf '%s' "$unreleased" | grep -qE '^### (Added|Changed|Removed)' && kind_expected=minor
cur_minor=${CURRENT#*.}; cur_minor=${cur_minor%.*}
new_minor=${VERSION#*.}; new_minor=${new_minor%.*}
kind_given=patch
[ "$new_minor" != "$cur_minor" ] && kind_given=minor
if [ "$kind_given" != "$kind_expected" ]; then
  echo "note: [Unreleased] reads like a $kind_expected bump, and $CURRENT -> $VERSION is a $kind_given one."
fi

echo "==> $CURRENT -> $VERSION ($DATE)"

# --- the edits ---------------------------------------------------------------

# A dry run puts the edits on disk to diff them, so it has to take them back on
# every way out - including the one that bit first: `--dry-run | head` closes
# the pipe, the script dies on SIGPIPE part-way through printing, and without
# this the bump is left sitting in the working tree.
if [ "$DRY_RUN" -eq 1 ]; then
  trap 'git checkout -- "${FILES[@]}" 2>/dev/null || true' EXIT INT TERM PIPE
fi

rewrite() { # file, awk program (version available as `ver`)
  awk -v ver="$VERSION" -v date="$DATE" "$2" "$1" > "$1.tmp"
  mv "$1.tmp" "$1"
}

# The first `version` in the root manifest is `[workspace.package]`'s, which
# every crate inherits.
rewrite Cargo.toml '
  !done && /^version = "/ { sub(/"[^"]*"/, "\"" ver "\""); done = 1 } { print }'

rewrite editors/vscode/package.json '
  !done && /"version": "/ { sub(/: "[^"]*"/, ": \"" ver "\""); done = 1 } { print }'

# [Unreleased] becomes this version, and a fresh empty one takes its place.
rewrite CHANGELOG.md '
  !done && /^## \[Unreleased\]/ {
    print "## [Unreleased]"; print ""; print "Nothing yet."; print ""
    print "## [" ver "] - " date
    done = 1
    next
  }
  { print }'

# The status sentence in both places a reader looks for "what is out".
for f in README.md docs/roadmap.md; do
  rewrite "$f" '
    { gsub(/`v[0-9]+\.[0-9]+\.[0-9]+` is the current release/,
           "`v" ver "` is the current release"); print }'
done

# Every crate version moves with the workspace, so the lockfile has to follow
# or `cargo build --locked` in the release workflow fails on a stale lock.
cargo update --workspace --offline >/dev/null 2>&1 || cargo check --workspace --quiet

./scripts/check-versions.sh

if [ "$DRY_RUN" -eq 1 ]; then
  echo
  git --no-pager diff -- "${FILES[@]}"
  git checkout -- "${FILES[@]}"
  echo
  echo "(dry run - nothing was kept)"
  exit 0
fi

# --- the gate ----------------------------------------------------------------

if [ "$GATE" -eq 0 ]; then
  echo "==> gate skipped (--no-gate)"
else
  DECKS=(01-start 02-writing 03-layout 04-components 05-motion 06-theming pitch seminar)

  echo "==> formatting, lints, tests"
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets
  cargo test --workspace

  echo "==> sample decks"
  cargo build --release --bin mirzam
  for d in "${DECKS[@]}"; do
    ./target/release/mirzam build "examples/$d.md" -o "$(mktemp -d)/$d" >/dev/null
  done

  # `mirzam check` renders each deck and fails on content clipped by its pane,
  # an unresolved connector, an animation left mid-entrance. It needs a browser
  # and nothing else - no npm, no checkout.
  if [ -n "${MIRZAM_CHROMIUM:-}" ] || command -v chromium >/dev/null || \
     command -v chromium-browser >/dev/null || command -v google-chrome >/dev/null; then
    echo "==> layout check"
    for d in "${DECKS[@]}"; do
      ./target/release/mirzam check "examples/$d.md"
    done
  else
    echo "!! no Chromium found - layout check skipped. Set MIRZAM_CHROMIUM and run:"
    echo "     for d in ${DECKS[*]}; do ./target/release/mirzam check examples/\$d.md; done"
  fi

  # Read the second run: the first pays for a cold page cache and can look like
  # a regression that is not there. What must stay flat is edit latency, not
  # full-build time - compare against the table in docs/roadmap.md.
  echo "==> benchmark (first run discarded)"
  cargo run --release -p mirzam-cli --bin mirzam-bench >/dev/null
  cargo run --release -p mirzam-cli --bin mirzam-bench

  # This builds the WASM package on its way to the extension, so it covers the
  # browser build too.
  if command -v npx >/dev/null; then
    echo "==> WASM and the VS Code extension"
    ./scripts/build-vsix.sh
  else
    echo "==> WASM (no npx, so the extension is not packaged)"
    ./scripts/build-wasm.sh
  fi
fi

# --- what is left to do ------------------------------------------------------

cat <<EOF

✓ v$VERSION is written down and the gate is green.

Still to do, in order:

  1. git commit -am "Release v$VERSION"     — say why this bump, for the notes
  2. git push origin HEAD:main              — the release is cut from main
  3. run the Release workflow with 'publish' checked:
       gh workflow run release.yml --ref main -f publish=true
     No gh? Actions tab -> Release -> Run workflow, or POST to
     /repos/{owner}/{repo}/actions/workflows/release.yml/dispatches. Do not
     push a tag: that is refused for credentials scoped to branches, and the
     workflow makes the tag from this manifest anyway.
  4. check the site: publishing triggers Pages, which moves the front page
     onto the new tag.
EOF
