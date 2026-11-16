# majordomus-covers: capability:mesh.state
# majordomus-timeout: 600
# A node whose own store answers offers it to the mesh as a signed, expiring journal record,
# and a stopped provider's offer is gone within its lifetime (I2671, ADR 0157 decision 2).
#
# Two checkouts, two node identities (their own XDG_STATE_HOME), two `majordomus serve`
# processes on loopback, linked as case 368 links them. Machine A declares a store in its
# node file (XDG_CONFIG_HOME/majordomus/build-cache.json, outside any repository); the store
# is a loopback stub that answers the readiness route. What this case holds, through B's
# `mesh state` and B's journal:
#
#   1. B reads A's offer while A's store answers: A's node, the declared endpoint and bucket,
#      healthy, its age the age of A's beat on B's clock;
#   2. the record B holds is a `build_cache_offered` event signed by A's key, and its body
#      names the store and nothing else: no credential field, no secret;
#   3. A's store stops: A's next heartbeat withdraws the offer, and B reads none and says why;
#   4. A's store answers again and is offered again; then A's server stops, and within the
#      offer's lifetime (the declared expiry, 4 s) B reads it as stale with nobody withdrawing.
#
# That a tampered record is refused, that an untrusted key's record is never an offer (also
# after a key is withdrawn under a running server), and that two servers of one machine make
# one offer are held by the unit tests of apps/majordomus-cli/src/mesh/build_cache/offer.rs:
# a server this case could start under an untrusted key is refused at the handshake and never
# delivers a record at all.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
mj_runs python3 || skip "$(mj_runs_why python3)"
export MAJORDOMUS_SHARE="$ROOT/share"

PIDS=""
STORE_PID=""
stop_all() {
  local pid
  for pid in $PIDS $STORE_PID; do kill "$pid" 2>/dev/null || true; done
}
trap stop_all EXIT

mkrepo() {
  mkdir -p "$1"
  (
    cd "$1" || exit
    git init -q .
    git config user.email t@example.com
    git config user.name t
    "$MJ" init >/dev/null
    "$MJ" update >/dev/null
    git add -A && git commit -qm base
  )
}

declare_mesh() { # repo [seed-url]
  mkdir -p "$1/.ai/repo/mesh"
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n'
    printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3954\n'
    [ -z "${2:-}" ] || printf '  seeds:\n    - %s\n' "$2"
  } > "$1/.ai/repo/mesh/majordomus.yaml"
  git -C "$1" add .ai/repo/mesh/majordomus.yaml
  git -C "$1" commit -qm "the mesh declaration"
}

