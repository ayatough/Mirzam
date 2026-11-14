#!/usr/bin/env bash
# Before and after, for a pull request description.
#
#   ./scripts/pr-evidence.sh <deck.md>                 every slide that changed
#   ./scripts/pr-evidence.sh <deck.md> --slide 2,5     those slides, changed or not
#   ./scripts/pr-evidence.sh <deck.md> --publish       and host the pictures
#   ./scripts/pr-evidence.sh <deck.md> --pdf           the exported PDF's pages
#
#   --base <ref>     what "before" is; default: where this branch left origin/main
#   --pdf            photograph `export pdf`, not the viewer
#   --mode dark      build and photograph both sides in that mode
#   --out <dir>      where the pictures and evidence.md go; default: out/evidence
#   --branch <name>  the directory the pictures are published under
#   --repo <o/r>     the repository the published links name
#   -- <flags>       anything after `--` goes to `mirzam build` and `check`
#
# The reviewer of a pull request here reads it on a phone and merges from the
# description alone, so the description has to *show* the change: the same deck
# rendered by the renderer as it was and by the renderer as it is now. This
# builds both renderers, renders the deck with each, photographs every slide,
# and writes the Markdown that sets the two side by side - plus what
# `mirzam check` said about the deck on each side, because "the heading is no
# longer clipped" is a claim and the checker's two verdicts are the evidence.
#
# The deck is always the one in the working tree. A fix's reproduction deck did
# not exist before the fix, and that is fine: the question a "before" answers
# is what the old renderer does with this source, not what the old source was.
#
# --pdf is for a change the viewer does not show: something the exported file
# gets wrong while the deck on screen is right. It exports the deck to PDF on
# each side and rasterises the pages, so what is compared is the file a reader
# is handed. The first pull request this loop ever opened fixed arrows missing
# from the PDF, and could show nothing, because the viewer had drawn them all
# along. Needs `pdftoppm` (poppler-utils).
#
# Without --slide it shows the slides whose pixels differ and counts the rest,
# so "nothing else moved" is on the page too rather than taken on trust.
#
# --publish pushes the pictures to the `pr-assets` branch, under
# `<branch>/<commit>/`, and writes links to them. GitHub's API takes no
# attachment upload, so a picture in a description has to be a file somewhere;
# that branch is the somewhere. It holds nothing else, and
# `.github/workflows/pr-assets.yml` removes a branch's pictures when its pull
# request closes. The commit is part of the path because raw file links are
# cached for minutes: a second round of pictures at the same address would show
# the first round.
#
# Needs a browser and `playwright-core`, exactly as `shoot-slides.mjs` does.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$PWD

DECK=""
SLIDES=""
BASE=""
MODE=""
PDF=0
OUT="out/evidence"
PUBLISH=0
BRANCH="${GITHUB_HEAD_REF:-}"
REPO="${GITHUB_REPOSITORY:-}"
EXTRA=()

while [ $# -gt 0 ]; do
  case "$1" in
    --slide) SLIDES="$2"; shift 2 ;;
    --base) BASE="$2"; shift 2 ;;
    --mode) MODE="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --branch) BRANCH="$2"; shift 2 ;;
    --repo) REPO="$2"; shift 2 ;;
    --publish) PUBLISH=1; shift ;;
    --pdf) PDF=1; shift ;;
    --) shift; EXTRA=("$@"); break ;;
    -h|--help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "error: unknown flag $1" >&2; exit 2 ;;
    *)
      if [ -n "$DECK" ]; then echo "error: one deck at a time, got $DECK and $1" >&2; exit 2; fi
      DECK="$1"; shift ;;
  esac
done

if [ -z "$DECK" ]; then
  echo "usage: ./scripts/pr-evidence.sh <deck.md> [--slide 2,5] [--pdf] [--base <ref>] [--mode light|dark]" >&2
  echo "       [--out <dir>] [--publish] [--branch <name>] [--repo <owner/repo>] [-- <build flags>]" >&2
  exit 2
fi
if [ ! -f "$DECK" ]; then echo "error: $DECK not found" >&2; exit 2; fi

if [ -z "$BASE" ]; then
  git fetch -q origin main 2>/dev/null || true
  BASE=$(git merge-base HEAD origin/main)
fi
BASE_SHA=$(git rev-parse --verify "$BASE^{commit}")
HEAD_SHA=$(git rev-parse HEAD)
SHORT=${HEAD_SHA:0:7}

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "note: uncommitted changes are in the \"after\" pictures, but they are filed under $SHORT" >&2
fi

FLAGS=()
if [ -n "$MODE" ]; then FLAGS+=(--mode "$MODE"); fi
FLAGS+=("${EXTRA[@]}")

if ! node -e 'import("playwright-core").catch(() => process.exit(1))' 2>/dev/null; then
  echo "error: playwright-core is not installed; it is not a repository dependency." >&2
  echo "       npm i playwright-core   (node_modules is ignored; delete package*.json after)" >&2
  exit 2
fi

