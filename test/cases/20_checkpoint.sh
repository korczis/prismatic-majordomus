# majordomus-covers: checkpoint
# majordomus-negative: checkpoint
# Checkpoints: creation, the cap that distinguishes them from handovers, computed identity,
# task association, resolution, and the ledger record that makes them visible to history.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# no task
expect_exit 12 bash -c "echo progress | '$MJ' checkpoint"
expect_grep 'no active task'
expect_exit 0 "$MJ" checkpoint --list
expect_grep 'no checkpoint records for this worktree'

"$MJ" start "t1" --scope lib >/dev/null
id=$(sed -n 's/^id: //p' .majordomus/state/current.yaml)

# an empty body updates checkpoint_at and writes no file
old=$(sed -n 's/^checkpoint_at: //p' .majordomus/state/current.yaml); sleep 1
expect_exit 0 bash -c "printf '' | '$MJ' checkpoint"
expect_grep 'no body'
[ "$(sed -n 's/^checkpoint_at: //p' .majordomus/state/current.yaml)" != "$old" ]
[ "$(find .majordomus/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]
expect_grep '"event":"task.checkpoint"' .majordomus/state/ledger.jsonl

# identity fields in the body are refused: prose must not forge what git computes
expect_exit 10 bash -c "printf 'progress\nhead: deadbeef\n' | '$MJ' checkpoint"
expect_grep 'computed'
[ "$(find .majordomus/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]

# over the cap: refused, and it says to write a handover instead
sed -i.bak 's/^  max_body_lines: 40/  max_body_lines: 3/' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak
expect_exit 10 bash -c "printf 'a\nb\nc\nd\ne\n' | '$MJ' checkpoint"
expect_grep 'is 5 lines, cap 3'
expect_grep 'handover instead'
[ "$(find .majordomus/state/checkpoints -name '*.md' | wc -l | tr -d ' ')" = 0 ]
sed -i.bak 's/^  max_body_lines: 3/  max_body_lines: 40/' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak

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
expect_grep "\"event\":\"task.checkpoint\".*\"checkpoint_path\":\"$f\"" .majordomus/state/ledger.jsonl

# --show resolves the newest for this task; --list shows it with a git label
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^Git state: exact'
expect_grep '^reproduced the fault$'
sleep 1
expect_exit 0 bash -c "printf 'second note\n' | '$MJ' checkpoint"
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^second note$'
expect_exit 0 "$MJ" checkpoint --show --path
expect_grep '^.majordomus/state/checkpoints/.*\.md$'
expect_exit 0 "$MJ" checkpoint --list
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'checkpoints/')" = 2 ]

# git moving on is reported, not hidden
git commit -qam more
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^Git state: advanced'

# a checkpoint belongs to its task: after a new task starts, the old ones do not resolve
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
git add -A && git commit -qm t1
expect_exit 0 "$MJ" start "t2" --scope lib
expect_exit 0 "$MJ" checkpoint --show
expect_grep '^No checkpoint for t-'
# but they are still listed, because the store is append-only
expect_exit 0 "$MJ" checkpoint --list
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'checkpoints/')" = 2 ]

# a finished task refuses a checkpoint: progress inside a task that has none is nonsense
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
expect_exit 15 bash -c "echo late | '$MJ' checkpoint"
expect_grep 'handed_over'
