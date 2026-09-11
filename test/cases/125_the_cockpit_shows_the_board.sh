# majordomus-covers: none
# The Cockpit shows the board: the sessions attached to this checkout's shared server, what
# each of them announced, where two of them have claimed the same ground, and where this
# checkout's server stands.
#
# All of that state existed and was served — /api/v1/peers, /api/v1/server, majordomus_peers,
# `serve status` — and none of it reached a page. The person running several sessions in one
# repository had no way to look at the fleet, so two of them built the same subsystem in one
# afternoon because neither looked first. This case holds the page that closes that: a real
# server, a real client attached over MCP at /mcp with the frames a client sends, a real
# announcement made with the tool a client calls, and the page fetched over HTTP.
#
# The claim is end to end and nothing here is planted: what the page shows arrives on it
# because a client said it to the server, not because a fixture wrote it down.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || { echo "    curl absent; skipping"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?

"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib site
echo a > lib/a
echo s > site/s
git add -A >/dev/null
git commit -qm base
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# every file this case writes lands outside the repository, so that the last assertion —
# that reading the board wrote nothing — is about the tool and not about the case
S="$(mktemp -d "${TMPDIR:-/tmp}/mj125.XXXXXX")"
LOG="$S/serve.log"
"$RB" serve --repo "$T" --host 127.0.0.1 --port 0 > "$LOG" 2>&1 < /dev/null &
srv=$!
trap 'kill "$srv" 2>/dev/null || true; rm -rf "$S"' EXIT
BASE=""
i=0
while [ "$i" -lt 120 ]; do
  BASE="$(sed -n 's/.*listening on \(http:\/\/[0-9.]*:[0-9]*\).*/\1/p' "$LOG" | head -1)"
  [ -z "$BASE" ] || break
  kill -0 "$srv" 2>/dev/null || { echo "    the server exited before it listened"; cat "$LOG"; exit 1; }
  i=$((i+1)); sleep 0.25
done
[ -n "$BASE" ] || { echo "    the server never reported an address"; cat "$LOG"; exit 1; }

# --- the page is there, and it is reached from the navigation rather than by knowing its URL
curl -fsS "$BASE/cockpit" > "$S/overview.html" || { echo "    the Cockpit does not answer"; exit 1; }
grep -q 'href="/cockpit/board"' "$S/overview.html" || { echo "    the board is not linked from the Cockpit's navigation"; exit 1; }
curl -fsS "$BASE/cockpit/board" > "$S/board0.html" || { echo "    /cockpit/board does not answer"; exit 1; }
grep -q '</html>' "$S/board0.html" || { echo "    the board is not a complete page"; exit 1; }

# --- a board nobody is on says so, and says there is no collision on it
grep -q 'Nothing is attached' "$S/board0.html" || { echo "    an empty board does not say it is empty"; exit 1; }
grep -q 'No two sessions on this board have claimed the same ground' "$S/board0.html" || { echo "    an empty board does not state that there is no collision"; exit 1; }

# --- a client attaches the way a client attaches: initialize over MCP at /mcp
attach() {
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"%s","version":"%s"}}}' "$1" "$2" > "$S/init.json"
  curl -fsS -D "$S/head.txt" -o "$S/init.out" -X POST -H 'Content-Type: application/json' --data-binary "@$S/init.json" "$BASE/mcp" \
    || { echo "    initialize was refused"; return 1; }
  sed -n 's/^[Mm][Cc][Pp]-[Ss]ession-[Ii][Dd]: *//p' "$S/head.txt" | tr -d '\r'
}
announce() {
  session="$1"; intent="$2"; claim="$3"; name="${4:-}"
  if [ -n "$name" ]; then
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_announce","arguments":{"claim":"%s","intent":"%s","scope":["%s"]}}}' "$name" "$intent" "$claim" > "$S/ann.json"
  else
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_announce","arguments":{"intent":"%s","scope":["%s"]}}}' "$intent" "$claim" > "$S/ann.json"
  fi
  curl -fsS -o "$S/ann.out" -X POST -H 'Content-Type: application/json' -H "Mcp-Session-Id: $session" --data-binary "@$S/ann.json" "$BASE/mcp" \
    || { echo "    the announcement was refused"; return 1; }
  grep -q '"error"' "$S/ann.out" && { echo "    the announcement answered an error"; cat "$S/ann.out"; return 1; }
  return 0
}

s1="$(attach claude-code 2.1.268)"
[ -n "$s1" ] || { echo "    initialize answered no session id"; cat "$S/head.txt"; exit 1; }
announce "$s1" "the whole crate" "apps/majordomus-cli" || exit 1

