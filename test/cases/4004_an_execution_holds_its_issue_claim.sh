# majordomus-covers: capability:executions.start
# majordomus-timeout: 300
# An execution started for an issue holds the issue's exclusive mesh claim while it runs
# (I4004, Mission Control B6).
#
# Before I4004 two starts of one issue both ran: nothing between the start and the handler
# asked whether somebody was already working on it. Proven here from outside the executable,
# against a real server whose mesh runs, on a port the system picks:
#
#   a start for I4004                  -> queued, claimed (holder execution-<id>), running;
#                                         its issue_claim is held, with a key, on issue/I4004
#   the mesh                           -> one live exclusive claim for I4004, by that session
#   a second start for I4004           -> refused, policy mesh.claim, claim_conflict naming
#                                         the first execution's session and claim; nothing ran
#   the first, stopped                 -> cancelled, its claim released; the mesh holds none
#   a third start for I4004 after that -> claimed and run to the end, then released
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4004.XXXXXX")"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
# a mesh that runs in the server and links nobody (the declaration of case 3067)
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-4004\n' >> .ai/repo/mesh/majordomus.yaml
git add -A >/dev/null && git commit -qm layer

# the server, from a copy of the executable, on loopback and a port the system picks; its
# address is read from the lease it publishes (the harness of cases 3861 and 4000)
cp "$RB" "$S/majordomus"
( exec "$S/majordomus" serve --host 127.0.0.1 --port 0 --idle 0 --discovery filesystem </dev/null >"$S/serve.log" 2>&1 ) &
SRV=$!
U=""
for _ in $(seq 1 600); do
  U="$(jq -r --argjson pid "$SRV" 'select(.pid == $pid) | .url // empty' .ai/local/state/mcp/server.json 2>/dev/null || true)"
  [ -n "$U" ] && break
  kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening:"; sed 's/^/      /' "$S/serve.log"; exit 1; }
  sleep 0.2
done
[ -n "$U" ] || { echo "    the server did not publish an address in 120 s:"; sed 's/^/      /' "$S/serve.log"; exit 1; }
curl -sS --max-time 30 "$U/api/v1/mesh/cooperation" > "$S/coop.json" || true
jq -e '.active == true' "$S/coop.json" >/dev/null \
  || { echo "    the server's mesh does not run:"; sed 's/^/      /' "$S/coop.json"; exit 1; }

start() { # name steps delay_ms
  curl -sS --max-time 30 -o "$S/$1.json" -w '%{http_code}' -X POST -H 'Content-Type: application/json' \
    -d "{\"capability\":\"executions.demonstrate\",\"input\":{\"steps\":$2,\"delay_ms\":$3},\"issue\":\"I4004\"}" \
    "$U/api/v1/executions/start" || true
}
get() { curl -sS --max-time 30 "$U/api/v1/executions/get?id=$1"; }
# Wait, bounded, until the execution's snapshot satisfies a jq expression; leave it in $S/<name>.now.
until_state() { # name id jq-expression
  for _ in $(seq 1 300); do
    get "$2" > "$S/$1.now.json" 2>/dev/null || true
    jq -e "$3" "$S/$1.now.json" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  echo "    $1 never reached $3:"; jq -c '{state, issue_claim, error}' "$S/$1.now.json" 2>/dev/null || cat "$S/$1.now.json"
  exit 1
}
live_claims() { # the live claims for I4004 on the mesh, as compact JSON, one per line
  "$S/majordomus" mesh state --full --format json 2>/dev/null \
    | jq -c '[.. | objects | select(has("scope") and (.issue? == "I4004") and ((.state? | tostring) | test("held")))] | unique_by(.key) | .[]'
}

# ---------------------------------------------------------------- the first start holds it
[ "$(start first 100 1000)" = 200 ] || { echo "    the first start was not accepted:"; sed 's/^/      /' "$S/first.json"; exit 1; }
FIRST="$(jq -r .id "$S/first.json")"
jq -e '.issue_claim == {"issue":"I4004","standing":"wanted"}' "$S/first.json" >/dev/null \
  || { echo "    the start's answer does not say it wants the claim:"; jq -c .issue_claim "$S/first.json"; exit 1; }