if [ "$PDF" -eq 1 ] && ! command -v pdftoppm >/dev/null; then
  echo "error: --pdf rasterises the pages with pdftoppm; install poppler-utils" >&2
  exit 2
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# --- the two renderers -------------------------------------------------------

echo "==> after: the working tree ($SHORT)" >&2
cargo build --quiet --bin mirzam
AFTER_BIN="$ROOT/target/debug/mirzam"

echo "==> before: ${BASE_SHA:0:7}" >&2
# A worktree in a fixed place under target/, with its own target directory,
# both kept between runs: cargo keys its cache on the source path and on file
# times, so the second "before" build of a branch is then seconds, where a
# fresh checkout is the whole workspace.
BASE_SRC="$ROOT/target/evidence-base-src"
if git -C "$BASE_SRC" rev-parse --git-dir >/dev/null 2>&1; then
  git -C "$BASE_SRC" checkout --quiet --force --detach "$BASE_SHA"
else
  rm -rf "$BASE_SRC"
  git worktree prune
  git worktree add --quiet --detach "$BASE_SRC" "$BASE_SHA"
fi
(cd "$BASE_SRC" && CARGO_TARGET_DIR="$ROOT/target/evidence-base" cargo build --quiet --bin mirzam)
BEFORE_BIN="$ROOT/target/evidence-base/debug/mirzam"

# --- render, check and photograph each side ----------------------------------

rm -rf "$OUT"
mkdir -p "$OUT"

# Sets <side>_built to 1 when the deck built, and leaves <side>.check.json and
# <side>-NN.png in $OUT.
render() { # side, binary
  local side=$1 bin=$2
  local shoot=(node scripts/shoot-slides.mjs "$work/$side/index.html" -o "$OUT" --name "$side"
               --width 1280 --height 720)
  if [ -n "$MODE" ]; then shoot+=(--mode "$MODE"); fi
  if ! "$bin" build "$DECK" -o "$work/$side" "${FLAGS[@]}" >"$work/$side.build.log" 2>&1; then
    echo "    $side: the deck does not build" >&2
    return 1
  fi
  # A check that finds problems exits non-zero, and that is a result, not a
  # failure of this script.
  "$bin" check "$DECK" --format json "${FLAGS[@]}" >"$OUT/$side.check.json" 2>/dev/null || true
  # Spelled out, because `render ... || x=0` below switches `set -e` off for
  # the whole function: a browser that would not start must still stop the run
  # rather than produce a page that says no slide differs.
  if [ "$PDF" -eq 1 ]; then
    if ! "$bin" export pdf "$DECK" -o "$work/$side.pdf" "${FLAGS[@]}" >"$work/$side.export.log" 2>&1; then
      cat "$work/$side.export.log" >&2
      echo "error: could not export the $side deck to PDF" >&2
      exit 1
    fi
    # One page is one slide, so the pages are numbered as the slides are.
    if ! pdftoppm -png -scale-to-x 1280 -scale-to-y -1 "$work/$side.pdf" "$OUT/$side"; then
      echo "error: could not rasterise the $side PDF" >&2
      exit 1
    fi
  elif ! "${shoot[@]}" >/dev/null; then
    echo "error: could not photograph the $side deck" >&2
    exit 1
  fi
  # Always numbered, so a one-slide deck is `before-1.png` beside `after-1.png`
  # and the two sides can be paired by name.
  if [ -f "$OUT/$side.png" ]; then mv "$OUT/$side.png" "$OUT/$side-1.png"; fi
}

before_built=1; render before "$BEFORE_BIN" || before_built=0
after_built=1; render after "$AFTER_BIN" || after_built=0
if [ "$after_built" -eq 0 ]; then
  cat "$work/after.build.log" >&2
  echo "error: the working tree cannot build $DECK, so there is no \"after\" to show" >&2
  exit 1
fi

# --- where the pictures will be read from ------------------------------------

if [ -z "$BRANCH" ]; then BRANCH=$(git branch --show-current); fi
if [ -z "$REPO" ]; then
  REPO=$(git remote get-url origin | sed -E 's#^.*[:/]([^/]+/[^/]+)$#\1#; s#\.git$##')
fi
DEST="$BRANCH/$SHORT"
if [ "$PUBLISH" -eq 1 ]; then
  URL="https://raw.githubusercontent.com/$REPO/pr-assets/$DEST"
else
  URL="."
fi

# --- the Markdown ------------------------------------------------------------

node - "$OUT" "$DECK" "$URL" "$SLIDES" "$before_built" "${BASE_SHA:0:7}" "$SHORT" "$PDF" <<'EOF' >"$OUT/evidence.md"
const { readdirSync, readFileSync, existsSync } = require("fs");
const { join } = require("path");
const [out, deck, url, only, beforeBuilt, baseSha, headSha, pdf] = process.argv.slice(2);
const what = pdf === "1" ? ", exported to PDF" : "";

