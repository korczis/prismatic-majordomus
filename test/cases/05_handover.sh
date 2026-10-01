# majordomus-covers: handover
# majordomus-negative: handover start
# claims: handover-record, no-transcripts
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
# no task
expect_exit 12 bash -c "printf '# Objective\nx\n# Current State\ny\n# Next Action\nz\n' | '$MJ' handover"
"$MJ" start "t1" --scope lib >/dev/null
# missing sections named
expect_exit 10 bash -c "printf '# Objective\nx\n# Current State\n\n' | '$MJ' handover"
expect_grep 'missing or empty section\(s\): .*Current State'
expect_grep 'Next Action'
# template placeholders count as empty
expect_exit 10 bash -c "printf '# Objective\n<one paragraph>\n# Current State\nreal\n# Next Action\nreal\n' | '$MJ' handover"
# identity fields in body rejected
expect_exit 10 bash -c "printf '# Objective\nx\nhead: abc\n# Current State\ny\n# Next Action\nz\n' | '$MJ' handover"
expect_grep 'computed'
# success
echo b >> lib/a
expect_exit 0 bash -c "printf '# Objective\nfix it\n# Current State\nhalf done\n# Next Action\nfinish it\n' | '$MJ' handover"
f="$LAST_OUT"; [ -f "$f" ]
[ "$(file_mode "$f")" = 600 ]
expect_grep '^head: '"$(git rev-parse HEAD)"'$' "$f"
expect_grep '^working_tree: dirty$' "$f"
expect_grep '^  - lib/a$' "$f"
expect_grep '^task_id: t-' "$f"
expect_grep '^# Next Action$' "$f"
expect_grep '"event":"task.handed_over"' .ai/local/state/ledger.jsonl
git check-ignore -q "$f" || { echo "    a handover record is not ignored; local state would travel"; exit 1; }   # present, ignored, never staged
[ -z "$(git diff --cached --name-only)" ]
# task still active (no --close), start refuses
expect_exit 15 "$MJ" start "t2" --scope lib
# resolve: exact now, advanced after a commit, none on another branch
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Match: same_worktree_same_branch'
expect_grep '^Git state: exact'
expect_grep '^# Objective'
# the verdict, not only the age: a record written a moment ago is fresh and is not history...
expect_grep '^Freshness: fresh'
expect_no_grep '^History:'
# ...and the same record, aged past the policy's stale threshold, is said to be history
expect_exit 0 "$MJ" handover --resolve --path
rec="$LAST_OUT"
cp "$rec" "$T/record.keep"
sed 's/^created_at: .*/created_at: 2020-01-01T00:00:00Z/' "$T/record.keep" > "$rec"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Freshness: stale — .*past the .* this repository calls stale'
expect_grep '^History: read this record as context'
cp "$T/record.keep" "$rec"
git commit -qam more
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Git state: advanced'
expect_exit 0 "$MJ" handover --resolve --path
expect_grep '^.ai/local/state/handovers/.*\.md$'
git checkout -qb other
expect_exit 0 "$MJ" handover --resolve
expect_grep '^No relevant handover.$'
git checkout -q -
# newest wins within the same tier
sleep 1
expect_exit 0 bash -c "printf '# Objective\nsecond\n# Current State\ns\n# Next Action\ns\n' | '$MJ' handover --close"
# append-only: the second handover is a new record, and the first is still there, byte for byte
[ "$(ls .ai/local/state/handovers/*.md | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    a second handover did not add a record beside the first"; ls .ai/local/state/handovers; exit 1; }
cmp -s "$rec" "$T/record.keep" || { echo "    a second handover changed or removed the first record"; exit 1; }
expect_exit 0 "$MJ" handover --resolve
expect_grep '^second$'
# --close lets a new task start; old one is archived, not lost
expect_grep '^outcome: handed_over$' .ai/local/state/current.yaml
expect_exit 0 "$MJ" start "t2" --scope lib
[ "$(ls .ai/local/state/archive/*.yaml | wc -l | tr -d ' ')" = 1 ]
# malformed record is skipped with a warning, not fatal
printf 'garbage\n' > .ai/local/state/handovers/zzz.md
expect_exit 0 "$MJ" handover --resolve
expect_grep 'warning: skipped'
