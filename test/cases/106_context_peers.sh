# majordomus-covers: context
# The peers section of `context`: the other workers attached to this repository's shared
# server, and specifically the ones whose claims meet this task's scope.
#
# Co-operation between concurrent workers failed by being voluntary — a session announced,
# reconnected under a new identity and was invisible to eight others for three hours; two
# sessions built the same subsystem because neither read the board. So the board arrives in
# the one command every worker is already told to run. What this case holds is that it
# arrives, that it names the collision rather than merely the peers, and above all that it
# is never load-bearing: no lease, no server, a server that does not answer, an executable
# that is not built, or a board that says nothing must all leave `context` exactly as it
# was.
#
# Where the server is now comes from `serve status`, the executable's one typed reading of
# the lease (ADR 0035, project.the-lease-is-read-once); the lease this case plants is read
# by that reader rather than by the shell. Everything asserted below is asserted about the
# same contract as before the move, which is the point of leaving the assertions alone.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq absent; skipping"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    curl absent; skipping"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "    python3 absent; skipping"; exit 0; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib site && echo a > lib/a && echo s > site/s && git add . && git commit -qm base
"$MJ" start "the ordering rule" --scope lib --profile implementation >/dev/null

# --- with no shared server, context is what it always was
expect_exit 0 "$MJ" context
expect_no_grep '^## PEERS'
order=$(printf '%s\n' "$LAST_OUT" | grep -E '^## ' | tr '\n' ' ')
case "$order" in "## GIT ## TASK"*) ;; *) echo "    a repository with one worker grew a section about it: $order"; exit 1 ;; esac

# --- an executable that is not built is the same answer, and it is proved above: the
# section is asked for from `serve status` and there is nothing to ask, so `context` is
# exactly what it was. That half of the contract is what a tree without a build can hold;
# the board itself needs the reader, so the rest is a skip rather than a failure.
BIN="${MAJORDOMUS_BIN:-$ROOT/apps/majordomus-cli/target/debug/majordomus}"
[ -x "$BIN" ] || { echo "    no built executable to read the lease with; the never-load-bearing half is proved above"; exit 0; }

# --- a board with one peer whose claim is inside this task's scope
board="$T/board.json"
cat > "$board" <<'JSON'
{"count":2,"peers":[
 {"id":"p1","client":{"name":"codex","version":"1"},"transport":"http","connected_at":"2026-09-09T18:00:00Z","last_seen_seconds_ago":4,"attached":true,
  "announcement":{"intent":"the evidence subsystem. Second sentence is not shown.","scope":["lib/evidence.sh","docs/EVIDENCE.md"],"at":"2026-09-09T18:00:00Z"}},
 {"id":"p2","client":{"name":"gemini-cli","version":"1"},"transport":"http","connected_at":"2026-09-09T17:00:00Z","last_seen_seconds_ago":900,"attached":false,
  "announcement":{"intent":"the installer","scope":["site"],"at":"2026-09-09T17:00:00Z"}}
]}
JSON
srv="$T/serve.py"
cat > "$srv" <<'PY'
import http.server, socketserver, sys, threading
body = open(sys.argv[2], 'rb').read()
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/api/v1/peers":
            self.send_response(404); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
with socketserver.TCPServer(("127.0.0.1", 0), H) as s:
    print(s.server_address[1], flush=True)
    open(sys.argv[1], "w").write(str(s.server_address[1]))
    s.serve_forever()
PY
python3 "$srv" "$T/port" "$board" > "$T/port.out" 2>"$T/serve.err" &
stub=$!
trap 'kill "$stub" 2>/dev/null || true' EXIT
i=0; until [ -s "$T/port" ] || [ "$i" -ge 100 ]; do i=$((i+1)); sleep 0.05; done
[ -s "$T/port" ] || { echo "    the stub board never bound a port"; cat "$T/serve.err"; exit 1; }
port="$(cat "$T/port")"
mkdir -p .ai/local/state/mcp
printf '{"schema":"majordomus-mcp-lease/v1","url":"http://127.0.0.1:%s"}\n' "$port" > .ai/local/state/mcp/server.json

expect_exit 0 "$MJ" context
expect_grep '^## PEERS'
# the peer is named with what it said, and only the first sentence of it
expect_grep 'p1   here'
expect_grep 'the evidence subsystem$'
expect_no_grep 'Second sentence is not shown'
expect_grep 'claims lib/evidence.sh docs/EVIDENCE.md'
# a peer that has gone is still on the board, and said so
expect_grep 'p2   gone'
# and the collision is named as a collision, both sides of it
expect_grep '^OVERLAP'
expect_grep 'p1 claims lib/evidence.sh, inside your lib'
# what does not meet the scope is not reported as if it did
expect_no_grep 'p2 claims site'
# the section sits between git and the task: a worker on your paths outranks your own records
order=$(printf '%s\n' "$LAST_OUT" | grep -E '^## ' | tr '\n' ' ')
case "$order" in "## GIT ## PEERS ## TASK"*) ;; *) echo "    peers is not between git and task: $order"; exit 1 ;; esac

# --- a board nobody is on says nothing at all
printf '{"count":0,"peers":[]}\n' > "$board"
kill "$stub" 2>/dev/null || true; wait "$stub" 2>/dev/null || true
python3 "$srv" "$T/port2" "$board" > /dev/null 2>&1 &
stub=$!
i=0; until [ -s "$T/port2" ] || [ "$i" -ge 100 ]; do i=$((i+1)); sleep 0.05; done
printf '{"schema":"majordomus-mcp-lease/v1","url":"http://127.0.0.1:%s"}\n' "$(cat "$T/port2")" > .ai/local/state/mcp/server.json
expect_exit 0 "$MJ" context
expect_no_grep '^## PEERS'

# --- a lease pointing at nothing is not an error: the hint is never load-bearing
kill "$stub" 2>/dev/null || true; wait "$stub" 2>/dev/null || true
expect_exit 0 "$MJ" context
expect_no_grep '^## PEERS'
expect_grep '^## GIT'
expect_grep '^## TASK'

# --- a malformed lease is the same: nothing, and no complaint
printf 'not json at all\n' > .ai/local/state/mcp/server.json
expect_exit 0 "$MJ" context
expect_no_grep '^## PEERS'

echo "    the board reaches the worker who never asked for it, and never at the cost of context"
