# majordomus-covers: start check
# majordomus-negative: start
# claims: scoped-task
# `start` refuses a task, --owner or --because the task record cannot hold, and writes nothing
# (I3310, evidence case_start_refuses_unsafe_scalars). The record keeps each of them as one
# scalar on one line, and the layer's YAML readers refuse a multi-line scalar and a tab, so
# before this a newline in the task wrote a current.yaml the next check could not parse. Every
# other character is held: a quote, a backslash and a non-ASCII letter come back from `check`
# exactly as they were given, where a double-quoted `\"` used to come back as `\"`.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth && echo x > lib/auth/a.txt && git add . && git commit -qm base
CUR=.ai/local/state/current.yaml
LEDGER=.ai/local/state/ledger.jsonl
nl='
'

# each refusal is a usage error (2), names what it refused, and leaves no record behind
expect_exit 2 "$MJ" start "first line${nl}second line" --scope lib/auth
expect_grep 'start: the task holds a line break'
[ ! -e "$CUR" ] || { echo "    a refused start wrote $CUR"; exit 1; }
expect_exit 2 "$MJ" start "trailing newline${nl}" --scope lib/auth
expect_grep 'start: the task holds'
expect_exit 2 "$MJ" start "$(printf 'a\tb')" --scope lib/auth
expect_grep 'start: the task holds'
expect_exit 2 "$MJ" start "$(printf 'carriage\rreturn')" --scope lib/auth
expect_grep 'start: the task holds'
expect_exit 2 "$MJ" start "$(printf 'escape\033[0m')" --scope lib/auth
expect_grep 'start: the task holds'
expect_exit 2 "$MJ" start "fine task" --scope lib/auth --owner "alice${nl}mallory"
expect_grep 'start: --owner holds a line break'
expect_exit 2 "$MJ" start "fine task" --scope lib/auth --exempt maintenance --because "one${nl}two"
expect_grep 'start: --because holds a line break'
[ ! -e "$CUR" ] || { echo "    a refused start wrote $CUR"; exit 1; }
expect_no_grep '"event":"task.started"' "$LEDGER"

# a quote, a backslash and a non-ASCII letter are stored and read back unchanged by check
task='say "hi" to C:\temp\ and Příliš žluťoučký kůň'"'"'s # not a comment \'
owner='O'"'"'Brien "ob" \x'
expect_exit 0 "$MJ" start "$task" --scope lib/auth --owner "$owner"
expect_file "$CUR"
expect_exit 0 "$MJ" check
expect_exit 0 "$MJ" check --explain
grep -qxF -- "  task=$task" <<<"$LAST_OUT" || { printf '    the task did not come back unchanged:\n%s\n' "$LAST_OUT"; exit 1; }
grep -qxF -- "  owner=$owner" <<<"$LAST_OUT" || { printf '    the owner did not come back unchanged:\n%s\n' "$LAST_OUT"; exit 1; }

# a refused start does not archive or replace a finished record either
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
before="$(cat "$CUR")"; started="$(grep -c '"event":"task.started"' "$LEDGER")"
expect_exit 2 "$MJ" start "next${nl}task" --scope lib/auth
[ "$(cat "$CUR")" = "$before" ] || { echo "    a refused start changed $CUR"; exit 1; }
[ ! -d .ai/local/state/archive ] || [ -z "$(ls -A .ai/local/state/archive)" ] \
  || { echo "    a refused start archived the finished record"; exit 1; }
[ "$(grep -c '"event":"task.started"' "$LEDGER")" = "$started" ] \
  || { echo "    a refused start appended task.started"; exit 1; }
