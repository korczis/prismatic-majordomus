# majordomus-covers: capability:executions.protocol
# majordomus-timeout: 300
# The execution state machine names authorizing, claimed, verifying, refused and
# unknown_outcome, and holds every event to one transition table (I4001, Mission Control B3).
#
# Before I4001 an execution was queued, running, cancelling or one of three final states, so
# a refused start, a claimed ticket nobody had entered yet and a run whose outcome could not
# be read all looked the same. Proven here from outside the executable, against a real server
# on a port the system picks, through the surfaces a client has:
#
#   executions.protocol lists every state and the moves it allows; the OpenAPI document
#     names the new states; unknown_outcome may not move to running
#   a report of started -> verifying                 -> applied: a legal move
#   a report of unknown_outcome, then started        -> the started is passed over (illegal)
#   a stop asked of an execution of unknown outcome  -> outcome_unknown, nothing moves
#   the reporter's own outcome                       -> reconciled to succeeded
#   the command line lists it by its new state while it waits
#   an event of a newer schema version               -> refused by name, nothing held
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4001.XXXXXX")"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
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

# ---------------------------------------------------------------- the table, self-described
curl -sS --max-time 30 "$U/api/v1/executions/protocol" > "$S/protocol.json" \
  || { echo "    executions.protocol did not answer"; exit 1; }
jq -e '[.states[].state] == ["authorizing","queued","claimed","running","verifying","cancelling","unknown_outcome","succeeded","failed","cancelled","refused"]' \
  "$S/protocol.json" >/dev/null \
  || { echo "    the protocol does not list every state in order:"; jq -c '[.states[]?.state]' "$S/protocol.json"; exit 1; }
moves() { jq -c --arg s "$1" '.states[] | select(.state == $s) | .may_move_to' "$S/protocol.json"; }
[ "$(moves unknown_outcome)" = '["verifying","succeeded","failed","cancelled"]' ] \
  || { echo "    unknown_outcome moves to $(moves unknown_outcome), not only by reconciliation"; exit 1; }
[ "$(moves queued)" = '["claimed","running","failed","cancelled","refused"]' ] \
  || { echo "    queued moves to $(moves queued)"; exit 1; }
jq -e '[.states[] | select(.final) | .state] == ["succeeded","failed","cancelled","refused"]
       and ([.states[] | select(.final) | .may_move_to | length] | add) == 0' "$S/protocol.json" >/dev/null \
  || { echo "    a final state is not final:"; jq -c '[.states[] | select(.final)]' "$S/protocol.json"; exit 1; }
for t in authorizing claimed verifying refused unknown_outcome; do
  jq -e --arg t "execution.$t" '.event_types | index($t)' "$S/protocol.json" >/dev/null \
    || { echo "    the protocol has no event execution.$t"; exit 1; }
done
curl -sS --max-time 30 "$U/openapi.json" > "$S/openapi.json" || { echo "    no OpenAPI document"; exit 1; }
# the state schema is derived from the enum, as an `enum` or as one `const` per documented variant
jq -e '[.. | strings] as $words | all("authorizing", "claimed", "verifying", "unknown_outcome", "refused"; . as $w | $words | index($w))' \
  "$S/openapi.json" >/dev/null \
  || { echo "    the OpenAPI document does not name the new states"; exit 1; }

