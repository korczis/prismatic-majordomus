# majordomus-covers: none
# The board, whole (I2657), through a real server over a fixture `majordomus init` installs:
#
#   GET /api/v1/peers/board   the derived answer: workers, claims, paths, collisions, the
#                             mesh's exclusive claims, the checkouts covered and the counts;
#                             every count is the length of a list in the same answer
#   /ui/board                 the screen over it: every panel, the source named, no control,
#                             and in the navigation of every screen
#
# A fixture has no attached worker, so the lists are empty here: what is held is the shape,
# the arithmetic of the counts and that an empty board is said in words. The derivation over
# a board with workers, claims and collisions is the unit tests' (capability::builtin::peers).
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the answer was not read"

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 120 -o "$2" -w '%{http_code}' "$U$1"; }

# ---------------------------------------------------------------- the answer
[ "$(get '/api/v1/peers/board?checkouts=this' board.json)" = 200 ] || { echo "    /api/v1/peers/board did not answer 200"; exit 1; }
jq -e '.complete == true' board.json >/dev/null || { echo "    one checkout, read from memory, is not a whole board"; exit 1; }
for key in counts workers claims paths checkouts mesh; do
  jq -e --arg k "$key" 'has($k)' board.json >/dev/null || { echo "    the answer carries no $key"; exit 1; }
done
jq -e '.counts.workers == (.workers | length)
   and .counts.claims == (.claims | length)
   and .counts.paths == (.paths | length)
   and .counts.collisions == ((.collisions // []) | length)
   and .counts.exclusive == ((.exclusive // []) | length)
   and .counts.checkouts == (.checkouts | length)
   and .counts.attached + .counts.departed == .counts.workers
   and .counts.unread == ([.checkouts[] | select(.reason != null)] | length)' board.json >/dev/null \
  || { echo "    a count is not the length of its list:"; jq -c .counts board.json; exit 1; }
jq -e '.checkouts | length == 1 and .[0].this_checkout == true' board.json >/dev/null \
  || { echo "    'checkouts=this' covered more or less than the one checkout asked"; exit 1; }
# asked twice, the same board is the same bytes
[ "$(get '/api/v1/peers/board?checkouts=this' again.json)" = 200 ] || { echo "    the second read did not answer"; exit 1; }
jq -S 'del(.workers[]?.last_seen_seconds_ago)' board.json > a.json
jq -S 'del(.workers[]?.last_seen_seconds_ago)' again.json > b.json
cmp -s a.json b.json || { echo "    two reads of an unchanged board differ"; exit 1; }

# ---------------------------------------------------------------- the screen
[ "$(get /ui/board page.html)" = 200 ] || { echo "    /ui/board did not answer 200"; exit 1; }
for want in 'Who is working here' 'peers.board' 'Who holds what' 'Collisions' 'Claims' \
  'Workers' 'Paths' 'Held exclusively' 'Checkouts' 'Nobody has announced anything.'; do
  grep -qF -- "$want" page.html || { echo "    /ui/board does not show: $want"; exit 1; }
done
# the page is one line of markup, so the main part is cut by its tags, not by lines
awk 'BEGIN { RS = "</main>" } NR == 1 { i = index($0, "<main"); if (i) print substr($0, i) }' page.html > main.html
[ -s main.html ] || { echo "    /ui/board has no main part"; exit 1; }
if grep -qE '<form|<button|method="post"' main.html; then
  echo "    the Board screen carries a control; it reads and never executes"; exit 1
fi
[ "$(get /ui dash.html)" = 200 ] || { echo "    /ui did not answer 200"; exit 1; }
grep -qF 'href="/ui/board"' dash.html || { echo "    the navigation does not lead to /ui/board"; exit 1; }

echo "    the board answers whole with counts that are its lists' lengths, twice the same; the screen draws it, offers no control, and is in the navigation"
