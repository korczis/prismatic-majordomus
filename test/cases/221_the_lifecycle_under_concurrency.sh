# majordomus-covers: session capture decision
# majordomus-negative: session
# Ten workers in one checkout, and two checkouts of one repository, all writing the lifecycle
# at once. What must survive: no corruption, no lost pointer, no episode that takes another
# episode's identity, and a state a reader can predict.
#
# THE REGRESSION THIS CASE IS NAMED FOR. `mj_session_point_at` used to remove the pointer and
# then create it, which is two operations with a window between them:
#
#   worker A   rm -f pointer                                  ln -s -> sessions-open/a.yaml
#   worker B                 rm -f pointer   ln -s ... FAILS (A got there first)
#   worker B                                                  cp b.yaml pointer
#
# and that last `cp` follows the symlink A has just created — so B's episode record is
# written *over A's episode file*. Measured against master on 2026-09-11: ten concurrent
# `session start`s in one checkout left ten open episodes carrying seven to nine distinct
# session ids, reproducibly, every round of five. An episode whose identity was overwritten
# does not fail: it goes on stamping the ledger with the winner's id, and closing either key
# publishes one record claiming to be both. That is the fault 111_session_per_provider was
# written to end — one worker's work filed under another worker's episode — arriving through
# the pointer rather than through the store, which is why that case did not see it.
#
# The other half is the ledger. It is append-only and it is the substrate every derived
# session fact is computed from (61_session_envelope), so a concurrent append that interleaves
# two lines is a corruption every later read inherits.
#
# The server half of concurrency — a server restarted while clients are attached — belongs to
# 113_shared_server_survives_a_crash and is not repeated here.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# not what a start event ensures beside the episode; case 108 owns that half
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
# the shims resolve the executable from their own path and then from PATH; without this they
# would drive whatever `majordomus` is installed on the machine running the suite
PATH="$(dirname "$MJ"):$PATH"; export PATH

