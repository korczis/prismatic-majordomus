# majordomus-covers: none
# majordomus-timeout: 300
# A rebuild leaves the server started from the build output ready on its address (I2059).
#
# The executable is laid out the way cargo lays out a build — <target>/debug/majordomus with
# cargo's CACHEDIR.TAG in <target> — in this case's own directory, and a server is started
# from it on a port the system picks. Only that process is stopped, by the pid this case
# started: no other checkout's server and no other session's binary is touched.
#
#   the lease names a copy under debug/served/, not debug/majordomus     the server runs a copy
#   a rebuild (a new file renamed over debug/majordomus)                 as cargo writes one
#   GET /api/v1/ready  ready true, no `stale`, before and after          the server still serves
#   serve ensure from the rebuilt file: ready, same address, same pid    a launcher attaches
#
# Case 885 holds the other half: a server run from a file that is then replaced stops calling
# itself ready. Here the file it runs from is never the one a build writes.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3061.XXXXXX")"
PID=""
trap '[ -n "$PID" ] && kill "$PID" 2>/dev/null; rm -rf "$S"' EXIT

mkdir -p "$S/target/debug"
printf 'Signature: 8a477f597d28d172789f06886806bc55\n' > "$S/target/CACHEDIR.TAG"
BUILT="$S/target/debug/majordomus"
cp "$RB" "$BUILT"
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer
LEASE=.ai/local/state/mcp/server.json

( exec "$BUILT" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
PID=$!
U=""
for _ in $(seq 1 150); do
  U="$(jq -r --argjson pid "$PID" 'select(.pid == $pid) | .url // empty' "$LEASE" 2>/dev/null || true)"
  [ -n "$U" ] && break
  sleep 0.2
done
[ -n "$U" ] || { echo "    the server did not publish an address"; cat "$S/serve.log"; exit 1; }

# the lease names the copy the server runs, never the file a build writes
EXE="$(jq -r '.executable.path // empty' "$LEASE")"
case "$EXE" in
  "$S"/target/debug/served/majordomus-*|*/target/debug/served/majordomus-*) ;;
  *) echo "    the lease names $EXE, not a copy under debug/served/"; jq . "$LEASE"; exit 1 ;;
esac
[ -x "$EXE" ] || { echo "    the copy the lease names is not there: $EXE"; exit 1; }

ready() {
  curl -fsS --max-time 30 "$U/api/v1/ready" > "$S/$1.json" \
    || { echo "    $U did not answer ($1)"; cat "$S/serve.log"; exit 1; }
  jq -e '.ready == true and (has("stale") | not)' "$S/$1.json" >/dev/null \
    || { echo "    the server is not ready ($1)"; cat "$S/$1.json"; exit 1; }
}
ready before

# a rebuild: a new file renamed over the build output, as cargo writes it. The bytes are the
# same executable; its modification time is not, which is what a rebuild changes.
cp "$RB" "$BUILT.new"
touch -t 202001010000 "$BUILT.new"
mv "$BUILT.new" "$BUILT"

ready after
cmp -s "$RB" "$EXE" || { echo "    the copy the server runs was changed by the rebuild"; exit 1; }

# a launcher of the rebuilt file attaches: the same server, on the same address, ready, and
# nothing started beside it. Before I2059 this call took the lease over from a server whose
# file had been replaced, and started a successor on an address of its own.
"$BUILT" serve ensure --port 0 --wait 30 --format json > "$S/ensure.json" 2>"$S/ensure.err" \
  || { echo "    serve ensure from the rebuilt file is not ready"; cat "$S/ensure.json" "$S/ensure.err"; exit 1; }
jq -e --arg u "$U" --argjson pid "$PID" \
  '.standing == "ready" and .url == $u and .pid == $pid and .started == false' "$S/ensure.json" >/dev/null \
  || { echo "    serve ensure does not report the same server ready on $U"; cat "$S/ensure.json"; exit 1; }
exit 0
