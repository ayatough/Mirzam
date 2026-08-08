#!/usr/bin/env bash
# Downloads photographs from Unsplash for use as pane backgrounds.
#
#   export UNSPLASH_ACCESS_KEY=...        # https://unsplash.com/oauth/applications
#   ./scripts/fetch-backgrounds.sh mountains "city at night" laboratory
#
# Files land in assets/bg/ (override with BG_DIR) and every photo is recorded in
# assets/bg/CREDITS.md. Unsplash requires that attribution, so keep the file with
# the images and credit the photographer wherever the deck is published.
#
# The images checked into examples/media/bg/ are drawn, not downloaded, so the
# repository stays self-contained; see scripts/make-sample-backgrounds.py.
set -euo pipefail

BG_DIR="${BG_DIR:-assets/bg}"
WIDTH="${BG_WIDTH:-2000}"

if [[ -z "${UNSPLASH_ACCESS_KEY:-}" ]]; then
  echo "error: set UNSPLASH_ACCESS_KEY (register an app at https://unsplash.com/oauth/applications)" >&2
  exit 1
fi
if [[ $# -eq 0 ]]; then
  echo "usage: $0 <query> [query...]" >&2
  exit 1
fi

mkdir -p "$BG_DIR"
CREDITS="$BG_DIR/CREDITS.md"
[[ -f "$CREDITS" ]] || printf '# Photo credits\n\nAll photos from [Unsplash](https://unsplash.com).\n\n' > "$CREDITS"

api() { curl -fsSL -H "Authorization: Client-ID $UNSPLASH_ACCESS_KEY" "$@"; }

for query in "$@"; do
  slug=$(printf '%s' "$query" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')
  echo "==> $query"

  escaped=$(printf '%s' "$query" | sed 's/ /%20/g')
  json=$(api "https://api.unsplash.com/photos/random?orientation=landscape&query=$escaped")

  # Tab separated, because photographer names contain spaces.
  IFS=$'\t' read -r url dl_loc author author_url photo_url < <(
    printf '%s' "$json" | python3 -c '
import json, sys
p = json.load(sys.stdin)
print("\t".join([p["urls"]["raw"], p["links"]["download_location"],
                 p["user"]["name"], p["user"]["links"]["html"],
                 p["links"]["html"]]))
')

  curl -fsSL -o "$BG_DIR/$slug.jpg" "$url&w=$WIDTH&fm=jpg&q=80"
  # Required by the Unsplash API guidelines: report the download.
  api "$dl_loc" > /dev/null

  printf -- '- `%s.jpg` - [%s](%s) on [Unsplash](%s)\n' \
    "$slug" "$author" "$author_url" "$photo_url" >> "$CREDITS"
  echo "    $BG_DIR/$slug.jpg"
done

echo "credits appended to $CREDITS"
echo "use it with:  ::: pane hero {bg=$BG_DIR/<name>.jpg dim=0.45}"
