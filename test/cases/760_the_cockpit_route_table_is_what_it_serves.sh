# majordomus-covers: none
# majordomus-timeout: 600
# The Cockpit's documented route table is what it serves (#330, PR #543): over a real socket,
# every plain route docs/COCKPIT.md documents answers 200 with a page, and every area the
# sidebar links to is a route the table documents. The unit tests compare STATIC_ROUTES with
# the dispatcher's source, the navigation and the table; this case holds the running server
# to the same table, so a route the table names and the server lost, or an area the sidebar
# offers and the table never mentions, is a failure no list kept beside the code can hide.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj760.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

# the plain routes the table documents: no identifier placeholder, no query string, no assets
awk '/^## What is on it/{on=1; next} on && /^## /{exit} on' "$ROOT/docs/COCKPIT.md" \
  | grep -oE '^\| `/cockpit[^`]*`' | sed -E 's/^\| `//; s/`$//; s/\?.*$//' \
  | grep -v '<' | grep -v '^/cockpit/assets' | sort -u > "$S/documented.txt"
n="$(wc -l < "$S/documented.txt" | tr -d ' ')"
[ "$n" -gt 10 ] || {
  echo "    the route table was not found in docs/COCKPIT.md ($n routes)"; exit 1; }

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer
serve_up "$S/out.txt" "$S/err.txt" || exit 1

fails=0
while read -r route; do
  code="$(curl -s -m 60 -o "$S/page.html" -w '%{http_code}' "$U$route")"
  [ "$code" = 200 ] || {
    echo "    $route is documented and answers $code"; fails=$((fails + 1)); continue; }
  grep -q '<html' "$S/page.html" || { echo "    $route answered no page"; fails=$((fails + 1)); }
done < "$S/documented.txt"

# the sidebar's area links: one segment under /cockpit, nothing after it
curl -s -m 60 "$U/cockpit" > "$S/overview.html"
grep -oE 'href="/cockpit(/[a-z-]+)?"' "$S/overview.html" | sed -E 's/^href="//; s/"$//' \
  | sort -u > "$S/linked.txt"
[ -s "$S/linked.txt" ] || { echo "    the overview links to no area"; exit 1; }
while read -r href; do
  grep -qxF "$href" "$S/documented.txt" \
    || { echo "    the sidebar links to $href, which docs/COCKPIT.md does not document"
         fails=$((fails + 1)); }
done < "$S/linked.txt"

serve_down
[ "$fails" = 0 ] || { echo "    $fails route(s) disagree with the documented table"; exit 1; }
m="$(wc -l < "$S/linked.txt" | tr -d ' ')"
echo "    $n documented routes answer; $m sidebar links are documented"