serve() { # repo state config
  ( cd "$1" && XDG_STATE_HOME="$2" XDG_CONFIG_HOME="$3" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$1.serve.log" 2>&1 ) &
  PIDS="$PIDS $!"
  LAST_SERVE=$!
}

url_of() { jq -r '.url // empty' "$1/.ai/local/state/mcp/server.json" 2>/dev/null; }

# poll N CMD... — run CMD every 0.2s until it succeeds, at most N tries.
poll() {
  local n="$1" i=0; shift
  while [ "$i" -lt "$n" ]; do
    "$@" >/dev/null 2>&1 && return 0
    i=$((i + 1)); sleep 0.2
  done
  printf '    gave up waiting for: %s\n' "$*"
  return 1
}

# store PORT — a loopback stand-in for MinIO that answers /minio/health/ready with 200; PORT 0
# takes a free one. Its port is written to $T/store.port once it listens.
store() {
  rm -f "$T/store.port"
  python3 -I - "$T/store-root" "$T/store.port" "$1" >"$T/store.log" 2>&1 <<'PY' &
import http.server, os, sys
os.chdir(sys.argv[1])
class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass
server = http.server.HTTPServer(("127.0.0.1", int(sys.argv[3])), Quiet)
with open(sys.argv[2] + ".tmp", "w") as f:
    f.write(str(server.server_address[1]))
os.rename(sys.argv[2] + ".tmp", sys.argv[2])
server.serve_forever()
PY
  STORE_PID=$!
  poll 100 test -s "$T/store.port" || { cat "$T/store.log"; exit 1; }
}

mkdir -p "$T/store-root/minio/health" && echo ok > "$T/store-root/minio/health/ready"
store 0
PORT="$(cat "$T/store.port")"
ENDPOINT="http://127.0.0.1:$PORT"
mkdir -p "$T/config-a/majordomus" "$T/config-b" "$T/state-a" "$T/state-b" "$T/data-a"
printf '{"provider":{"provider":"minio","scope":"lan","endpoint":"%s","bucket":"majordomus-build-cache","priority":3,"data_path":"%s"}}\n' \
  "$ENDPOINT" "$T/data-a" > "$T/config-a/majordomus/build-cache.json"

A="$T/machine-a"; B="$T/machine-b"
mkrepo "$A"; mkrepo "$B"
declare_mesh "$A"
serve "$A" "$T/state-a" "$T/config-a"
A_PID=$LAST_SERVE
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$A/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$A.serve.log"; exit 1; }
URL_A="$(url_of "$A")"
declare_mesh "$B" "$URL_A"
serve "$B" "$T/state-b" "$T/config-b"
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$B/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$B.serve.log"; exit 1; }
URL_B="$(url_of "$B")"

linked() { ( cd "$1" && "$RB" mesh peers --format json ) | jq -e '[.machines[].runtimes[] | select(.link.state == "connected")] | length == 1'; }
poll 150 linked "$A" || { cat "$A.serve.log" "$B.serve.log"; exit 1; }
poll 150 linked "$B"

# reading — B's build-cache reading, as `mesh state` answers it
reading() { ( cd "$B" && "$RB" mesh state --format json ) | jq -c '.build_cache // {}'; }
offered() { reading | jq -e --arg e "$ENDPOINT" '[.offers[]? | select(.endpoint == $e)] | length == 1'; }
none_and() { reading | jq -e --arg w "$1" '((.offers // []) | length == 0) and ([.ignored[]?.why | select(contains($w))] | length >= 1)'; }

# ---------------------------------------------------------------- 1. B reads A's offer
poll 100 offered || { echo "    B never read A's offer: $(reading)"; cat "$A.serve.log"; exit 1; }
offer="$(reading | jq -c '.offers[0]')"
printf '%s' "$offer" | jq -e '.provider == "minio" and .scope == "lan" and .bucket == "majordomus-build-cache"
  and .priority == 3 and .health == "healthy" and .protocol == "s3" and (.age_seconds <= 4)
  and (.headroom_gib | type == "number")' >/dev/null \
  || { echo "    the offer B reads is not the store A declared: $offer"; exit 1; }
A_NODE="$(printf '%s' "$offer" | jq -r '.node')"

# ---------------------------------------------------------------- 2. signed by A, and nothing secret
events="$(curl -fsS --max-time 10 "$URL_B/api/v1/mesh/events?newest=true&limit=1000")" \
  || { echo "    B's journal could not be read"; exit 1; }
record="$(printf '%s' "$events" | jq -c '[.events[] | select(.body.kind == "build_cache_offered")] | last')"
printf '%s' "$record" | jq -e --arg n "$A_NODE" '(.stream | startswith($n + "-")) and (.sig | length == 128) and (.pk | length == 64)' >/dev/null \
  || { echo "    B holds no build_cache_offered record signed by A's node: $record"; exit 1; }
printf '%s' "$record" | jq -e '.body.offer | keys == ["bucket","endpoint","headroom_gib","lifetime_seconds","priority","protocol","provider","scope"]' >/dev/null \
  || { echo "    the record carries a field nobody listed: $(printf '%s' "$record" | jq -c '.body.offer | keys')"; exit 1; }
printf '%s' "$record" | jq -e '.body.offer.lifetime_seconds == 4' >/dev/null \
  || { echo "    the record's lifetime is not the declared expiry: $record"; exit 1; }
if printf '%s' "$record" | grep -Eqi 'secret|password|token|credential|access_key'; then
  echo "    the record carries a secret-shaped word: $record"; exit 1
fi

# ---------------------------------------------------------------- 3. the store stops: withdrawn
kill "$STORE_PID"; wait "$STORE_PID" 2>/dev/null || true; STORE_PID=""
poll 100 none_and "withdrawn: its own probe says unhealthy" \
  || { echo "    B still reads a stopped store's offer: $(reading)"; exit 1; }

# ---------------------------------------------------------------- 4. offered again; then A stops
store "$PORT"
poll 100 offered || { echo "    a store that answers again is not offered again: $(reading)"; exit 1; }
kill "$A_PID"; wait "$A_PID" 2>/dev/null || true
started=$SECONDS
poll 100 none_and "stale: its runtime last beat" \
  || { echo "    a stopped provider's offer is not stale: $(reading)"; exit 1; }
# the lifetime is 4 s; a poll every 0.2 s and a beat a second leave it stale well within 10
[ $((SECONDS - started)) -le 10 ] || { echo "    stale only after $((SECONDS - started)) s"; exit 1; }
echo "    ok: offered while it answers, signed by its node, withdrawn when it stops, stale when its runtime stops"
