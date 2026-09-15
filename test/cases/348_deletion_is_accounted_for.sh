# majordomus-covers: recover
# Every deletion this tool makes is accounted for, and the published promise says so.
#
# SECURITY.md is what a reader is asked to trust, and it said "Retention rotates to archived
# files; nothing is deleted". The first half holds. The last four words did not: `recover
# records` folds duplicate closed records of one episode into the oldest — the union of their
# changed_files and commits, plus a `## Recovery` section naming the fold — and then removes the
# superseded files from the tree. Nothing is lost and files are deleted, which is a nuance the
# sentence flattened into a false absolute. A promise nothing tests is how it drifted.
#
# So the promise is asserted here against the behaviour rather than against other prose: what
# `recover` removes, it has first folded into the record that stays, and it says so in the ledger.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- 1. the promise names the exception
SEC="$ROOT/SECURITY.md"
grep -q 'No recursive deletion' "$SEC" || { echo "    SECURITY.md no longer promises anything about deletion"; exit 1; }
grep -q 'recover' "$SEC" || { echo "    SECURITY.md promises about deletion without naming the one command that deletes"; exit 1; }
grep -q 'nothing is deleted\.' "$SEC" && { echo "    SECURITY.md still says nothing is deleted; recover records removes superseded records"; exit 1; }
echo "    the published promise names the command that removes and what it accounts for"

# ---------------------------------------------------------------- 2. a fixture with two records of one episode
"$MJ" init >/dev/null
git add -A >/dev/null 2>&1; git commit -qm init >/dev/null 2>&1
SID="s-20260101000000-aaaa"
rec() { # file, created_at, changed file, commit
  mkdir -p .ai/repo/sessions
  cat > ".ai/repo/sessions/$1" <<REC
---
schema: session/v1
kind: session
session_id: $SID
created_at: $2
task_id: none
profile: none
branch: master
head: 0000000000000000000000000000000000000000
working_tree: clean
changed_files:
  - $3
commits:
  - $4
outcome: finished
---

# The episode

REC
}
rec "20260101T000000Z--a.md" 2026-01-01T00:00:00Z lib/one.sh 1111111111111111111111111111111111111111
rec "20260101T010000Z--b.md" 2026-01-01T01:00:00Z lib/two.sh 2222222222222222222222222222222222222222
git add .ai/repo/sessions >/dev/null 2>&1; git commit -qm "two records of one episode" >/dev/null 2>&1
# the store carries its own README contract, so records are counted by what makes one
n_records() { grep -l '^session_id: ' .ai/repo/sessions/*.md 2>/dev/null | wc -l | tr -d ' '; }
n_before="$(n_records)"
[ "$n_before" = 2 ] || { echo "    the fixture did not produce two records ($n_before)"; exit 1; }

# ---------------------------------------------------------------- 3. --check writes nothing
expect_exit 0 "$MJ" recover records --check
[ "$(n_records)" = 2 ] \
  || { echo "    --check removed a record; the plan is supposed to write nothing"; exit 1; }
echo "    the plan writes nothing"

# ---------------------------------------------------------------- 4. the fold accounts for what it removes
expect_exit 0 "$MJ" recover records
kept="$(grep -l "^session_id: $SID\$" .ai/repo/sessions/*.md 2>/dev/null | head -1)"
[ -n "$kept" ] || { echo "    the fold left no record of the episode at all"; exit 1; }
[ "$(n_records)" = 1 ] \
  || { echo "    the fold left $(n_records) records; one episode is one record"; exit 1; }
case "$kept" in *20260101T000000Z--a.md) ;; *) echo "    the kept record is not the oldest: $kept"; exit 1 ;; esac
grep -q '^## Recovery$' "$kept" || { echo "    the kept record does not say that it absorbed another"; exit 1; }
for f in lib/one.sh lib/two.sh; do
  grep -q "$f" "$kept" || { echo "    the removed record's changed file $f is not in the kept one: content was lost, not folded"; exit 1; }
done
for c in 1111111111111111111111111111111111111111 2222222222222222222222222222222222222222; do
  grep -q "$c" "$kept" || { echo "    the removed record's commit $c is not in the kept one"; exit 1; }
done
echo "    what was removed is in the record that stayed, and the record says so"

# ---------------------------------------------------------------- 5. the ledger carries the removal
L=.ai/local/state/ledger.jsonl
[ -f "$L" ] || { echo "    no ledger; a deletion nobody recorded is not accounted for"; exit 1; }
grep -q 'session.recovered' "$L" || { echo "    the fold is not in the ledger:"; tail -3 "$L" | sed 's/^/    | /'; exit 1; }
echo "    the removal is written to the ledger"
