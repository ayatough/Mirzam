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
# Each binary runs MIRZAM_BENCH_REPS times (5 by default) and the table reports
# the fastest run of each, because a slow run means the machine was busy and a
# fast one cannot mean the code was: the minimum is the honest estimate.
#
# Builds land in /tmp/mirzam-bench-history and are reused across runs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${MIRZAM_BENCH_WORK:-/tmp/mirzam-bench-history}"
reps="${MIRZAM_BENCH_REPS:-5}"
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

raw="$work/raw.txt"
: > "$raw"
for tag in "${tags[@]}" working-tree; do
  bin="$work/target-$tag/release/mirzam-bench"
  [ -x "$bin" ] || continue
  echo "measuring $tag ($reps runs) ..." >&2
  for _ in $(seq "$reps"); do
    echo "@@ $tag"
    "$bin"
  done >> "$raw"
done

python3 - "$raw" <<'PY'
import re, sys

line = re.compile(
    r"^\s*(?P<deck>.+?): full\s+(?P<full>[\d.]+) ms \| single edit\s+(?P<edit>[\d.]+) ms"
)
best, order, decks = {}, [], []
tag = None
for row in open(sys.argv[1], encoding="utf-8"):
    if row.startswith("@@ "):
        tag = row[3:].strip()
        if tag not in order:
            order.append(tag)
        continue
    m = line.match(row)
    if not m:
        continue
    deck = m["deck"].strip()
    if deck not in decks:
        decks.append(deck)
    for metric, value in (("full", float(m["full"])), ("edit", float(m["edit"]))):
        key = (tag, deck, metric)
        best[key] = min(best.get(key, value), value)

for metric, title in (("full", "Full build"), ("edit", "Single-slide edit")):
    print(f"\n### {title}, ms - the fastest run of each\n")
    print("| Version | " + " | ".join(decks) + " |")
    print("|---" * (1 + len(decks)) + "|")
    for tag in order:
        cells = [f"{best[(tag, d, metric)]:.1f}" if (tag, d, metric) in best else "-"
                 for d in decks]
        print(f"| {tag} | " + " | ".join(cells) + " |")
PY
