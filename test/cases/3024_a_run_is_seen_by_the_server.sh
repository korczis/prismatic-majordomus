# majordomus-covers: none
# An execution a person starts is an execution the server can see (I1501).
#
# `majordomus run` executes in its own process. Before I1501 the server serving the same
# checkout had a store of its own that nothing but executions.start wrote to, so with a run
# in flight that server answered 0 executions, and /cockpit/executions and the live channel
# showed a plane nobody wrote to. Now the run reports itself to the server already serving
# the checkout (executions.report), and with no server it says on stderr that nothing else
# can see it. What must stay true, each proved by asking the server, never by reading code:
#
#   1  with no server, the run still runs and succeeds, and says so in one plain line;
#   2  with a server, the server's execution list holds the run WHILE it is in flight —
#      polled, running, started by the command line, with the pid of the run's process —
#      and after it the server holds the same outcome the run printed;
#   3  a stop asked of the server reaches the run, which ends cancelled in both places;
#   4  a run never starts a server: once the server is gone, the plain line is back.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"

RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3024.XXXXXX")"
SRV=""; RUN=""
trap '[ -n "$RUN" ] && kill "$RUN" 2>/dev/null; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

NOT_VISIBLE='no server is serving this repository, so this execution is visible to this process only'

# ---------------------------------------------------------------- 1. no server
"$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":1,"delay_ms":0}' --format json \
  > "$S/alone.json" 2> "$S/alone.err" \
  || { echo "    a run with no server failed"; cat "$S/alone.err"; exit 1; }
[ "$(jq -r .state "$S/alone.json")" = succeeded ] \
  || { echo "    a run with no server did not succeed"; cat "$S/alone.json"; exit 1; }
[ "$(grep -c -F "$NOT_VISIBLE" "$S/alone.err")" = 1 ] \
  || { echo "    a run with no server did not say, once, that nothing else can see it:"; sed 's/^/    | /' "$S/alone.err"; exit 1; }

# ---------------------------------------------------------------- 2. a server, polled in flight
serve_up "$S/out.txt" "$S/err.txt" || exit 1
get() { curl -s -m 30 "$U$1"; }

# six steps of half a second: three seconds in flight
"$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":6,"delay_ms":500}' --format json \
  > "$S/run.json" 2> "$S/run.err" &
RUN=$!
SEEN=""; i=0
while [ -z "$SEEN" ]; do
  get '/api/v1/executions?capability=executions.demonstrate' > "$S/inflight.json"
  SEEN="$(jq -r --argjson pid "$RUN" \
    '.executions[] | select(.actor.kind == "cli" and .actor.pid == $pid and .state == "running") | .id' \
    "$S/inflight.json" | head -n 1)"
  [ -n "$SEEN" ] && break
  kill -0 "$RUN" 2>/dev/null || { echo "    the run ended before the server listed it running:"; jq -c . "$S/inflight.json"; sed 's/^/    | /' "$S/run.err"; exit 1; }
  i=$((i+1)); [ "$i" -lt 100 ] || { echo "    the server never listed the run in 10 s:"; jq -c . "$S/inflight.json"; exit 1; }
  sleep 0.1
done
# the observation was made in flight: the run's process is still going, and the server says
# so with the counts a control plane shows
kill -0 "$RUN" 2>/dev/null || { echo "    the run had already ended when it was seen"; exit 1; }
jq -e '.active >= 1 and .remembered >= 1' "$S/inflight.json" >/dev/null \
  || { echo "    the server lists the run and counts nothing active:"; jq -c '{count,remembered,active}' "$S/inflight.json"; exit 1; }
# and the command line that reads the server sees the same, in flight
"$RB" executions list --repo "$PWD" > "$S/cli-list.txt" 2>&1 \
  || { echo "    executions list failed"; cat "$S/cli-list.txt"; exit 1; }
grep -q "^running .*$SEEN" "$S/cli-list.txt" \
  || { echo "    executions list does not show the run in flight:"; sed 's/^/    | /' "$S/cli-list.txt"; exit 1; }

rc=0; wait "$RUN" || rc=$?; RUN=""
[ "$rc" = 0 ] || { echo "    the reported run exited $rc"; sed 's/^/    | /' "$S/run.err"; exit 1; }
grep -q -F "$NOT_VISIBLE" "$S/run.err" \
  && { echo "    a run the server saw said nothing else could see it"; exit 1; }