until_state first "$FIRST" '.state == "running"'
jq -e --arg s "execution-$FIRST" '.issue_claim.standing == "held" and .issue_claim.session == $s
       and .issue_claim.scope == ["issue/I4004"] and (.issue_claim.key | type == "string")' "$S/first.now.json" >/dev/null \
  || { echo "    the running execution does not hold its claim:"; jq -c .issue_claim "$S/first.now.json"; exit 1; }
KEY="$(jq -r .issue_claim.key "$S/first.now.json")"
curl -sS --max-time 30 "$U/api/v1/executions/events?id=$FIRST" > "$S/first.events.json"
jq -e --arg s "execution-$FIRST" '[.events[] | .type] as $t
       | ($t | index("execution.claimed")) < ($t | index("execution.started"))
       and ([.events[] | select(.type == "execution.claimed") | .data.holder] == [$s])' "$S/first.events.json" >/dev/null \
  || { echo "    the first was not claimed before it was started:"; jq -c '[.events[] | .type]' "$S/first.events.json"; exit 1; }
live_claims > "$S/claims.1"
[ "$(wc -l < "$S/claims.1" | tr -d ' ')" = 1 ] && grep -q "\"$KEY\"" "$S/claims.1" \
  || { echo "    the mesh does not hold exactly the first's claim:"; sed 's/^/      /' "$S/claims.1"; exit 1; }
grep -q '"exclusive"' "$S/claims.1" || { echo "    the claim is not exclusive:"; cat "$S/claims.1"; exit 1; }

# ---------------------------------------------------------------- the second start is refused
[ "$(start second 1 0)" = 200 ] || { echo "    the second start was not accepted:"; sed 's/^/      /' "$S/second.json"; exit 1; }
SECOND="$(jq -r .id "$S/second.json")"
until_state second "$SECOND" '.state == "refused"'
jq -e --arg s "execution-$FIRST" --arg k "$KEY" '.error.code == "refused"
       and (.error.message | startswith("claim_conflict: I4004"))
       and (.error.message | contains($s)) and (.error.message | contains($k))
       and .issue_claim.standing == "refused" and (has("started_at") | not)' "$S/second.now.json" >/dev/null \
  || { echo "    the second start was not refused naming the first's claim:"; jq -c '{state, error, issue_claim}' "$S/second.now.json"; exit 1; }
curl -sS --max-time 30 "$U/api/v1/executions/events?id=$SECOND" > "$S/second.events.json"
jq -e '[.events[] | .type] | (index("execution.started") | not) and index("execution.refused")
       and ([.[] | select(. == "execution.claimed")] | length == 0)' "$S/second.events.json" >/dev/null \
  || { echo "    the refused start ran or was claimed:"; jq -c '[.events[] | .type]' "$S/second.events.json"; exit 1; }
jq -e '[.events[] | select(.type == "execution.refused") | .data.policy] == ["mesh.claim"]' "$S/second.events.json" >/dev/null \
  || { echo "    the refusal does not name its policy:"; jq -c '[.events[] | select(.type == "execution.refused")]' "$S/second.events.json"; exit 1; }

# ---------------------------------------------------------------- the first, stopped, gives it back
curl -sS --max-time 30 -o "$S/cancel.json" -X POST -H 'Content-Type: application/json' \
  -d "{\"id\":\"$FIRST\"}" "$U/api/v1/executions/cancel" || true
until_state first "$FIRST" '.state == "cancelled"'
jq -e '.issue_claim.standing == "released" and (.issue_claim | has("detail") | not)' "$S/first.now.json" >/dev/null \
  || { echo "    the cancelled execution did not release its claim:"; jq -c .issue_claim "$S/first.now.json"; exit 1; }
live_claims > "$S/claims.2"
[ -s "$S/claims.2" ] && { echo "    a claim for I4004 is still live after the run ended:"; sed 's/^/      /' "$S/claims.2"; exit 1; }

# ---------------------------------------------------------------- and the issue is free again
[ "$(start third 1 0)" = 200 ] || { echo "    the third start was not accepted:"; sed 's/^/      /' "$S/third.json"; exit 1; }
THIRD="$(jq -r .id "$S/third.json")"
until_state third "$THIRD" '.state == "succeeded"'
jq -e '.issue_claim.standing == "released"' "$S/third.now.json" >/dev/null \
  || { echo "    the third did not run under, and release, its claim:"; jq -c .issue_claim "$S/third.now.json"; exit 1; }

echo "    $FIRST held $KEY; $SECOND refused naming it; cancelled -> released; $THIRD claimed, ran, released"
