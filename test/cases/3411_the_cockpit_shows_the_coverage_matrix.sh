# majordomus-covers: none
# majordomus-timeout: 600
# The Cockpit shows the coverage matrix (I1011 slice 2), asked of the real server in a
# repository `init` wrote:
#
#   1. /cockpit/coverage answers with the page: a row per executable capability, a cell per
#      column, and statistics that name health.coverage as where they were read
#   2. it is an area: the navigation of every Cockpit page links it
#   3. it is the capability's answer: the page's gap count is the one GET
#      /api/v1/health/coverage gives
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3411.XXXXXX")"; trap 'serve_down; rm -rf "$S"' EXIT
# shellcheck disable=SC2034  # RB is read by serve_up in test/lib.sh, which starts the server
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

serve_up "$S/serve.out" "$S/serve.err" || exit 1
code="$(curl -s --max-time 60 -o "$S/page.html" -w '%{http_code}' "$U/cockpit/coverage")"
[ "$code" = 200 ] || { echo "    GET /cockpit/coverage answered $code"; exit 1; }
curl -fsS --max-time 60 "$U/api/v1/health/coverage" > "$S/matrix.json" || { echo "    GET /api/v1/health/coverage failed"; exit 1; }
curl -fsS --max-time 60 "$U/cockpit" > "$S/overview.html" || { echo "    GET /cockpit failed"; exit 1; }
serve_down

# --- 1. the page
for want in '<h1[^>]*>Coverage' 'data-capability="health.coverage"' 'Every executable capability' '>Gaps<'; do
  grep -qE "$want" "$S/page.html" || { echo "    the page does not carry $want"; exit 1; }
done
rows="$(jq '.rows | length' "$S/matrix.json")"
[ "$rows" -gt 0 ] || { echo "    the matrix holds no row"; exit 1; }
for id in $(jq -r '.rows[:3][].capability' "$S/matrix.json"); do
  grep -qF "href=\"/cockpit/capabilities/$id\"" "$S/page.html" || { echo "    the page has no row for $id"; exit 1; }
done
echo "    ok: /cockpit/coverage draws the matrix of $rows capabilities and says what it asked"

# --- 2. an area of the navigation
grep -qF 'href="/cockpit/coverage"' "$S/overview.html" || { echo "    the Cockpit's navigation does not link the coverage page"; exit 1; }
echo "    ok: the navigation links it"

# --- 3. the capability's own answer
gaps="$(jq '.gaps' "$S/matrix.json")"
grep -qE "mj-stat-value\">$gaps</span><span class=\"mj-stat-label\">gaps<" "$S/page.html" \
  || { echo "    the page does not say the $gaps gap(s) the capability answers"; exit 1; }
echo "    ok: the page's gaps are the capability's ($gaps)"