OPEN=.ai/local/state/sessions-open
PTR=.ai/local/state/session-current.yaml
LEDGER=.ai/local/state/ledger.jsonl
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
opens()       { find "$OPEN" -maxdepth 1 -name '*.yaml' 2>/dev/null | wc -l | tr -d ' '; }
ids()         { cat "$OPEN"/*.yaml 2>/dev/null | sed -n 's/^session_id: //p' | LC_ALL=C sort -u | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- ten opens at once
# Ten windows of a provider attaching to one checkout in the same second. Each is a worker,
# each gets an episode of its own, and each episode keeps the identity it was given.
i=0
while [ "$i" -lt 10 ]; do
  start_event "win-$i" startup >/dev/null 2>&1 &
  i=$((i + 1))
done
wait 2>/dev/null || true
must "ten concurrent opens produced $(opens) episodes, not 10" [ "$(opens)" = 10 ]
[ "$(ids)" = 10 ] || { printf '    ten episodes carry only %s distinct session ids: one worker overwrote another\n' "$(ids)"
                       for f in "$OPEN"/*.yaml; do printf '    | %s %s %s\n' "$(basename "$f")" "$(sed -n 's/^session_id: //p' "$f")" "$(sed -n 's/^provider_session: //p' "$f")"; done
                       exit 1; }
# Each episode carries the identity it is keyed by. This is the assertion that catches the
# overwrite even when the ids happen not to collide: the file named for win-3 must hold
# win-3's episode and nobody else's.
i=0
while [ "$i" -lt 10 ]; do
  got="$(sed -n 's/^provider_session: //p' "$OPEN/win-$i.yaml" | tr -d '"')"
  [ "$got" = "win-$i" ] || { printf '    %s holds the episode of provider session %s\n' "$OPEN/win-$i.yaml" "$got"; exit 1; }
  i=$((i + 1))
done
# The pointer is one of the ten and it is a link into the store, never a copy of a record:
# a copy is a second account of an open episode, which is what the store exists to prevent.
must "the pointer is not a link into the store after ten concurrent opens" [ -L "$PTR" ]
must "the pointer aims at nothing" [ -e "$PTR" ]
target="$(sed -n 's/^session_id: //p' "$PTR" | head -n 1)"
grep -l "^session_id: $target\$" "$OPEN"/*.yaml >/dev/null 2>&1 \
  || { echo "    the pointer names an episode that is not open here"; exit 1; }

# ---------------------------------------------------------------- ten appends at once
# Ten workers writing the ledger. Every line must be whole and every line must be attributed
# to the episode that wrote it — a line stamped with a neighbour's id is a record filed under
# the wrong worker, which is the failure 111 measured in this repository.
before="$(wc -l < "$LEDGER" | tr -d ' ')"
i=0
while [ "$i" -lt 10 ]; do
  ( MAJORDOMUS_PROVIDER_SESSION="win-$i" "$MJ" decision add "Worker $i decided" \
      --why "ten workers appending at once must produce ten whole lines" >/dev/null 2>&1 ) &
  i=$((i + 1))
done
wait 2>/dev/null || true
after="$(wc -l < "$LEDGER" | tr -d ' ')"
must "ten concurrent appends added $((after - before)) ledger lines, not 10" [ "$((after - before))" = 10 ]
# Whole lines. An interleaved append shows up as a line that is not an object, so the check
# is the ledger's own shape rather than a byte count: jq when it is there, a bracket test
# when it is not, because the suite runs where jq may be absent.
if command -v jq >/dev/null 2>&1; then
  jq -e . "$LEDGER" >/dev/null 2>"$T/jq.err" \
    || { echo "    the ledger stopped being one JSON object per line under concurrent appends:"; sed 's/^/    | /' "$T/jq.err"; exit 1; }
else
  awk 'substr($0,1,1) != "{" || substr($0,length($0),1) != "}" { print NR": "$0; bad=1 } END { exit bad+0 }' "$LEDGER" \
    || { echo "    the ledger holds a line that is not a whole object"; exit 1; }
fi
# Attribution. Each decision was written inside its own provider session, so each carries
# that episode's stamp and no other.
i=0
while [ "$i" -lt 10 ]; do
  want="$(sed -n 's/^session_id: //p' "$OPEN/win-$i.yaml" | head -n 1)"
  grep -F "\"session\":\"$want\"" "$LEDGER" | grep -qF "Worker $i decided" \
    || { printf '    the decision of worker %s is not stamped with its own episode %s\n' "$i" "$want"
         grep -F "Worker $i decided" "$LEDGER" | sed 's/^/    | /'; exit 1; }
  i=$((i + 1))
done

# ---------------------------------------------------------------- ten closes at once
# Every episode ends, each end event closes its own and no other, and the store is left with
# exactly one record each. 220 owns the two-closes-of-one-episode race; this is the other
# shape: ten different episodes closing together through one store.
i=0
while [ "$i" -lt 10 ]; do
  end_event "win-$i" clear >/dev/null 2>&1 &
  i=$((i + 1))
done
wait 2>/dev/null || true
must "ten concurrent closes left $(opens) episodes open" [ "$(opens)" = 0 ]
must "closing the last episode left the pointer behind" [ ! -e "$PTR" ]
must "closing the last episode left a dangling pointer" [ ! -L "$PTR" ]
n="$(find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' ')"
must "ten episodes produced $n records, not 10" [ "$n" = 10 ]
must "the concurrent closes left staging files in the tracked store" \
  [ "$(find .ai/repo/sessions -maxdepth 1 -name '.tmp.*' 2>/dev/null | wc -l | tr -d ' ')" = 0 ]
# Ten episodes, ten identities. `doctor` refuses a store where two records claim one, and
# this is the read-back that proves the ten records are ten episodes and not one repeated.
must "ten records carry $(grep -h '^session_id: ' .ai/repo/sessions/2*.md | LC_ALL=C sort -u | wc -l | tr -d ' ') distinct identities" \
  [ "$(grep -h '^session_id: ' .ai/repo/sessions/2*.md | LC_ALL=C sort -u | wc -l | tr -d ' ')" = 10 ]

# ---------------------------------------------------------------- two worktrees at once
# The local half is per checkout, so two worktrees of one repository have two stores and must
# not see each other's open episodes. The pointer of one is not the pointer of the other, and
# a record written over there is never offered as context here (`different_context` exists for
# exactly this and is not re-proved; 62_session_divergence owns the vocabulary).
git add -A >/dev/null 2>&1; git commit -qm "state" >/dev/null 2>&1 || true
git worktree add -q "$T/wt" -b other >/dev/null 2>&1 || git worktree add -q "$T/wt" -b other
( cd "$T/wt" && "$MJ" session start --provider-session shared-key --owner other >/dev/null 2>&1 ) &
"$MJ" session start --provider-session shared-key --owner here >/dev/null 2>&1 &
wait 2>/dev/null || true
must "the second worktree's episode landed in this checkout's store" [ "$(opens)" = 1 ]
here="$(sed -n 's/^session_id: //p' "$OPEN/shared-key.yaml" | head -n 1)"
there="$(sed -n 's/^session_id: //p' "$T/wt/.ai/local/state/sessions-open/shared-key.yaml" | head -n 1)"
must "two worktrees sharing one provider-session key were given one episode" [ "$here" != "$there" ]
must "the other worktree's episode names this worktree" \
  [ "$(sed -n 's/^worktree: //p' "$T/wt/.ai/local/state/sessions-open/shared-key.yaml" | head -n 1)" != "$(pwd -P)" ]
# and this checkout's status is about this checkout
expect_exit 0 "$MJ" session status
expect_grep "Session: +$here"
expect_no_grep "$there"
