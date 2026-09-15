# majordomus-covers: handover
# majordomus-timeout: 600
# A handover written with the shell tool on one runtime is published to the mesh, consumed
# on another runtime, and found there by the shell tool's own `handover --resolve` — the
# operator's path across the two programs and two servers, with no file copied by hand.
# On the way: an exclusive claim made on one runtime makes the other runtime's command line
# exit 10 with claim_conflict, and `mesh verify` passes on both.
#
# Two checkouts, two node identities (their own XDG_STATE_HOME), two `majordomus serve`
# processes on loopback: the second declares the first as its seed. The multi-process
# depth — isolation, trust, crash, three runtimes — is apps/majordomus-cli/tests/
# mesh_cooperation.rs; the real network is test/mesh-lab/run.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

mkrepo() {
  mkdir -p "$1"
  (
    cd "$1"
    git init -q .
    git config user.email t@example.com
    git config user.name t
    "$MJ" init >/dev/null
    "$MJ" update >/dev/null
    mkdir -p lib && echo a > lib/a
    git add -A && git commit -qm base
    git checkout -qb feature/mesh
  )
}

declare_mesh() { # repo [seed-url]
  mkdir -p "$1/.ai/repo/mesh"
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
    printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-368\n'
    [ -z "${2:-}" ] || printf '  seeds:\n    - %s\n' "$2"
  } > "$1/.ai/repo/mesh/majordomus.yaml"
  # Declarations are discovered through the version-control index, as in any repository
  # `majordomus init` created: a declaration is what a person commits.
  git -C "$1" add .ai/repo/mesh/majordomus.yaml
}

serve() { # repo state
  ( cd "$1" && XDG_STATE_HOME="$2" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$1.serve.log" 2>&1 ) &
  PIDS="$PIDS $!"
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

A="$T/machine-a"; B="$T/machine-b"
mkrepo "$A"; mkrepo "$B"
mkdir -p "$T/state-a" "$T/state-b"

declare_mesh "$A"
serve "$A" "$T/state-a"
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$A/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$A.serve.log"; exit 1; }
URL_A="$(url_of "$A")"
declare_mesh "$B" "$URL_A"
serve "$B" "$T/state-b"
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$B/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$B.serve.log"; exit 1; }

linked() { ( cd "$1" && "$RB" mesh peers --format json ) | jq -e '[.machines[].runtimes[] | select(.link.state == "connected")] | length == 1'; }
poll 150 linked "$A" || { cat "$A.serve.log" "$B.serve.log"; exit 1; }
poll 150 linked "$B"

# ---------------------------------------------------------------- 1. a claim excludes across runtimes
cd "$A"
expect_exit 0 "$RB" mesh claim lib --session a1 --issue '#184'
expect_grep '^written '
claim_seen() { ( cd "$B" && "$RB" mesh state --format json ) | jq -e '[.state.claims[] | select(.state.state == "held")] | length == 1'; }
poll 100 claim_seen
cd "$B"
expect_exit 10 "$RB" mesh claim lib/a --session b1
expect_grep 'claim_conflict'

# ---------------------------------------------------------------- 2. a handover crosses and resolves
cd "$A"
"$MJ" start "carry the mesh" --scope lib >/dev/null
expect_exit 0 bash -c "printf '# Objective\nship the mesh\n# Current State\nlinks work\n# Next Action\nrender the machine tree\n' | '$MJ' handover"
# stdout alone: expect_exit folds stderr (the server discovery's log lines) into its capture.
ID="$("$RB" mesh handover publish --issue '#184' --format json 2>/dev/null | jq -r '.key')"
[ -n "$ID" ] && [ "$ID" != null ] || { echo "    publish answered no handover id: $LAST_OUT"; exit 1; }
held_on_b() { ( cd "$B" && "$RB" mesh state --format json ) | jq -e --arg h "$ID" '[.state.handovers[] | select(.id == $h)] | length == 1'; }
poll 100 held_on_b

cd "$B"
expect_exit 0 "$RB" mesh handover consume "$ID" --session b1
expect_grep '^record +\.ai/local/state/handovers/'
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Match: same_branch'
expect_grep '^# Next Action'
expect_grep 'render the machine tree'
# Consumed twice, written once.
expect_exit 0 "$RB" mesh handover consume "$ID" --session b1
[ "$(grep -l "mesh_handover: $ID" .ai/local/state/handovers/*.md | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    a second consume wrote a second record"; exit 1; }
consumed_on_a() { ( cd "$A" && "$RB" mesh state --format json ) | jq -e --arg h "$ID" '[.state.handovers[] | select(.id == $h and (.consumed_by | length) >= 1)] | length == 1'; }
poll 100 consumed_on_a

# ---------------------------------------------------------------- 3. both runtimes verify
cd "$B"
expect_exit 0 "$RB" mesh verify
expect_grep 'cooperation verified'
cd "$A"
expect_exit 0 "$RB" mesh verify --format json
expect_grep '"ok": true'
