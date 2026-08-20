#!/usr/bin/env bash
# Run the standing benchmark against every release tag on this machine.
#
# The numbers in docs/roadmap.md were measured on whatever hardware was to hand
# at the time, so they cannot answer "did release N make this slower". This can:
# it builds each tag's own `mirzam-bench` and runs them all on one machine, one
# after another. `crates/mirzam-cli/src/bin/mirzam-bench.rs` has not changed
# since v0.1.0, so the decks being measured are the same decks.
#
#   ./scripts/bench-history.sh                 # every tag, then the working tree
#   ./scripts/bench-history.sh v0.6.0 v0.8.0   # just these
#
# Builds land in /tmp/mirzam-bench-history and are reused across runs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${MIRZAM_BENCH_WORK:-/tmp/mirzam-bench-history}"
mkdir -p "$work"

if [ $# -gt 0 ]; then
  tags=("$@")
else
  mapfile -t tags < <(git -C "$root" tag | sort -V)
fi

build() {  # build <name> <checkout-dir>
  local name="$1" dir="$2"
  echo "building $name ..." >&2
  ( cd "$dir" && CARGO_TARGET_DIR="$work/target-$name" \
      cargo build --release -q -p mirzam-cli --bin mirzam-bench ) >&2
}

for tag in "${tags[@]}"; do
  dir="$work/src-$tag"
  if [ ! -d "$dir" ]; then
    git -C "$root" worktree add --detach -f "$dir" "$tag" >/dev/null 2>&1 \
      || { echo "cannot check out $tag, skipping" >&2; continue; }
  fi
  build "$tag" "$dir" || { echo "$tag does not build here, skipping" >&2; continue; }
done

build "working-tree" "$root"

echo
for tag in "${tags[@]}" working-tree; do
  bin="$work/target-$tag/release/mirzam-bench"
  [ -x "$bin" ] || continue
  echo "=== $tag ==="
  "$bin"
  echo
done
