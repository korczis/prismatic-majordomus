# majordomus-covers: check
# majordomus-negative: check
# claims: scope-enforcement
# A task answers for what changes after it starts. The scope doctrine read `git status`,
# which is everything changed now, so whatever the tree already held when the task began was
# counted as the task's: a tracked file the repository's own hooks regenerate, somebody's
# work in progress, an untracked draft. An adopting repository met it on its first task
# (2026-10-13): scoped to three paths, refused at `finish --check` for seven files it had
# never opened, each changed before `start` and byte-identical since.
#
# The rule: `start` records what was dirty and what each path held; a path is not the
# task's while it still holds exactly that. A path the task edits, creates or commits is the
# task's as before, so the doctrine loses no teeth, and a record belongs to one task.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth reports && echo x > lib/auth/a.txt && echo r1 > reports/index.json \
  && echo n > notes.md && echo gone > old.txt && git add . && git commit -qm base

# the tree before any task: a regenerated file, work in progress, a draft, a removal
echo r2 > reports/index.json; echo wip >> notes.md; echo draft > "a draft.txt"; rm old.txt
expect_exit 0 "$MJ" start "fix auth" --scope lib/auth --owner alice
[ -f .ai/local/state/start-dirty.tsv ] || { echo "    start recorded nothing about the tree it began in"; exit 1; }
[ -z "$(git status --porcelain -- .ai/local 2>/dev/null)" ] || { echo "    the record shows in git status"; exit 1; }
echo y >> lib/auth/a.txt

rc=0; out="$("$MJ" check 2>&1)" || rc=$?
printf '%s\n' "$out" | grep -q 'outside claimed scope' && { echo "    files changed before the task are held against it:"; printf '%s\n' "$out" | grep 'outside claimed scope' | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q '^INFO scope .*4 path(s) were already changed when the task started' \
  || { echo "    check does not say what it left out: $(printf '%s\n' "$out" | grep ' scope ')"; exit 1; }
printf '%s\n' "$out" | grep -q '^OK   scope .*1 touched file(s), all within scope' \
  || { echo "    the task's own file is not the one counted: $(printf '%s\n' "$out" | grep ' scope ')"; exit 1; }

# edited after the start: the task's, by content, though the path was dirty before
echo more >> notes.md
rc=0; out="$("$MJ" check 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a pre-dirty file edited by the task passed (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q '^FAIL scope  *notes.md .*outside claimed scope' || { echo "    the edited file is not named: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'reports/index.json .*outside claimed scope' && { echo "    an untouched pre-dirty file is named beside it"; exit 1; }
# put back to what it held at the start: not the task's again
printf 'n\nwip\n' > notes.md
out="$("$MJ" check 2>&1)" || true
printf '%s\n' "$out" | grep -q 'outside claimed scope' && { echo "    a file restored to its starting content is still held against the task"; exit 1; }

# a path with a space, restored after the task removed it; and a removal undone
rm "a draft.txt"
out="$("$MJ" check 2>&1)" || true
printf '%s\n' "$out" | grep -q 'outside claimed scope' && { echo "    a pre-existing draft the task deleted left the status and is still named: $out"; exit 1; }
echo draft > "a draft.txt"
echo gone > old.txt   # the removal undone: the path is clean, so nothing to count
out="$("$MJ" check 2>&1)" || true
printf '%s\n' "$out" | grep -q 'outside claimed scope' && { echo "    a path that is clean again is named: $out"; exit 1; }
rm old.txt

# created after the start, outside the scope: refused as always
echo z > other.txt
rc=0; out="$("$MJ" check 2>&1)" || rc=$?
[ "$rc" = 10 ] && printf '%s\n' "$out" | grep -q '^FAIL scope  *other.txt' || { echo "    a file created outside the scope passed (exit $rc)"; exit 1; }
rm other.txt

# committed by the task: the commit is the task's, whatever the file held before
git add reports/index.json && git commit -qm "reports: carried along"
rc=0; out="$("$MJ" check 2>&1)" || rc=$?
[ "$rc" = 10 ] && printf '%s\n' "$out" | grep -q '^FAIL scope  *reports/index.json' || { echo "    a pre-dirty file the task committed passed (exit $rc)"; exit 1; }
git reset -q --soft HEAD~1 && git reset -q -- reports/index.json

# the record is one task's: under another task's name it answers nothing
sed '1s/.*/task	t-somebody-else/' .ai/local/state/start-dirty.tsv > .ai/local/state/start-dirty.tsv.new && mv .ai/local/state/start-dirty.tsv.new .ai/local/state/start-dirty.tsv
rc=0; out="$("$MJ" check 2>&1)" || rc=$?
[ "$rc" = 10 ] && printf '%s\n' "$out" | grep -q 'reports/index.json .*outside claimed scope' \
  || { echo "    another task's record excused this one's tree (exit $rc)"; exit 1; }
