# majordomus-covers: history
# majordomus-negative: history doctor watch
# History: the ledger read back. Ordering, filters, the JSON form, validation of malformed
# lines, and rotation that archives without deleting.
. "$ROOT/test/lib.sh"

# An event written before the repository has any commit must still be a well-formed line.
# `git rev-parse HEAD` in a repository with no commits prints the literal string "HEAD" on
# stdout and then fails, so a fallback that appends produces an identity field with an
# embedded newline — and one corrupt line in an append-only file is corrupt for good.
P="$T/before-first-commit"
mkdir -p "$P" && ( cd "$P" && git init -q . && git config user.email t@example.com && git config user.name t )
expect_exit 0 "$MJ" --repo "$P" init
expect_exit 0 "$MJ" --repo "$P" update
expect_exit 0 "$MJ" --repo "$P" history --validate
expect_grep 'history --validate: ok'
[ "$(grep -c '^{' "$P/.majordomus/state/ledger.jsonl")" = "$(wc -l < "$P/.majordomus/state/ledger.jsonl" | tr -d ' ')" ] \
  || { echo "    a ledger line does not begin with { — an identity field carried a newline"; exit 1; }
grep -q '"head":"NONE"' "$P/.majordomus/state/ledger.jsonl" \
  || { echo "    head was not recorded as NONE in a repository with no commits"; exit 1; }
rm -rf "$P"

"$MJ" init >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# before anything has happened
expect_exit 0 "$MJ" history
expect_grep 'no ledger yet'
"$MJ" update >/dev/null
expect_exit 0 "$MJ" history
expect_grep 'projections.updated'

# a whole lifecycle, so the events are real rather than hand-written
"$MJ" start "t1" --scope lib >/dev/null
id=$(sed -n 's/^id: //p' .majordomus/state/current.yaml)
printf 'a note\n' | "$MJ" checkpoint >/dev/null
"$MJ" decision add "use the existing store" --why "no second source of truth" >/dev/null
"$MJ" question add "which environments?" >/dev/null
"$MJ" question resolve 1 --answer "staging only" >/dev/null
echo b >> lib/a
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
"$MJ" finish --outcome completed --verify-command true >/dev/null

# oldest first, so a filtered run reads as a narrative
expect_exit 0 "$MJ" history --all
first=$(printf '%s\n' "$LAST_OUT" | head -1); last=$(printf '%s\n' "$LAST_OUT" | grep -E '^20' | tail -1)
printf '%s' "$first" | grep -q 'projections.updated'
printf '%s' "$last" | grep -q 'task.finished'
[ "$(printf '%s' "$first" | cut -c1-20)" \< "$(printf '%s' "$last" | cut -c1-20)" ]

# every lifecycle event is present, and each renders a useful detail column
for e in task.started task.checkpoint decision.recorded question.opened question.resolved task.handed_over task.finished; do
  expect_grep "$e"
done
expect_grep 'outcome=completed verify_exit=0'
expect_grep 'staging only'

# filters
expect_exit 0 "$MJ" history --task "$id" --all
expect_no_grep 'projections.updated'
expect_exit 0 "$MJ" history --task nosuch
expect_grep 'no matching events'
expect_exit 0 "$MJ" history --event task.started --all
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'task.started')" = 1 ]
expect_exit 0 "$MJ" history --since 1h --all
expect_grep 'task.started'
expect_exit 0 "$MJ" history --since 2026-01-01T00:00:00Z --all
expect_grep 'task.started'
expect_exit 2 "$MJ" history --since nonsense
expect_exit 2 "$MJ" history --limit x

# --limit keeps the newest and says how many it hid
expect_exit 0 "$MJ" history --limit 2
[ "$(printf '%s\n' "$LAST_OUT" | grep -cE '^20')" = 2 ]
expect_grep '^\(2 of [0-9]+ events'
expect_grep 'task.finished'

# --json is the matching lines verbatim, one object per line
expect_exit 0 "$MJ" --json history --all
printf '%s\n' "$LAST_OUT" | while IFS= read -r l; do
  case "$l" in '{"ts":"'*'"event":"'*'}') ;; *) echo "    not a ledger line: $l"; exit 1 ;; esac
done
if command -v jq >/dev/null; then printf '%s\n' "$LAST_OUT" | jq -e . >/dev/null; fi

# --validate accepts a healthy ledger and rejects a corrupted one, naming the line
expect_exit 0 "$MJ" history --validate
expect_grep 'history --validate: ok'
cp .majordomus/state/ledger.jsonl ledger.bak
printf 'this is not json\n' >> .majordomus/state/ledger.jsonl
expect_exit 10 "$MJ" history --validate
expect_grep 'FAIL ledger +line [0-9]+'
# the same corruption is a failure for every gate that reads it
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL records +ledger.jsonl'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT records +ledger.jsonl'
# and a malformed line is skipped by the reader rather than crashing it
expect_exit 0 "$MJ" history --all
expect_grep 'task.finished'
expect_no_grep 'this is not json'
cp ledger.bak .majordomus/state/ledger.jsonl

# --rotate: nothing to do under the cap
expect_exit 0 "$MJ" history --rotate
expect_grep '^nothing to rotate'
[ "$(find .majordomus/state -name '*.archived' | wc -l | tr -d ' ')" = 0 ]

# over the cap: the oldest lines move to an archive, none are lost, and it is recorded
before=$(wc -l < .majordomus/state/ledger.jsonl | tr -d ' ')
sed -i.bak 's/^  retention_max_lines: 5000/  retention_max_lines: 3/' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak
expect_exit 0 "$MJ" history --rotate
expect_grep '^rotated: [0-9]+ line\(s\)'
arch=$(find .majordomus/state -name '*.jsonl.archived')
[ -f "$arch" ]
# 3 kept + the ledger.rotated event appended after the rotation
[ "$(wc -l < .majordomus/state/ledger.jsonl | tr -d ' ')" = 4 ]
[ "$(( $(wc -l < "$arch" | tr -d ' ') + 3 ))" = "$before" ]     # nothing was deleted
expect_grep '"event":"ledger.rotated"' .majordomus/state/ledger.jsonl
expect_exit 0 "$MJ" history --event ledger.rotated --all
expect_grep 'archived=[0-9]+ kept=3'
