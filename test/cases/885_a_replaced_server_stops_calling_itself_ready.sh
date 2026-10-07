# majordomus-covers: none
# majordomus-timeout: 300
# A server serving code that no longer exists stops calling itself ready (I1502).
#
# The server is started from a copy of the executable in this case's own directory, on a
# port the system picks, and only that process is stopped, by the pid this case started:
# the shared server of any checkout, and the binary every other session runs, are never
# touched. The copy is then replaced on disk the way a rebuild replaces it — a new file
# renamed over the path — and the server's own answers are read:
#
#   GET /api/v1/ready  ready true, no `stale`        before the replacement
#   GET /api/v1/ready  ready false, `stale` says why  from the first request after it
#   GET /              `stale` null, then the same reason
#
# There is no window in which the replaced server still says ready: the check is made on
# every request against the file the process was loaded from, so the first answer after
# the rename is already false, and it stays false for as long as that process serves. A
# fresh start from the new file is ready again, which is the proof that a current server is
# not reported stale. No second executable is needed to tell: everything is read from the
# server's own documents.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj885.XXXXXX")"
PID=""
trap '[ -n "$PID" ] && kill "$PID" 2>/dev/null; rm -rf "$S"' EXIT

cp "$RB" "$S/majordomus"
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

# start: serve this checkout from the copy, and wait for its lease to name an address
start() {
  ( exec "$S/majordomus" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
  PID=$!
  U=""
  for _ in $(seq 1 150); do
    U="$(jq -r --argjson pid "$PID" 'select(.pid == $pid) | .url // empty' .ai/local/state/mcp/server.json 2>/dev/null)"
    [ -n "$U" ] && return 0
    sleep 0.2
  done
  echo "    the server did not publish an address"; cat "$S/serve.log"; return 1
}
stop() { kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; PID=""; }

start || exit 1
curl -fsS --max-time 30 "$U/api/v1/ready" > "$S/ready1.json"
jq -e '.ready == true and (has("stale") | not)' "$S/ready1.json" >/dev/null \
  || { echo "    a current server is not ready"; cat "$S/ready1.json"; exit 1; }
curl -fsS --max-time 30 -H 'Accept: application/json' "$U/" > "$S/root1.json"
jq -e '.stale == null' "$S/root1.json" >/dev/null \
  || { echo "    the root document of a current server names a staleness"; jq '{stale}' "$S/root1.json"; exit 1; }

# a rebuild: a new file renamed over the path the server was loaded from. The bytes are the
# same executable; its modification time is not, which is what a rebuild changes.
cp "$RB" "$S/majordomus.new"
touch -t 202001010000 "$S/majordomus.new"
mv "$S/majordomus.new" "$S/majordomus"

curl -fsS --max-time 30 "$U/api/v1/ready" > "$S/ready2.json"
jq -e '.ready == false and (.stale | test("replaced"))' "$S/ready2.json" >/dev/null \
  || { echo "    a server whose executable was replaced still calls itself ready"; cat "$S/ready2.json"; exit 1; }
curl -fsS --max-time 30 -H 'Accept: application/json' "$U/" > "$S/root2.json"
jq -e '.stale | type == "string" and test("replaced")' "$S/root2.json" >/dev/null \
  || { echo "    the root document does not say the code is gone"; jq '{stale}' "$S/root2.json"; exit 1; }
# the reason is served to anyone who can reach the socket: it names no path of the host
if grep -qF "$S" "$S/root2.json" "$S/ready2.json"; then
  echo "    the staleness reason names a path of the host"; exit 1
fi

# a fresh start from the new file is current again
stop
start || exit 1
curl -fsS --max-time 30 "$U/api/v1/ready" > "$S/ready3.json"
jq -e '.ready == true and (has("stale") | not)' "$S/ready3.json" >/dev/null \
  || { echo "    a fresh start from the new file is reported stale"; cat "$S/ready3.json"; exit 1; }
exit 0
