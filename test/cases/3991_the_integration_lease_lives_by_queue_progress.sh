# majordomus-covers: none
# majordomus-timeout: 600
# The integration lease lives by its executor's progress through the queue, not by its process
# existing (I3991). Two clones of one forge repository, each with its own `majordomus serve`,
# linked through the repository's mesh on loopback, whose progress ttl is 8 s:
#
#   1. a continuous drain on A takes the lease and its mesh claim, finds nothing to do, and
#      advances nothing: its claim reads suspended while its process lives, and its brief
#      names a lease held without queue progress
#   2. a continuous drain on B is admitted over the suspended claim, and keeps working
#   3. A resumes on a ready pull request: the mesh releases its claim as overtaken, A merges
#      nothing, records lease_lost, and its drain stops with exit 10; B's claim still holds
#
# The unit decisions are in integration::exclusive and integration::tests; case 924 proves
# the claim itself across machines.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/origin.git"; BIN="$T/bin"; FORGE="$T/forge"
mkdir -p "$BIN" "$FORGE"
gitq init -q --bare -b master "$ORIGIN"
# the forge: B is never shown a pull request; A is shown one ready pull request once the case
# says so. A merge is logged and refused, so one that is asked for fails the case loudly
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$(basename "\$(pwd -P)") \$*" >> "$FORGE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "pr list")
    case "\$(pwd -P)" in
      */machine-a) if [ -f "$FORGE/ready" ]; then cat "$FORGE/prs.json"; else echo '[]'; fi ;;
      *) echo '[]' ;;
    esac ;;
  "pr merge") echo "MERGE ASKED" >> "$FORGE/log"; exit 1 ;;
  "pr view") echo OPEN ;;
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
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
    [ -f .ai/manifest.yaml ] || "$MJ" init >/dev/null
    "$MJ" update >/dev/null
    gitq add -A && gitq commit -q --allow-empty -m base && gitq push -q -f origin HEAD:master )
}

declare_mesh() { # repo [seed-url]
  mkdir -p "$1/.ai/repo/mesh"
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
    printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  session_ttl_seconds: 8\n  repository: case-3991\n'
    [ -z "${2:-}" ] || printf '  seeds:\n    - %s\n' "$2"
  } > "$1/.ai/repo/mesh/majordomus.yaml"
  git -C "$1" add .ai/repo/mesh/majordomus.yaml
  gitq -C "$1" commit -qm "the mesh declaration"
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

# the ready pull request A will be shown: a change on the forge's master, its required check
# passed
( cd "$A" && gitq fetch -q origin && gitq checkout -q -b feature/1 origin/master \
  && echo one > one.txt && gitq add one.txt && gitq commit -qm "change 1" \
  && gitq push -q origin HEAD:refs/heads/feature/1 HEAD:refs/pull/1/head && gitq checkout -q master )
H="$(git -C "$A" rev-parse feature/1)"
printf '[{"number":1,"title":"change 1","author":{"login":"someone"},"headRefName":"feature/1","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}]\n' \
  "$H" > "$FORGE/prs.json"

XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" refresh >/dev/null || { echo "    A could not observe the forge"; exit 1; }
XDG_STATE_HOME="$T/state-b" "$RB" prs --repo "$B" refresh >/dev/null || { echo "    B could not observe the forge"; exit 1; }
# the record that unlocks continuous mode on each clone (ADR 0101 §13), as case 924 writes it
for C in "$A" "$B"; do
  TRAIL="$(git -C "$C" rev-parse --path-format=absolute --git-common-dir)/majordomus/integration/events.jsonl"
  mkdir -p "$(dirname "$TRAIL")"
  for _ in 1 2 3 4 5; do
    printf '%s\n' '{"at":"2026-10-05T00:00:00Z","actor":"seed","action":"merge_succeeded","pr":null,"reasons":[],"detail":"an earlier bounded merge"}' >> "$TRAIL"
  done
done

