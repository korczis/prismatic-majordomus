# majordomus-covers: none
# majordomus-timeout: 600
# One integration executor per repository across machines (ADR 0101 §7): two clones of one
# forge repository, each with its own node identity and its own `majordomus serve`, linked
# through the repository's mesh on loopback.
#
#   1. a continuous drain on A holds the lease and an exclusive mesh claim on
#      integration/o/r/master; its brief names the claim
#   2. `prs drain` on B exits 12, names A's session, and takes no lease of its own
#   3. A's drain stopped with SIGTERM releases the claim, and B's executor runs
#
# The killed-executor recovery and a checkout without a mesh are proven in
# apps/majordomus-cli/tests/integration_across_machines.rs; the unit decisions in
# integration::exclusive.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/origin.git"; BIN="$T/bin"
mkdir -p "$BIN"
gitq init -q --bare -b master "$ORIGIN"
cat > "$BIN/gh" <<EOF
#!/bin/sh
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  *) echo '[]' ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

clone() { # dir
  gitq clone -q "$ORIGIN" "$1" 2>/dev/null || { mkdir -p "$1"; gitq -C "$1" init -q -b master; gitq -C "$1" remote add origin "$ORIGIN"; }
  ( cd "$1" || exit
    # the second clone already has the first one's .ai/ from the origin
    [ -f .ai/manifest.yaml ] || "$MJ" init >/dev/null
    "$MJ" update >/dev/null
    gitq add -A && gitq commit -q --allow-empty -m base && gitq push -q -f origin HEAD:master )
}

declare_mesh() { # repo [seed-url]
  mkdir -p "$1/.ai/repo/mesh"
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
    printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-924\n'
    [ -z "${2:-}" ] || printf '  seeds:\n    - %s\n' "$2"
  } > "$1/.ai/repo/mesh/majordomus.yaml"
  git -C "$1" add .ai/repo/mesh/majordomus.yaml
}

serve() { # repo state
  ( cd "$1" && XDG_STATE_HOME="$2" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$1.serve.log" 2>&1 ) &
  PIDS="$PIDS $!"
}

url_of() { jq -r '.url // empty' "$1/.ai/local/state/mcp/server.json" 2>/dev/null; }

poll() { # N CMD...
  local n="$1" i=0; shift
  while [ "$i" -lt "$n" ]; do
    "$@" >/dev/null 2>&1 && return 0
    i=$((i + 1)); sleep 0.2
  done
  printf '    gave up waiting for: %s\n' "$*"
  return 1
}

A="$T/machine-a"; B="$T/machine-b"
clone "$A"; clone "$B"
mkdir -p "$T/state-a" "$T/state-b"
declare_mesh "$A"
serve "$A" "$T/state-a"
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$A/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$A.serve.log"; exit 1; }
declare_mesh "$B" "$(url_of "$A")"
serve "$B" "$T/state-b"
poll 150 bash -c "[ -n \"\$(jq -r '.url // empty' '$B/.ai/local/state/mcp/server.json' 2>/dev/null)\" ]" || { cat "$B.serve.log"; exit 1; }
linked() { ( cd "$1" && "$RB" mesh peers --format json ) | jq -e '[.machines[].runtimes[] | select(.link.state == "connected")] | length == 1'; }
poll 150 linked "$A" || { cat "$A.serve.log" "$B.serve.log"; exit 1; }
poll 150 linked "$B"

XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" refresh >/dev/null || { echo "    A could not observe the forge"; exit 1; }
XDG_STATE_HOME="$T/state-b" "$RB" prs --repo "$B" refresh >/dev/null || { echo "    B could not observe the forge"; exit 1; }

# ---------------------------------------------------------------- 1. A holds it
XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" drain --continuous --interval 30 >"$T/a.out" 2>"$T/a.err" &
DRAIN=$!; PIDS="$PIDS $DRAIN"
held_on_b() { ( cd "$B" && "$RB" mesh state --format json ) | jq -e '[.state.claims[] | select(.state.state == "held" and .scope == ["integration/o/r/master"])] | length == 1'; }
poll 300 held_on_b || { echo "    B never saw A's integration claim"; cat "$T/a.out" "$T/a.err"; exit 1; }
expect_exit 0 "$RB" prs --repo "$A" brief
expect_grep 'across machines by mesh claim'

# ---------------------------------------------------------------- 2. B is refused
LOCK_B="$(git -C "$B" rev-parse --path-format=absolute --git-common-dir)/majordomus/locks/integration-master.lock"
rc=0; out="$(XDG_STATE_HOME="$T/state-b" "$RB" prs --repo "$B" drain --max 1 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    B's executor was not refused (exit $rc): $out"; exit 1; }
case "$out" in *"another integration executor holds integration/o/r/master on the mesh"*"held by session "*"/integration-"*) ;;
  *) echo "    the refusal does not name the mesh holder: $out"; exit 1 ;; esac
[ ! -f "$LOCK_B" ] || { echo "    B took its clone's lease although the repository's was held"; exit 1; }

# ---------------------------------------------------------------- 3. released, then B's
kill -TERM "$DRAIN"; wait "$DRAIN" 2>/dev/null || true
free_on_b() { ( cd "$B" && "$RB" mesh state --format json ) | jq -e '[.state.claims[] | select(.state.state == "held" and .scope == ["integration/o/r/master"])] | length == 0'; }
poll 300 free_on_b || { echo "    A's claim was not released"; exit 1; }
rc=0; out="$(XDG_STATE_HOME="$T/state-b" "$RB" prs --repo "$B" drain --max 1 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    B's executor did not run once the repository was free (exit $rc): $out"; exit 1; }
exit 0