const shots = (side) => {
  const m = new Map();
  for (const f of readdirSync(out)) {
    const hit = f.match(new RegExp(`^${side}-(\\d+)\\.png$`));
    if (hit) m.set(+hit[1], f);
  }
  return m;
};
const before = shots("before");
const after = shots("after");
const same = (n) =>
  before.has(n) && after.has(n) &&
  readFileSync(join(out, before.get(n))).equals(readFileSync(join(out, after.get(n))));

const all = [...new Set([...before.keys(), ...after.keys()])].sort((a, b) => a - b);
const wanted = only ? only.split(",").map((s) => +s.trim()) : all.filter((n) => !same(n));
const img = (side, n) => {
  const f = side.get(n);
  return f ? `<img src="${url}/${f}" width="420">` : "*no such slide*";
};

const check = (side) => {
  const file = join(out, `${side}.check.json`);
  if (!existsSync(file)) return null;
  try { return JSON.parse(readFileSync(file, "utf8")); } catch { return null; }
};
const verdict = (c) => {
  if (!c) return "did not run";
  const errors = c.diagnostics.filter((d) => d.severity === "error");
  const warnings = c.diagnostics.filter((d) => d.severity !== "error");
  const parts = [c.ok ? "ok" : "**problems**", `${c.slides} slide(s)`];
  if (errors.length) parts.push(`${errors.length} error(s)`);
  if (warnings.length) parts.push(`${warnings.length} warning(s)`);
  return parts.join(", ");
};
const list = (c) =>
  c.diagnostics
    .map((d) => {
      const where = [d.slide != null ? `slide ${d.slide}` : null, d.pane ? `pane \`${d.pane}\`` : null]
        .filter(Boolean)
        .join(", ");
      return `  - \`${d.kind}\`${where ? ` (${where})` : ""}: ${d.message}`;
    })
    .join("\n");

const lines = [];
lines.push(`### \`${deck}\`${what} — before \`${baseSha}\`, after \`${headSha}\``, "");

const cb = beforeBuilt === "1" ? check("before") : null;
const ca = check("after");
lines.push("**`mirzam check`**", "");
lines.push(`- before: ${beforeBuilt === "1" ? verdict(cb) : "the deck does not build"}`);
if (cb && cb.diagnostics.length) lines.push(list(cb));
lines.push(`- after: ${verdict(ca)}`);
if (ca && ca.diagnostics.length) lines.push(list(ca));
lines.push("");

if (wanted.length === 0) {
  lines.push(`No slide's pixels differ (${all.length} compared).`);
} else {
  lines.push("| | before | after |", "|---|---|---|");
  for (const n of wanted) {
    const note = same(n) ? "<br>*identical*" : "";
    lines.push(`| ${n}${note} | ${img(before, n)} | ${img(after, n)} |`);
  }
  if (!only) {
    const unchanged = all.length - wanted.length;
    lines.push("", `${wanted.length} of ${all.length} slide(s) differ; the other ${unchanged} are pixel-identical.`);
  }
}
console.log(lines.join("\n"));
EOF

# --- publish -----------------------------------------------------------------

if [ "$PUBLISH" -eq 1 ]; then
  if [ -z "$BRANCH" ]; then
    echo "error: --publish files the pictures under a branch name; pass --branch" >&2
    exit 2
  fi
  echo "==> publishing to pr-assets:$DEST" >&2
  # Built with plumbing in a scratch index, so nothing here touches the working
  # tree, the real index or the branch that is checked out.
  attempt=0
  until [ "$attempt" -ge 3 ]; do
    attempt=$((attempt + 1))
    export GIT_INDEX_FILE="$work/index.$attempt"
    parent=""
    if git fetch -q origin pr-assets 2>/dev/null; then
      parent=$(git rev-parse FETCH_HEAD)
      git read-tree "$parent"
    fi
    for f in "$OUT"/*.png; do
      blob=$(git hash-object -w "$f")
      git update-index --add --cacheinfo "100644,$blob,$DEST/$(basename "$f")"
    done
    tree=$(git write-tree)
    unset GIT_INDEX_FILE
    commit=$(git -c user.name="${GIT_AUTHOR_NAME:-mirzam-evidence}" \
                 -c user.email="${GIT_AUTHOR_EMAIL:-evidence@users.noreply.github.com}" \
                 commit-tree "$tree" ${parent:+-p "$parent"} -m "Evidence for $BRANCH at $SHORT")
    # Two pull requests publishing at once race for the branch tip; the loser
    # rebuilds on the winner's commit rather than overwriting it.
    if git push -q origin "$commit:refs/heads/pr-assets"; then break; fi
    if [ "$attempt" -ge 3 ]; then
      echo "error: could not push to pr-assets in three attempts" >&2
      exit 1
    fi
    sleep $((attempt * 2))
  done
fi

cat "$OUT/evidence.md"
echo >&2
echo "✓ $OUT/evidence.md — paste it into the pull request description" >&2
if [ "$PUBLISH" -eq 0 ]; then
  echo "  the pictures are local files; run again with --publish to host them" >&2
fi
