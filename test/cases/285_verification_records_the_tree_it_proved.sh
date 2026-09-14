# majordomus-covers: finish
# majordomus-negative: finish
#
# A recorded verification names the tree it ran over, and a tree that moved under the run
# is not a verified tree. Without this, `--verify-command` records an exit code and a
# duration that describe no particular state of the repository: in a checkout several
# workers share, the tree the command saw and the tree that gets committed are routinely
# not the same one.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null; }

"$MJ" start "t1" --scope lib >/dev/null
echo b >> lib/a
note
# A verification that leaves the tree as it found it is recorded with that tree's id.
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'OK +verification .*exit 0, [0-9]+s, tree [0-9a-f]{12}'
expect_grep '"verify":\{"command":"true","exit":0,"seconds":[0-9]+,"tree":"[0-9a-f]{40}"\}' .ai/local/state/ledger.jsonl
tree1="$(sed -n 's/.*"tree":"\([0-9a-f]*\)".*/\1/p' .ai/local/state/ledger.jsonl | tail -n1)"
[ "$tree1" = "$(idx="$(mktemp -u "${TMPDIR:-/tmp}/mj-t.XXXXXX")"; GIT_INDEX_FILE="$idx" git read-tree HEAD && GIT_INDEX_FILE="$idx" git add -A && GIT_INDEX_FILE="$idx" git write-tree; rm -f "$idx")" ] \
  || { printf '    the recorded tree is not the tree that is here: %s\n' "$tree1"; exit 1; }

git add -A && git commit -qm t1
"$MJ" start "t2" --scope lib >/dev/null
id2="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
echo c >> lib/a
note
# A verification that writes into the tree under itself proves nothing about either state.
expect_exit 10 "$MJ" finish --outcome completed --verify-command "echo x >> lib/a"
expect_grep 'FAIL verification .*the tree changed while it ran'
expect_grep 'finish: refused'
expect_grep '^outcome: active$' .ai/local/state/current.yaml
expect_no_grep "\"task_id\":\"$id2\",\"outcome\":\"completed\"" .ai/local/state/ledger.jsonl
# Once the tree settles, the same task finishes and records the settled tree.
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
tree2="$(sed -n 's/.*"tree":"\([0-9a-f]*\)".*/\1/p' .ai/local/state/ledger.jsonl | tail -n1)"
[ "$tree2" != "$tree1" ] || { printf '    the second verification recorded the first tree\n'; exit 1; }

# A failing verification still refuses, and records nothing about a tree it did not prove.
git add -A && git commit -qm t2
"$MJ" start "t3" --scope lib >/dev/null
echo d >> lib/a
note
expect_exit 10 "$MJ" finish --outcome completed --verify-command "false"
expect_grep 'FAIL verification .*false — exit 1'
expect_no_grep 'the tree changed while it ran'