curl -fsS "$BASE/cockpit/board" > "$S/board1.html" || { echo "    the board stopped answering"; exit 1; }
# the client is named by the name it gave in its own initialize, by the id the board gave it,
# and by what it said it is working on
grep -q 'claude-code' "$S/board1.html" || { echo "    the board does not name the client"; exit 1; }
grep -q '>p1<' "$S/board1.html" || { echo "    the board does not name the peer id"; exit 1; }
grep -q 'the whole crate' "$S/board1.html" || { echo "    the announcement did not reach the page"; exit 1; }
grep -q 'apps/majordomus-cli' "$S/board1.html" || { echo "    the claimed scope did not reach the page"; exit 1; }
grep -q 'Nothing is attached' "$S/board1.html" && { echo "    the board still calls itself empty with a session on it"; exit 1; }

# --- a second client that claims ground inside the first one's is a collision, and the page
#     raises it rather than filing it as one row among many
s2="$(attach codex 0.31)"
[ -n "$s2" ] || { echo "    the second client got no session id"; exit 1; }
announce "$s2" "one page of the cockpit" "apps/majordomus-cli/src/cockpit/pages.rs" || exit 1

curl -fsS "$BASE/cockpit/board" > "$S/board2.html" || { echo "    the board stopped answering"; exit 1; }
grep -q 'p2 codex and p1 claude-code' "$S/board2.html" || { echo "    the collision does not name both sessions"; exit 1; }
grep -q 'claim(s) meet' "$S/board2.html" || { echo "    the collision is not stated in words"; exit 1; }
grep -q 'mj-alert--fail' "$S/board2.html" || { echo "    the collision is not raised as an alert"; exit 1; }
grep -q 'apps/majordomus-cli/src/cockpit/pages.rs' "$S/board2.html" || { echo "    the meeting claim is not shown"; exit 1; }
grep -q 'No two sessions on this board have claimed the same ground' "$S/board2.html" && { echo "    the page claims there is no collision while showing one"; exit 1; }
# and the page agrees with the capability it is a projection of
curl -fsS "$BASE/api/v1/peers" > "$S/peers.json" || { echo "    the peers route does not answer"; exit 1; }
grep -q '"overlaps"' "$S/peers.json" || { echo "    the capability reports no overlap for two claims that meet"; cat "$S/peers.json"; exit 1; }

# --- one session, several workers: a client that fans work out shares its MCP session with
#     all of them, so one peer holds one named claim per worker. Before named claims each
#     announcement replaced the last and the board described whichever worker spoke most
#     recently; a page that rendered only the newest would put that defect straight back.
announce "$s1" "repository entry for every provider" "share/providers.yaml" "worker-b" || exit 1
curl -fsS "$BASE/cockpit/board" > "$S/board_fleet.html" || { echo "    the board stopped answering"; exit 1; }
grep -q 'worker-b' "$S/board_fleet.html" || { echo "    a named claim is not named on the page"; exit 1; }
grep -q 'repository entry for every provider' "$S/board_fleet.html" || { echo "    the second claim did not reach the page"; exit 1; }
grep -q 'share/providers.yaml' "$S/board_fleet.html" || { echo "    the ground the second claim took is not shown"; exit 1; }
grep -q 'the whole crate' "$S/board_fleet.html" || { echo "    the first claim was erased by the second"; exit 1; }
grep -q 'apps/majordomus-cli' "$S/board_fleet.html" || { echo "    the ground the first claim took was forgotten"; exit 1; }
# one connection is still one session, however many claims it holds
grep -q '>p3<' "$S/board_fleet.html" && { echo "    a second claim became a second peer"; exit 1; }

# --- where this checkout's server stands, from the lease this very process holds
grep -q 'This checkout.\{1,6\}s server' "$S/board2.html" || { echo "    the board does not show this checkout's server"; exit 1; }
grep -q 'The lease this process holds' "$S/board2.html" || { echo "    the answering process does not name its own lease"; exit 1; }
grep -q '>ready<' "$S/board2.html" || { echo "    a server serving its own Cockpit does not call itself ready"; exit 1; }
grep -qF "$BASE" "$S/board2.html" || { echo "    the address this request arrived on is not on the page"; exit 1; }
grep -q 'Every checkout of this repository' "$S/board2.html" || { echo "    the wide answer server.status gives is not on the page"; exit 1; }

# --- a session that ends leaves the board with what it said: the work outlives the connection
curl -fsS -o /dev/null -X DELETE -H "Mcp-Session-Id: $s2" "$BASE/mcp" || { echo "    the session would not end"; exit 1; }
curl -fsS "$BASE/cockpit/board" > "$S/board3.html" || { echo "    the board stopped answering"; exit 1; }
grep -q 'one page of the cockpit' "$S/board3.html" || { echo "    what a departed session announced was forgotten"; exit 1; }
grep -q '>gone<' "$S/board3.html" || { echo "    a departed session is not shown as departed"; exit 1; }

# --- and none of it wrote to the repository
curl -fsS -o /dev/null -X DELETE -H "Mcp-Session-Id: $s1" "$BASE/mcp" || true
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    reading the board changed the repository"; git status --porcelain; exit 1; }

echo "    a session that announces reaches a page a person can open, and two that collide are told so on it"
