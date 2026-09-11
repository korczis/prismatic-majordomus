# majordomus-covers: checkpoint
# majordomus-negative: checkpoint
# Checkpoints: creation, the cap that distinguishes them from handovers, computed identity,
# task association, resolution, and the ledger record that makes them visible to history.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# no task — the record is the episode's, so it is written, and the task field says `none`
#
# This asserted `exit 12, no active task` until ADR 0052. The refusal was the defect: it
# meant that finishing a task turned progress records off for every episode after it, and
# between 2026-09-05 and 2026-09-11 this repository wrote no checkpoint at all while every
# health check passed. A checkpoint belongs to the episode; a task is an optional relation
# it names when it has one. `test/cases/130` keeps the whole outage.
expect_exit 0 "$MJ" checkpoint --list
expect_grep 'no checkpoint records for this worktree'
expect_exit 0 bash -c "echo progress | '$MJ' checkpoint"
first="$(find .ai/local/state/checkpoints -name '*.md' | head -n 1)"
[ -n "$first" ] || { echo "    a checkpoint outside a task wrote no record"; exit 1; }
grep -q '^task_id: none' "$first" || { echo "    the record does not say it belongs to no task"; exit 1; }
# and the event it emits carries no task_id at all: every reader of this event collects the
# field's distinct values as task identifiers, and a literal "none" would enter a session
# record's task list as though somebody had opened a task by that name
grep '"event":"task.checkpoint"' .ai/local/state/ledger.jsonl | tail -n 1 | grep -q '"task_id"' \
  && { echo "    a checkpoint outside a task emitted a task_id anyway"; exit 1; }
rm -f "$first"

"$MJ" start "t1" --scope lib >/dev/null
id=$(sed -n 's/^id: //p' .ai/local/state/current.yaml)

# an empty body updates checkpoint_at and writes no file
old=$(sed -n 's/^checkpoint_at: //p' .ai/local/state/current.yaml); sleep 1
expect_exit 0 bash -c "printf '' | '$MJ' checkpoint"
expect_grep 'no body'
[ "$(sed -n 's/^checkpoint_at: //p' .ai/local/state/current.yaml)" != "$old" ]
[ "$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]
expect_grep '"event":"task.checkpoint"' .ai/local/state/ledger.jsonl

# identity fields in the body are refused: prose must not forge what git computes
expect_exit 10 bash -c "printf 'progress\nhead: deadbeef\n' | '$MJ' checkpoint"
expect_grep 'computed'
[ "$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]

# over the cap: refused, and it says to write a handover instead
sed -i.bak 's/^  max_body_lines: 40/  max_body_lines: 3/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 10 bash -c "printf 'a\nb\nc\nd\ne\n' | '$MJ' checkpoint"
expect_grep 'is 5 lines, cap 3'
expect_grep 'handover instead'
[ "$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]
sed -i.bak 's/^  max_body_lines: 3/  max_body_lines: 40/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak

# a body is written with computed front matter, mode 0600, and is never staged
echo b >> lib/a
expect_exit 0 bash -c "printf 'reproduced the fault\nnext: write the test\n' | '$MJ' checkpoint"
f="$LAST_OUT"; [ -f "$f" ]
[ "$(file_mode "$f")" = 600 ]
expect_grep '^head: '"$(git rev-parse HEAD)"'$' "$f"
expect_grep '^task_id: '"$id"'$' "$f"
expect_grep '^working_tree: dirty$' "$f"
expect_grep '^  - lib/a$' "$f"
expect_grep '^reproduced the fault$' "$f"
[ -z "$(git diff --cached --name-only)" ]
expect_grep "\"event\":\"task.checkpoint\".*\"checkpoint_path\":\"$f\"" .ai/local/state/ledger.jsonl

# --show resolves the newest for this task; --list shows it with a git label
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^Git state: exact'
expect_grep '^reproduced the fault$'
sleep 1
expect_exit 0 bash -c "printf 'second note\n' | '$MJ' checkpoint"
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^second note$'
expect_exit 0 "$MJ" checkpoint --show --path
expect_grep '^.ai/local/state/checkpoints/.*\.md$'
expect_exit 0 "$MJ" checkpoint --list
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'checkpoints/')" = 2 ]

# git moving on is reported, not hidden
git commit -qam more
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^Git state: advanced'

# a checkpoint belongs to its task: after a new task starts, the old ones do not resolve
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
git add -A; git commit -qm t1 --allow-empty   # local state is ignored; the commit only moves git on
expect_exit 0 "$MJ" start "t2" --scope lib
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^No checkpoint for t-'
# but they are still listed, because the store is append-only
expect_exit 0 "$MJ" checkpoint --list
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'checkpoints/')" = 2 ]

# A handed-over task does not refuse a checkpoint.
#
# This asserted `exit 15, handed_over` until ADR 0052, and that refusal is the whole of the
# 2026-09-05 outage in one line: a task marked `handed_over` on the 5th and never replaced
# silenced this repository's progress records for six days, while episodes kept opening and
# closing and every health check passed. The episode that goes on working after a task is
# handed over is still working, and what it is doing is still worth recording. The task's
# own `checkpoint_at` is what must not move — that field is the task's, and the task is
# over.
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
grep -q '^outcome: handed_over' .ai/local/state/current.yaml \
  || { echo "    the handover did not hand the task over, so this asserts nothing"; exit 1; }
before="$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')"
expect_exit 0 bash -c "echo late | '$MJ' checkpoint"
after="$(find .ai/local/state/checkpoints -name '*.md' | wc -l | tr -d ' ')"
[ "$after" -gt "$before" ] || { echo "    a handed-over task suppressed the episode's checkpoint"; exit 1; }
