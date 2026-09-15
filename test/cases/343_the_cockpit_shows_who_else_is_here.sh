# majordomus-covers: none
# The board a person can see, and the difference between nobody and nobody-answered.
#
# `peers.list` gathers every checkout's board and says, in `complete`, whether it managed to
# read them all — the capability was written that way because a listing that silently drops
# the checkouts it could not reach makes a reader conclude nobody else is working here. The
# answer reached MCP and HTTP, and no page: the Cockpit had fifteen areas and none of them was
# this one, so "is anyone else on this?" stayed a question you asked other sessions.
#
# What is decided here is that the page and the API answer the same question the same way, and
# that the one distinction the capability exists for survives the projection: where the API
# says a board could not be read, the page says so too. A page that rendered an unreachable
# checkout as silence would be the original defect with a nicer font.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "    skip: curl not installed"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj343.XXXXXX")"; trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer >/dev/null

R="$PWD"
trap '"$RB" serve stop --repo "$R" >/dev/null 2>&1; rm -rf "$S"' EXIT
"$RB" serve ensure --port 0 --idle 60 --wait 40 --format json > "$S/ensure.json" 2>"$S/ensure.err" \
  || { echo "    the server did not come up:"; tail -3 "$S/ensure.err" | sed 's/^/      /'; exit 1; }
url="$(jq -r '.url // .server.url // empty' "$S/ensure.json")"
[ -n "$url" ] || { echo "    the server reported no url"; cat "$S/ensure.json"; exit 1; }

curl -fsS "$url/api/v1/peers" > "$S/api.json" || { echo "    GET /api/v1/peers failed"; exit 1; }
curl -fsS -H 'Accept: text/html' "$url/cockpit/peers" > "$S/page.html" \
  || { echo "    the Peers page did not render"; exit 1; }

# ---------------------------------------------------------------- 1. the area exists at all
grep -q 'Peers' "$S/page.html" || { echo "    the page does not name itself"; exit 1; }
curl -fsS -H 'Accept: text/html' "$url/cockpit" > "$S/index.html" || { echo "    /cockpit did not render"; exit 1; }
grep -q '/cockpit/peers' "$S/index.html" \
  || { echo "    the Cockpit's own navigation does not offer the Peers area"; exit 1; }
echo "    the Cockpit has a peers area, and its navigation leads there"

# ---------------------------------------------------------------- 2. the page and the API agree
# A peer is an attached MCP session; this case asks over HTTP and is therefore not one. So
# what is decided is agreement, whatever the number happens to be — including zero, which is
# the reading most likely to be rendered as silence.
count="$(jq -r '.count' "$S/api.json")"
boards="$(jq -r '.boards | length' "$S/api.json")"
grep -q ">$count<" "$S/page.html" \
  || { echo "    the page does not show the peer count the API reports ($count)"; exit 1; }
grep -q ">$boards<" "$S/page.html" \
  || { echo "    the page does not show the checkout count the API reports ($boards)"; exit 1; }
for id in $(jq -r '.peers[].id' "$S/api.json"); do
  grep -qF "$id" "$S/page.html" || { echo "    peer $id is on the board and not on the page"; exit 1; }
done
if [ "$count" = 0 ]; then
  grep -qE 'holds a worker|not the whole repository' "$S/page.html" \
    || { echo "    the board holds nobody and the page says nothing about it"; exit 1; }
  echo "    a board with nobody on it says so in words, rather than rendering an empty table"
else
  echo "    every worker the API lists is on the page, with the same counts"
fi

# ---------------------------------------------------------------- 3. absent is not empty
# The distinction the capability exists for. Whatever this machine's checkouts happen to be,
# the page must not disagree with the answer about whether the board is whole.
complete="$(jq -r '.complete' "$S/api.json")"
if [ "$complete" = false ]; then
  grep -q 'could not be asked' "$S/page.html" \
    || { echo "    the API says a board was not read and the page renders that as silence"; exit 1; }
  for w in $(jq -r '.boards[] | select(.reason != null) | .worktree' "$S/api.json"); do
    grep -qF "$w" "$S/page.html" || { echo "    the unread checkout $w is not named on the page"; exit 1; }
  done
  echo "    a board that could not be read is named on the page, with the reason"
else
  grep -q 'every checkout answered' "$S/page.html" \
    || { echo "    the board is whole and the page does not say so"; exit 1; }
  echo "    the board is whole, and the page says that rather than leaving it to be assumed"
fi

# ---------------------------------------------------------------- 4. an overlap is a warning
# Not planted here: two peers with meeting scopes need two attached sessions, which a case
# cannot honestly fake with one. What is decided is that the page renders the pairs the answer
# carries, whatever their number, and says what a claim is and is not.
ov="$(jq -r '.overlaps | length' "$S/api.json")"
if [ "$ov" -gt 0 ]; then
  grep -q 'Two workers, one scope' "$S/page.html" \
    || { echo "    the answer carries $ov overlapping claim(s) and the page does not show them"; exit 1; }
  echo "    the overlapping claims the answer carries are on the page"
else
  grep -q 'Two workers, one scope' "$S/page.html" \
    && { echo "    the page shows an overlap section with no overlap to show"; exit 1; }
  echo "    no overlap in the answer and none invented on the page"
fi

echo "    the Cockpit shows who else is here, and says when it could not ask"