# ---------------------------------------------------------------- a reported execution
ID="x-$(date -u +%Y%m%dT%H%M%SZ)-4001abcd"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ev() { # sequence type [data] [schema version]
  if [ -n "${3:-}" ]; then
    printf '{"schema_version":"%s","event_id":"%s#%s","execution_id":"%s","sequence":%s,"timestamp":"%s","type":"%s","data":%s}' \
      "${4:-1}" "$ID" "$1" "$ID" "$1" "$STAMP" "$2" "$3"
  else
    printf '{"schema_version":"%s","event_id":"%s#%s","execution_id":"%s","sequence":%s,"timestamp":"%s","type":"%s"}' \
      "${4:-1}" "$ID" "$1" "$ID" "$1" "$STAMP" "$2"
  fi
}
report() { # name events-json
  printf '{"id":"%s","capability":"executions.demonstrate","title":"Demonstrate an execution","input":{"steps":1},"cancellable":true,"pid":%s,"events":[%s]}' \
    "$ID" "$$" "$2" > "$S/$1.in.json"
  curl -sS --max-time 30 -o "$S/$1.json" -w '%{http_code}' -X POST \
    -H 'Content-Type: application/json' --data-binary "@$S/$1.in.json" "$U/api/v1/executions/report" || true
}
check() { # name status jq-expression what
  [ "$(report "$1" "$4")" = "$2" ] || { echo "    $1: answered not $2:"; sed 's/^/      /' "$S/$1.json"; exit 1; }
  jq -e "$3" "$S/$1.json" >/dev/null || { echo "    $1: $5:"; jq -c '{applied,through,state: .execution.state}' "$S/$1.json"; exit 1; }
}
LOST='{"error":{"code":"unavailable","message":"the worker stopped answering","correlation_id":"'"$ID"'"}}'

# legal: started, then verifying
check legal 200 '.applied == 2 and .through == 3 and .execution.state == "verifying"' \
  "$(ev 2 execution.started),$(ev 3 execution.verifying)" "a legal move was not applied"
# the outcome is lost; a rerun is illegal and passed over, and nothing moves
check lost 200 '.applied == 1 and .execution.state == "unknown_outcome" and .execution.error.code == "unavailable"' \
  "$(ev 4 execution.unknown_outcome "$LOST")" "unknown_outcome was not entered"
check rerun 200 '.applied == 0 and .through == 5 and .execution.state == "unknown_outcome"' \
  "$(ev 5 execution.started)" "unknown_outcome moved back to running"

# a stop cannot change what did or did not happen
curl -sS --max-time 30 -o "$S/cancel.json" -X POST -H 'Content-Type: application/json' \
  -d "{\"id\":\"$ID\"}" "$U/api/v1/executions/cancel" || true
jq -e '.outcome == "outcome_unknown" and .execution.state == "unknown_outcome"' "$S/cancel.json" >/dev/null \
  || { echo "    a stop of an execution of unknown outcome answered:"; sed 's/^/      /' "$S/cancel.json"; exit 1; }

# the command line reads it by its new state
"$RB" executions list --repo "$PWD" --state unknown_outcome --format json > "$S/list.json" 2> "$S/list.err" \
  || { echo "    executions list --state unknown_outcome failed:"; sed 's/^/      /' "$S/list.err"; exit 1; }
jq -e --arg id "$ID" '[.executions[] | select(.id == $id and .state == "unknown_outcome")] | length == 1' "$S/list.json" >/dev/null \
  || { echo "    the command line does not list it as unknown_outcome:"; jq -c '[.executions[]? | {id,state}]' "$S/list.json"; exit 1; }

# the reporter's own outcome reconciles it
check reconciled 200 '.applied == 1 and .execution.state == "succeeded" and (.execution | has("error") | not)' \
  "$(ev 6 execution.completed '{"output":{"ok":true}}')" "the outcome did not reconcile it"

# ---------------------------------------------------------------- a newer schema version
ID="x-$(date -u +%Y%m%dT%H%M%SZ)-4001beef"
[ "$(report newer "$(ev 2 execution.started '' 2)")" = 400 ] \
  || { echo "    an event of schema version 2 was taken:"; sed 's/^/      /' "$S/newer.json"; exit 1; }
grep -q 'schema version 2' "$S/newer.json" \
  || { echo "    the refusal does not name the version:"; sed 's/^/      /' "$S/newer.json"; exit 1; }
[ "$(curl -sS --max-time 30 -o /dev/null -w '%{http_code}' "$U/api/v1/executions/get?id=$ID")" = 404 ] \
  || { echo "    a refused report left an execution behind"; exit 1; }

echo "    the protocol lists 11 states and their moves; started->verifying applied; unknown_outcome->running passed over; stop answered outcome_unknown; reconciled to succeeded; schema version 2 refused by name"
