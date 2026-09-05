# majordomus-covers: start check question handover finish history
# majordomus-negative: start check question finish
# majordomus-lifecycle: refused
#
# The adversarial workflow. Every contract line is proved in isolation by 06_finish; what
# this case proves is that they still hold across a sequence of commands — a task that goes
# wrong in three different ways at once, is refused, is repaired one failure at a time, and
# is only then accepted.
#
# The sequence: start, work outside the claimed scope, open a blocking question, ask check,
# be refused by finish, repair each cause, and finish accepted. A contract that holds for
# each command separately can still fail to hold across the boundary between them, which is
# where a real task lives.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib docs
echo a > lib/a; echo d > docs/d
git add . && git commit -qm base

"$MJ" start "narrow the parser" --scope lib >/dev/null
id="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
[ -n "$id" ] || { echo "    no task id after start"; exit 1; }

# --- go wrong in three ways at once -----------------------------------------------------
echo work >> lib/a                       # in scope, legitimate
echo stray >> docs/d                     # outside the claimed scope
expect_exit 0 "$MJ" question add "does the parser need to accept tabs?"
expect_grep "opened for $id"

# check reports the scope violation and the blocker, and exits 10 rather than warning
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +docs/d'
expect_grep 'blockers'

# --- refused, and the refusal names every cause at once ----------------------------------
# Not one failure at a time: a worker who fixes the first refusal and is then refused again
# for a second reason learns the contract one painful round trip at a time.
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'FAIL scope .* outside claimed scope'
expect_grep 'FAIL blockers'
expect_grep 'FAIL note'
expect_grep 'refused'
# the task is untouched by a refusal: it is still active, and still the same task
expect_grep '^outcome: active$' .ai/local/state/current.yaml
expect_grep "^id: $id\$" .ai/local/state/current.yaml
# a refusal for a blocker is not a reason to accept a different outcome silently either
expect_exit 10 "$MJ" finish --outcome completed
expect_grep 'FAIL blockers'

# --- repair, one cause at a time, and watch the findings disappear one at a time ---------
git checkout -- docs/d                                  # 1. the out-of-scope change goes away
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_no_grep 'FAIL scope'
expect_grep 'FAIL blockers'

n="$("$MJ" question list | grep -cE '^[0-9]+  \[unresolved\]' || true)"
[ "$n" -ge 1 ] || { echo "    question list shows no numbered entry to resolve"; exit 1; }
expect_exit 0 "$MJ" question resolve 1 --answer "no; tabs are refused with a message"
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"   # 2. blocker resolved
expect_no_grep 'FAIL blockers'
expect_grep 'FAIL note'

printf '# Objective\nnarrow the parser\n# Current State\ntabs refused with a message\n# Next Action\nnone\n' \
  | "$MJ" handover >/dev/null                            # 3. the note the outcome requires
# verification is still required, and is still not satisfied by asserting it happened
expect_exit 10 "$MJ" finish --outcome completed
expect_grep 'FAIL verification'

# --- accepted, and only now -------------------------------------------------------------
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'finish: .* completed'
expect_grep '^outcome: completed$' .ai/local/state/current.yaml

# --- what the durable record says happened ----------------------------------------------
# The acceptance is recorded with the verification that earned it.
expect_grep "\"event\":\"task.finished\".*\"task_id\":\"$id\".*\"outcome\":\"completed\"" .ai/local/state/ledger.jsonl
expect_exit 0 "$MJ" history --task "$id" --all
expect_grep 'task.started'
expect_grep 'question.opened'
expect_grep 'question.resolved'
expect_grep 'task.handed_over'
expect_grep 'task.finished'

# The four refusals above are absent from that history: finish records an acceptance and
# says nothing when it refuses, so a task refused four times before passing is
# indistinguishable in the ledger from one that passed first time. Recorded here as the
# assertion this case will carry when the event exists; see the finish-refusal-leaves-no-trace
# row in docs/HARDCODING_LEDGER.yaml.
[ "$("$MJ" history --task "$id" --all | grep -c 'finish.refused' || true)" = 0 ] || {
  echo "    finish.refused now exists; add the assertion this comment describes"; exit 1; }
