# majordomus-covers: none
# The PR Drain screen of majordomus ui (I2470), through a real server over a real fixture:
#
#   the screen          paints at once and defers the drain to its own panel route, carrying
#                       the reader's choices in that route's query
#   a hostile value     a disposition that is not the queue's word is dropped, never echoed
#   no forge observed   the drain says so and shows no tab and no number
#   an issue            ?issue= opens it from the plan, with what it depends on and what it
#                       unblocks as links; an id the plan does not hold says so
#   the navigation      every screen leads to the drain
#
# The unit tests in apps/majordomus-cli/src/ui/prs.rs own the drain over a queue answer (tabs,
# search, inspector, blockers, graph); this case owns the seam they cannot reach: the routes a
# browser asks, answered by the executable a person runs.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
pj_init
pj_milestone M1
pj_issue I0001 M1
pj_issue I0002 M1 I0001
git add -A >/dev/null && git commit -qm "a plan of two issues"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- the screen
page="$(get '/ui/prs?disposition=conflicting&q=a%20b&pr=7')"
printf '%s' "$page" | grep -q '<h1[^>]*>PR Drain</h1>' \
  || { echo "    /ui/prs has no PR Drain hero:"; printf '%s\n' "$page" | head -c 600 | sed 's/^/    | /'; exit 1; }
printf '%s' "$page" | grep -qF 'data-panel-src="/ui/panels/drain?disposition=conflicting&amp;q=a%20b&amp;pr=7"' \
  || { echo "    the drain panel does not carry the reader's choices:"; printf '%s' "$page" | grep -o 'data-panel-src="[^"]*"' | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- a hostile value
page="$(get '/ui/prs?disposition=x%22%20onclick%3D%22alert(1)')"
printf '%s' "$page" | grep -qF 'data-panel-src="/ui/panels/drain"' \
  || { echo "    a disposition that is not the queue's word reached the page:"; printf '%s' "$page" | grep -o 'data-panel-src="[^"]*"' | sed 's/^/    | /'; exit 1; }
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

# ---------------------------------------------------------------- no forge observed
drain="$(get '/ui/panels/drain?fragment')"
printf '%s' "$drain" | grep -q 'The forge was not observed in this checkout' \
  || { echo "    the drain does not say the forge was not observed:"; printf '%s\n' "$drain" | head -c 600 | sed 's/^/    | /'; exit 1; }
printf '%s' "$drain" | grep -q 'mj-ui-tabs' && { echo "    an unobserved forge drew tabs"; exit 1; }
printf '%s' "$drain" | grep -q '<html' && { echo "    a fragment came back framed"; exit 1; }

# ---------------------------------------------------------------- an issue
page="$(get '/ui/prs?issue=I0002')"
printf '%s' "$page" | grep -q 'id="issue"' \
  || { echo "    ?issue=I0002 opened no issue:"; printf '%s\n' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; exit 1; }
printf '%s' "$page" | grep -qF 'href="/ui/prs?issue=I0001"' \
  || { echo "    the issue's dependency is not a link to it"; exit 1; }
page="$(get '/ui/prs?issue=I0001')"
printf '%s' "$page" | grep -qF 'href="/ui/prs?issue=I0002"' \
  || { echo "    the issue does not lead to the issue it unblocks"; exit 1; }
page="$(get '/ui/prs?issue=I9999')"
printf '%s' "$page" | grep -q 'I9999 is not an issue of the plan' \
  || { echo "    an id the plan does not hold is not said to be unknown"; exit 1; }

# ---------------------------------------------------------------- the navigation
page="$(get '/ui')"
printf '%s' "$page" | grep -qF 'href="/ui/prs"' \
  || { echo "    the navigation does not lead to the drain"; exit 1; }

echo "    the drain paints, carries the reader's choices, drops what is not a word, and opens an issue"
