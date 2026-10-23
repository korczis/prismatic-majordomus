# majordomus-covers: capability:executions.start
# majordomus-timeout: 300
# A repeated start with one idempotency key is one execution (I4000, Mission Control B1).
#
# A double click on Implement, or a retry after a timeout whose answer never arrived, sends
# the same start twice. Before I4000 each send started its own execution; a start that changes
# the repository would then run twice. Proven here from outside the executable, against a real
# server on a port the system picks:
#
#   the same key and input, sent again               -> 200, the same id, deduplicated: true
#   the same key and input, sent twice at one instant -> one execution
#   the same key with another input                  -> 409 conflict, naming the first id
#   no key, sent twice                               -> two executions, as before
#   a key that is not one (a space, too long)        -> 400 invalid_input, nothing started
#   `majordomus run --idempotency-key` with a bad key -> refused before anything runs
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4000.XXXXXX")"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

# the server, from a copy of the executable, on loopback and a port the system picks; its
# address is read from the lease it publishes (the harness of case 3861)
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

# status of a start, its body in $S/<name>.json
start() { # name body
  curl -sS --max-time 30 -o "$S/$1.json" -w '%{http_code}' -X POST \
    -H 'Content-Type: application/json' -d "$2" "$U/api/v1/executions/start" || true
}
expect() { # name status
  got="$(start "$1" "$3")"
  [ "$got" = "$2" ] || { echo "    $1: answered $got, not $2:"; sed 's/^/      /' "$S/$1.json"; exit 1; }
}
DEMO='"capability":"executions.demonstrate"'

# ---------------------------------------------------------------- the same key, sent again
expect first 200 "{$DEMO,\"input\":{\"steps\":1,\"delay_ms\":0},\"idempotency_key\":\"click-1\"}"
FIRST="$(jq -r .id "$S/first.json")"
jq -e '.deduplicated == false and .idempotency_key == "click-1"' "$S/first.json" >/dev/null \
  || { echo "    the first start is not marked as the one that began it:"; cat "$S/first.json"; exit 1; }
# the same input with its keys in another order is the same input
expect again 200 "{\"idempotency_key\":\"click-1\",$DEMO,\"input\":{\"delay_ms\":0,\"steps\":1}}"
jq -e --arg id "$FIRST" '.id == $id and .deduplicated == true' "$S/again.json" >/dev/null \
  || { echo "    a repeated start was not answered $FIRST, deduplicated:"; cat "$S/again.json"; exit 1; }

# ---------------------------------------------------------------- two at one instant
body="{$DEMO,\"input\":{\"steps\":1,\"delay_ms\":0},\"idempotency_key\":\"click-2\"}"
start race-a "$body" > "$S/race-a.status" &
A=$!
start race-b "$body" > "$S/race-b.status" &
B=$!
wait "$A" "$B"
for r in race-a race-b; do
  [ "$(cat "$S/$r.status")" = 200 ] || { echo "    $r answered $(cat "$S/$r.status")"; cat "$S/$r.json"; exit 1; }
done
[ "$(jq -r .id "$S/race-a.json")" = "$(jq -r .id "$S/race-b.json")" ] \
  || { echo "    two starts sent at once with one key were answered two ids"; exit 1; }
[ "$(jq -s '[.[].deduplicated] | sort' "$S/race-a.json" "$S/race-b.json" | jq -c .)" = '[false,true]' ] \
  || { echo "    exactly one of the two simultaneous starts must have begun it"; exit 1; }

# ---------------------------------------------------------------- another input: a conflict
expect conflict 409 "{$DEMO,\"input\":{\"steps\":2,\"delay_ms\":0},\"idempotency_key\":\"click-1\"}"
jq -e --arg id "$FIRST" '.error.code == "conflict" and (.error.message | contains($id))' "$S/conflict.json" >/dev/null \
  || { echo "    the conflict does not name $FIRST:"; cat "$S/conflict.json"; exit 1; }

# ---------------------------------------------------------------- no key: unchanged
expect plain-a 200 "{$DEMO,\"input\":{\"steps\":1,\"delay_ms\":0}}"
expect plain-b 200 "{$DEMO,\"input\":{\"steps\":1,\"delay_ms\":0}}"
[ "$(jq -r .id "$S/plain-a.json")" != "$(jq -r .id "$S/plain-b.json")" ] \
  || { echo "    two starts without a key were one execution"; exit 1; }
jq -e '.deduplicated == false and (has("idempotency_key") | not)' "$S/plain-b.json" >/dev/null \
  || { echo "    a start without a key says otherwise:"; cat "$S/plain-b.json"; exit 1; }

# ---------------------------------------------------------------- a key that is not one
for key in 'two words' "$(printf 'k%.0s' $(seq 1 129))"; do
  expect bad-key 400 "{$DEMO,\"input\":{\"steps\":1,\"delay_ms\":0},\"idempotency_key\":\"$key\"}"
  jq -e '.error.code == "invalid_input"' "$S/bad-key.json" >/dev/null \
    || { echo "    the key '$key' was not refused as invalid input:"; cat "$S/bad-key.json"; exit 1; }
done

# ---------------------------------------------------------------- what the server holds
curl -sS --max-time 30 "$U/api/v1/executions?capability=executions.demonstrate" > "$S/list.json"
held="$(jq '.executions | length' "$S/list.json")"
[ "$held" = 4 ] || { echo "    the server holds $held executions, not 4 (click-1, click-2, two without a key):"; jq -c '[.executions[] | {id, idempotency_key}]' "$S/list.json"; exit 1; }
[ "$(jq '[.executions[] | select(.idempotency_key == "click-2")] | length' "$S/list.json")" = 1 ] \
  || { echo "    the key sent twice at once holds more than one execution"; exit 1; }

# ---------------------------------------------------------------- the command line
if "$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":1,"delay_ms":0}' \
     --idempotency-key 'two words' --quiet > "$S/run.out" 2> "$S/run.err"; then
  echo "    majordomus run took an idempotency key that is not one"; exit 1
fi
grep -q 'idempotency key' "$S/run.err" \
  || { echo "    majordomus run did not say why it refused the key:"; sed 's/^/      /' "$S/run.err"; exit 1; }

echo "    a repeated start is answered the execution it began, at once or later; another input with the key is a 409; no key starts one each time"
exit 0
