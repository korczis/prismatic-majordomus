# majordomus-covers: none
# A second worker's record lands on the first worker's task (I0701, milestone M002).
#
# ---------------------------------------------------------------------------------------------
# THIS CASE PINS A DEFECT. IT CHARACTERISES THE GAP; IT DOES NOT BLESS THE BEHAVIOUR.
# When M002 makes a durable record name the worker that wrote it, the assertions marked GAP
# below are expected to turn red, and the change that turns them red rewrites them to assert
# the new attribution. Turning them green again by weakening them is the regression.
# ---------------------------------------------------------------------------------------------
#
# Three sessions collided in one checkout on one evening: one had started a task, and the
# others recorded decisions, checkpoints and questions while it was active. Every record
# was filed under the first session's task, because decision, checkpoint and question take
# the task from the checkout's current-task record (.ai/local/state/current.yaml) and from
# nothing the caller says about itself. The ledger stamps the checkout's open session, not
# the caller's either. So a record says which task was active, never who wrote it.
#
# The sequence, in a disposable repository: worker A starts a task; worker B, a different
# provider session and a different user, records a decision, a checkpoint and a question.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

a() { MAJORDOMUS_PROVIDER_SESSION=worker-a-0701 USER=worker-a-0701 "$@"; }
b() { MAJORDOMUS_PROVIDER_SESSION=worker-b-0701 USER=worker-b-0701 "$@"; }

expect_exit 0 a "$MJ" start "worker A's task" --scope lib
id=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)
[ -n "$id" ] || { echo "    worker A's start left no current task"; exit 1; }

expect_exit 0 b "$MJ" decision add "Worker B chose the narrower fix" --why "recorded by worker B"
expect_exit 0 b "$MJ" question add "Worker B asks whether the old client still runs"
expect_exit 0 b bash -c "echo 'worker B progress' | '$MJ' checkpoint"

# GAP: each record of worker B is filed under worker A's task.
grep -q "^Task: $id\$" .ai/local/state/decisions.md \
  || { echo "    worker B's decision is no longer filed under worker A's task $id (has M002 landed? rewrite this case)"; exit 1; }
grep -q "$id — Worker B asks" .ai/local/state/open-questions.md \
  || { echo "    worker B's question is no longer filed under worker A's task $id (has M002 landed? rewrite this case)"; exit 1; }
cp_file="$(grep -rl 'worker B progress' .ai/local/state/checkpoints | head -n 1)"
[ -n "$cp_file" ] || { echo "    worker B's checkpoint wrote no record"; exit 1; }
grep -q "^task_id: $id\$" "$cp_file" \
  || { echo "    worker B's checkpoint is no longer filed under worker A's task $id (has M002 landed? rewrite this case)"; exit 1; }
for ev in decision.recorded question.opened task.checkpoint; do
  grep "\"event\":\"$ev\"" .ai/local/state/ledger.jsonl | tail -n 1 | grep -q "\"task_id\":\"$id\"" \
    || { echo "    the ledger's $ev for worker B no longer names worker A's task $id"; exit 1; }
done

# GAP: nothing worker B wrote names worker B.
for f in .ai/local/state/decisions.md .ai/local/state/open-questions.md "$cp_file" .ai/local/state/ledger.jsonl; do
  grep -q 'worker-b-0701' "$f" \
    && { echo "    $f now names worker B — the gap this case characterises is closing; rewrite it to assert the attribution"; exit 1; }
done
echo "    ok (gap pinned): worker B's decision, question and checkpoint are filed under worker A's task, and none names worker B"