[ "$(jq -r .id "$S/run.json")" = "$SEEN" ] \
  || { echo "    the server listed $SEEN, the run printed $(jq -r .id "$S/run.json")"; exit 1; }
get "/api/v1/executions/get?id=$SEEN" > "$S/after.json"
jq -e '.state == "succeeded" and .output.steps == 6 and (.steps | length) == 6 and .finished_at != null' "$S/after.json" >/dev/null \
  || { echo "    the server does not hold the run's outcome:"; jq -c '{state,output,steps:(.steps|length)}' "$S/after.json"; exit 1; }
[ "$(jq -c .output "$S/after.json")" = "$(jq -c .output "$S/run.json")" ] \
  || { echo "    the server's output differs from the run's:"; jq -c .output "$S/after.json" "$S/run.json"; exit 1; }
# the history is the run's story, in order, with dense sequences
get "/api/v1/executions/events?id=$SEEN" > "$S/history.json"
jq -e '[.events[].type] as $t | ($t[0] == "execution.created") and ($t | index("execution.started")) and ($t[-1] == "execution.completed")' "$S/history.json" >/dev/null \
  || { echo "    the server's history is not the run's:"; jq -c '[.events[].type]' "$S/history.json"; exit 1; }
jq -e '[.events[].sequence] as $s | $s == [range(1; ($s | length) + 1)]' "$S/history.json" >/dev/null \
  || { echo "    the reported sequences are not dense:"; jq -c '[.events[].sequence]' "$S/history.json"; exit 1; }
# and the Cockpit says who ran it
curl -s -m 30 -H 'Accept: text/html' "$U/cockpit/executions/$SEEN" > "$S/page.html"
grep -q "process $(jq -r .actor.pid "$S/after.json")" "$S/page.html" \
  || { echo "    the Cockpit page does not name the process that ran it"; exit 1; }

# ---------------------------------------------------------------- 3. a stop asked of the server
"$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":40,"delay_ms":300}' --format json \
  > "$S/long.json" 2> "$S/long.err" &
RUN=$!
LONG=""; i=0
until [ -n "$LONG" ]; do
  LONG="$(get '/api/v1/executions?state=running' | jq -r --argjson pid "$RUN" '.executions[] | select(.actor.pid == $pid) | .id' | head -n 1)"
  [ -n "$LONG" ] && break
  i=$((i+1)); [ "$i" -lt 100 ] || { echo "    the long run was never listed"; exit 1; }
  sleep 0.1
done
curl -s -m 30 -X POST -H 'Content-Type: application/json' -d "{\"id\":\"$LONG\"}" "$U/api/v1/executions/cancel" > "$S/cancel.json"
[ "$(jq -r .outcome "$S/cancel.json")" = requested ] \
  || { echo "    the server did not take the stop:"; cat "$S/cancel.json"; exit 1; }
rc=0; wait "$RUN" || rc=$?; RUN=""
# the run's own exit code for an execution that did not succeed
[ "$rc" = 10 ] || { echo "    the stopped run exited $rc, not 10"; sed 's/^/    | /' "$S/long.err"; exit 1; }
[ "$(jq -r .state "$S/long.json")" = cancelled ] \
  || { echo "    the run did not stop when the server was asked: $(jq -r .state "$S/long.json")"; exit 1; }
[ "$(jq -r '.steps | length' "$S/long.json")" -lt 40 ] || { echo "    the run went to its end"; exit 1; }
[ "$(get "/api/v1/executions/get?id=$LONG" | jq -r .state)" = cancelled ] \
  || { echo "    the server does not hold the stop's outcome"; exit 1; }

# ---------------------------------------------------------------- 4. a run never starts a server
serve_down
"$RB" run executions.demonstrate --repo "$PWD" --input '{"steps":1,"delay_ms":0}' --format json \
  > "$S/gone.json" 2> "$S/gone.err" \
  || { echo "    a run after the server went failed"; cat "$S/gone.err"; exit 1; }
grep -q -F "$NOT_VISIBLE" "$S/gone.err" \
  || { echo "    a run after the server went did not say nothing else can see it:"; sed 's/^/    | /' "$S/gone.err"; exit 1; }

echo "    a run is listed by the server serving its checkout while it runs, a stop asked there reaches it, and with no server it says nothing else can see it"
