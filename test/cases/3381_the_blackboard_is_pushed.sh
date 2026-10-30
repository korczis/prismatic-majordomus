# majordomus-covers: none
# The blackboard is pushed over the one live channel (ADR 0176, I2981).
#
# A page that wants to show who works on what used to ask again on a timer. The board and the
# mesh journal now advance one generation when they change, and `/events?follow=blackboard`
# writes the change when it happens. What is decided here, on a running server with a declared
# mesh, a real WebSocket follower and a real MCP client:
#
#   1. a follower is told the server's epoch and given the blackboard at once;
#   2. a peer's announcement reaches it as a snapshot frame caused by the change, not by a
#      refresh, and a review request reaches it as a signal — both without a poll;
#   3. a follower that resumes from its cursor and epoch is told nothing twice;
#   4. a cursor of another process is answered with a resync and a fresh snapshot;
#   5. a page of another origin and a query the channel does not take are refused.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v python3 >/dev/null 2>&1 || skip "no python3"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_MONITOR_FOR

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3381\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
cd "$R" || exit

# ---------------------------------------------------------------- a follower
# Prints every text frame as one JSON line with the second it arrived, until killed or until
# the frame count it was given.
cat > "$T/follow.py" <<'PY'
import base64, json, os, socket, sys, time
base, target, limit = sys.argv[1], sys.argv[2], int(sys.argv[3])
origin = sys.argv[4] if len(sys.argv) > 4 else None
host, port = base.split("//")[1].split(":")
s = socket.create_connection((host, int(port)), timeout=60)
key = base64.b64encode(os.urandom(16)).decode()
extra = "Origin: %s\r\n" % origin if origin else ""
s.sendall(("GET %s HTTP/1.1\r\nHost: %s:%s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n%s"
           "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (target, host, port, extra, key)).encode())
head = b""
while b"\r\n\r\n" not in head:
    b = s.recv(1)
    if not b:
        break
    head += b
status = head.split(b"\r\n")[0].decode("utf-8", "replace")
print(json.dumps({"status": status}), flush=True)
if " 101 " not in status + " ":
    sys.exit(0)
buf = b""
def need(n):
    global buf
    while len(buf) < n:
        chunk = s.recv(65536)
        if not chunk:
            return False
        buf += chunk
    return True
seen = 0
while seen < limit:
    if not need(2):
        break
    op = buf[0] & 0x0f
    ln = buf[1] & 0x7f
    off = 2
    if ln == 126:
        need(4); ln = int.from_bytes(buf[2:4], "big"); off = 4
    elif ln == 127:
        need(10); ln = int.from_bytes(buf[2:10], "big"); off = 10
    if not need(off + ln):
        break
    payload, buf = buf[off:off+ln], buf[off+ln:]
    if op != 0x1:
        continue
    frame = json.loads(payload.decode())
    frame["received"] = time.time()
    print(json.dumps(frame), flush=True)
    seen += 1
PY
F="/events?follow=blackboard&checkouts=this"
wait_for() { # <file> <jq filter> <seconds> — true when a line of the file matches
  local n=0
  while [ "$n" -lt $(( $3 * 10 )) ]; do
    grep -v '"status"' "$1" 2>/dev/null | jq -e -s "map(select($2)) | length > 0" >/dev/null 2>&1 && return 0
    sleep 0.1; n=$((n + 1))
  done
  return 1
}

python3 "$T/follow.py" "$url" "$F" 1000 > "$T/live.jsonl" 2>"$T/live.err" &
FOLLOWER=$!; PIDS="$PIDS $FOLLOWER"

# ---------------------------------------------------------------- 1. ready, then the blackboard
wait_for "$T/live.jsonl" '.type == "blackboard.snapshot" and .data.trigger == "connect"' 60 \
  || { echo "    the follower was not given the blackboard:"; cat "$T/live.jsonl" "$T/live.err" | head -20; exit 1; }
head -1 "$T/live.jsonl" | jq -e '.status | test(" 101 ")' >/dev/null || { echo "    no upgrade"; head -1 "$T/live.jsonl"; exit 1; }
sed -n 2p "$T/live.jsonl" | jq -e '.type == "stream.ready" and .data.scope == "blackboard" and (.data.epoch | length > 0)' >/dev/null \
  || { echo "    the first frame is not stream.ready with the server's epoch"; sed -n 2p "$T/live.jsonl"; exit 1; }
EPOCH="$(sed -n 2p "$T/live.jsonl" | jq -r '.data.epoch')"
echo "    a follower is told the server's epoch and given the blackboard at once"

# ---------------------------------------------------------------- 2. pushed, not polled
H="$(mktemp)"
curl -fsS --max-time 10 -D "$H" -o /dev/null -X POST "$url/mcp" \
  -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case-pusher","version":"1"}}}' \
  || { echo "    the MCP client could not attach"; exit 1; }
SID="$(tr -d '\r' < "$H" | awk 'tolower($1)=="mcp-session-id:" {print $2}')"
before="$(python3 -c 'import time; print(time.time())')"
curl -fsS --max-time 30 -X POST "$url/mcp" -H "mcp-session-id: $SID" \
  -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_announce","arguments":{"intent":"pushed to the follower","scope":["docs"]}}}' >/dev/null \
  || { echo "    the MCP client could not announce"; exit 1; }
wait_for "$T/live.jsonl" '.type == "blackboard.snapshot" and .data.trigger == "change" and ([.data.blackboard.workers[]? | select(.intent == "pushed to the follower")] | length == 1)' 25 \
  || { echo "    the announcement was not pushed to the follower:"; jq -c '{type, trigger: .data.trigger}' "$T/live.jsonl" 2>/dev/null | tail -8; exit 1; }
took="$(grep -v '"status"' "$T/live.jsonl" | jq -s --argjson t0 "$before" \
  'map(select(.type == "blackboard.snapshot" and .data.trigger == "change" and ([.data.blackboard.workers[]? | select(.intent == "pushed to the follower")] | length == 1))) | .[0].received - $t0 | . * 1000 | floor')"
echo "    an announcement reached the follower as a change, in ${took} ms, without a poll"

"$RB" mesh session open --session s1 --worker 3d --intent "the reviewed work" >/dev/null 2>&1 \
  || { echo "    a mesh session could not be opened"; exit 1; }
"$RB" mesh review request "3381: read this before it lands" --session s1 >/dev/null 2>&1 \
  || { echo "    the review could not be requested"; exit 1; }
wait_for "$T/live.jsonl" '.type == "blackboard.signals" and ([.data.signals[] | select(.kind == "review_requested")] | length >= 1)' 25 \
  || { echo "    the review request did not reach the follower as a signal:"; jq -c '{type, trigger: .data.trigger}' "$T/live.jsonl" | tail -8; exit 1; }
echo "    a review request reached the follower as a signal"
CURSOR="$(grep -v '"status"' "$T/live.jsonl" | jq -rs 'map(select(.type == "blackboard.signals")) | last | .data.cursor')"
kill "$FOLLOWER" 2>/dev/null || true

# ---------------------------------------------------------------- 3. resumed, nothing twice
python3 "$T/follow.py" "$url" "$F&after=$CURSOR&epoch=$EPOCH" 3 > "$T/resumed.jsonl" 2>/dev/null &
P=$!; PIDS="$PIDS $P"
wait_for "$T/resumed.jsonl" '.type == "blackboard.snapshot"' 60 || { echo "    the resumed follower got no snapshot"; cat "$T/resumed.jsonl"; exit 1; }
kill "$P" 2>/dev/null || true
grep -q '"stream.resync"' "$T/resumed.jsonl" && { echo "    a cursor of this process was refused"; cat "$T/resumed.jsonl"; exit 1; }
again="$(grep -v '"status"' "$T/resumed.jsonl" | jq -s '[.[] | select(.type == "blackboard.signals") | .data.signals[] | select(.kind == "review_requested")] | length')"
[ "$again" = 0 ] || { echo "    the resumed follower was told the review again"; exit 1; }
echo "    a follower that resumes from its cursor and epoch is told nothing twice"

# ---------------------------------------------------------------- 3b. a snapshot is a point to resume from
# a follower that connects after the review is sent no signal at all: the snapshot's cursor is
# what it resumes from, and it is told nothing it was not sent
python3 "$T/follow.py" "$url" "$F" 3 > "$T/quiet.jsonl" 2>/dev/null &
P=$!; PIDS="$PIDS $P"
wait_for "$T/quiet.jsonl" '.type == "blackboard.snapshot"' 60 || { echo "    the quiet follower got no snapshot"; cat "$T/quiet.jsonl"; exit 1; }
kill "$P" 2>/dev/null || true
QUIET="$(grep -v '"status"' "$T/quiet.jsonl" | jq -rs 'map(select(.type == "blackboard.snapshot")) | first | .data.cursor // "absent"')"
[ "$QUIET" != absent ] && [ -n "$QUIET" ] || { echo "    the snapshot carries no cursor to resume from"; head -3 "$T/quiet.jsonl"; exit 1; }
python3 "$T/follow.py" "$url" "$F&after=$QUIET&epoch=$EPOCH" 3 > "$T/quiet-resumed.jsonl" 2>/dev/null &
P=$!; PIDS="$PIDS $P"
wait_for "$T/quiet-resumed.jsonl" '.type == "blackboard.snapshot"' 60 || { echo "    the quiet follower could not resume"; cat "$T/quiet-resumed.jsonl"; exit 1; }
kill "$P" 2>/dev/null || true
grep -q '"stream.resync"' "$T/quiet-resumed.jsonl" && { echo "    the snapshot's cursor was refused"; cat "$T/quiet-resumed.jsonl"; exit 1; }
again="$(grep -v '"status"' "$T/quiet-resumed.jsonl" | jq -s '[.[] | select(.type == "blackboard.signals") | .data.signals[] | select(.kind == "review_requested")] | length')"
[ "$again" = 0 ] || { echo "    a follower resumed from a snapshot's cursor was told the review it was never sent"; exit 1; }
echo "    a follower sent no signal resumes from its snapshot's cursor and is told nothing twice"

# ---------------------------------------------------------------- 4. another process's cursor
python3 "$T/follow.py" "$url" "$F&after=$CURSOR&epoch=another-process" 3 > "$T/foreign.jsonl" 2>/dev/null &
P=$!; PIDS="$PIDS $P"
wait_for "$T/foreign.jsonl" '.type == "blackboard.snapshot"' 60 || { echo "    the foreign follower got no snapshot"; cat "$T/foreign.jsonl"; exit 1; }
kill "$P" 2>/dev/null || true
grep -v '"status"' "$T/foreign.jsonl" | jq -s -e 'map(.type) | index("stream.resync") < index("blackboard.snapshot")' >/dev/null \
  || { echo "    a cursor of another process was not answered with a resync before a fresh snapshot"; cat "$T/foreign.jsonl"; exit 1; }
echo "    a cursor of another process is answered with a resync and a fresh snapshot"

# ---------------------------------------------------------------- 5. refused
python3 "$T/follow.py" "$url" "$F" 1 "https://evil.example" > "$T/origin.jsonl" 2>/dev/null
head -1 "$T/origin.jsonl" | jq -e '.status | test(" 403 ")' >/dev/null \
  || { echo "    a follower from another origin was not refused"; cat "$T/origin.jsonl"; exit 1; }
python3 "$T/follow.py" "$url" "/events?follow=blackboard&execution=e1" 1 > "$T/odd.jsonl" 2>/dev/null
head -1 "$T/odd.jsonl" | jq -e '.status | test(" 400 ")' >/dev/null \
  || { echo "    a query the follower does not take was not refused"; cat "$T/odd.jsonl"; exit 1; }
echo "    a page of another origin and a query the channel does not take are refused"

echo "    the blackboard is pushed over the one live channel"