# the integration claims, as B's runtime reads them: key, session, standing, detail
claims() { ( cd "$B" && XDG_STATE_HOME="$T/state-b" "$RB" mesh state --full --format json ) 2>/dev/null \
  | jq -c '[.state.claims[] | select(.scope == ["integration/o/r/master"])
      | {key, session, state: .state.state, detail: (.state.detail // "")}]'; }

# ---------------------------------------------------------------- 1. A holds it and does nothing
XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" drain --continuous --interval 30 >"$T/a.out" 2>"$T/a.err" &
DRAIN_A=$!; PIDS="$PIDS $DRAIN_A"
held_by_a() { claims | jq -e '[.[] | select(.state == "held")] | length == 1'; }
poll 300 held_by_a || { echo "    B never saw A's integration claim"; cat "$T/a.out" "$T/a.err"; exit 1; }
KEY_A="$(claims | jq -r '[.[] | select(.state == "held")][0].key')"
suspended_a() { claims | jq -e --arg k "$KEY_A" '[.[] | select(.key == $k and .state == "suspended")] | length == 1'; }
poll 150 suspended_a || { echo "    A's idle claim was not suspended within 30 s (ttl 8 s): $(claims)"; cat "$T/a.out" "$T/a.err"; exit 1; }
kill -0 "$DRAIN_A" 2>/dev/null || { echo "    A's executor ended; the suspension must come while it lives"; cat "$T/a.out" "$T/a.err"; exit 1; }
idle_in_brief() { XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" brief | grep -q 'no queue progress for [0-9]* s (over the ttl of 8 s)'; }
poll 50 idle_in_brief || { echo "    A's brief does not name a lease held without progress:"; XDG_STATE_HOME="$T/state-a" "$RB" prs --repo "$A" brief; exit 1; }

# ---------------------------------------------------------------- 2. B is admitted, and works
XDG_STATE_HOME="$T/state-b" "$RB" prs --repo "$B" drain --continuous --interval 30 >"$T/b.out" 2>"$T/b.err" &
DRAIN_B=$!; PIDS="$PIDS $DRAIN_B"
held_by_b() { claims | jq -e --arg k "$KEY_A" '[.[] | select(.key != $k and .state == "held")] | length == 1'; }
poll 300 held_by_b || { echo "    B was not admitted over A's suspended claim: $(claims)"; cat "$T/b.out" "$T/b.err"; exit 1; }
SESSION_B="$(claims | jq -r --arg k "$KEY_A" '[.[] | select(.key != $k and .state == "held")][0].session | split("/") | last')"
# B's executor finds nothing either; the case beats its session as its progress would
( while :; do
    ( cd "$B" && XDG_STATE_HOME="$T/state-b" "$RB" mesh beat --session "$SESSION_B" --phase "case 3991: B works" ) >/dev/null 2>&1
    sleep 1
  done ) &
PIDS="$PIDS $!"

# ---------------------------------------------------------------- 3. A resumes: overtaken, merges nothing
touch "$FORGE/ready"
rc=0
for _ in $(seq 1 600); do kill -0 "$DRAIN_A" 2>/dev/null || break; sleep 0.2; done
kill -0 "$DRAIN_A" 2>/dev/null && { echo "    A's executor did not stop after it resumed"; cat "$T/a.out" "$T/a.err"; exit 1; }
wait "$DRAIN_A" || rc=$?
[ "$rc" = 10 ] || { echo "    A's executor exited $rc, not 10"; cat "$T/a.out" "$T/a.err"; exit 1; }
grep -q 'no longer holds the integration lease' "$T/a.out" \
  || { echo "    A's drain does not say it lost the lease:"; cat "$T/a.out" "$T/a.err"; exit 1; }
grep -q 'integration lease was lost before #1' "$T/a.out" \
  || { echo "    A's drain does not stop on the lost lease:"; cat "$T/a.out"; exit 1; }
grep -q 'MERGE ASKED' "$FORGE/log" && { echo "    an overtaken executor asked the forge to merge"; exit 1; }
TRAIL_A="$(git -C "$A" rev-parse --path-format=absolute --git-common-dir)/majordomus/integration/events.jsonl"
grep -q '"action":"lease_lost"' "$TRAIL_A" || { echo "    A's trail does not record lease_lost"; tail -5 "$TRAIL_A"; exit 1; }
grep -q '"action":"merge_attempted"' "$TRAIL_A" && { echo "    A's trail records a merge attempt"; exit 1; }
released_overtaken() { ( cd "$A" && XDG_STATE_HOME="$T/state-a" "$RB" mesh events --newest --limit 1000 --format json ) 2>/dev/null \
  | jq -e '[.events[] | select(.body.kind == "claim_released") | select((.body.reason // "") | test("another session was admitted"))] | length >= 1'; }
poll 50 released_overtaken || { echo "    A's claim was not released as overtaken: $(claims)"; exit 1; }
claims | jq -e --arg k "$KEY_A" '[.[] | select(.key == $k and .state == "released")] | length == 1' >/dev/null \
  || { echo "    A's claim does not read released: $(claims)"; exit 1; }
held_by_b >/dev/null || { echo "    B's claim did not stay held: $(claims)"; exit 1; }
kill -TERM "$DRAIN_B"; wait "$DRAIN_B" 2>/dev/null || true
echo "    A idled into suspension, B was admitted, A resumed overtaken and merged nothing"
exit 0
