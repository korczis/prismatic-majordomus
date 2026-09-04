# A closed session is an envelope of references, and it names its own episode only.
#
# Two properties are load-bearing and each has a way of failing quietly:
#
#   1. The record references; it does not copy. A session that inlined the bodies it names
#      would be a transcript with better formatting, which is the thing the whole design
#      refuses.
#   2. The references are selected by the session stamp the writer put on each ledger line,
#      not by a time range. The ledger is one file per repository. The first real run of
#      this code used a time window and collected another worker's tasks, checkpoints and
#      handovers, because nothing in a timestamp can tell two concurrent workers apart.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
LED=.majordomus/state/ledger.jsonl
SES=.majordomus/state/sessions

# --- closing with nothing open is a missing artifact, not a silent success
expect_exit 12 "$MJ" session close
expect_grep 'no open session'

# --- an episode across two tasks names both, and copies neither
"$MJ" session start --owner tester >/dev/null
"$MJ" start "first piece" --scope lib >/dev/null
printf 'the cause is in normalisation, not in the comparison\n' | "$MJ" checkpoint >/dev/null
"$MJ" decision add "Normalise before comparing" --why "the fixture proved the comparison was innocent" >/dev/null
printf '# Objective\no\n\n# Current State\nc\n\n# Next Action\nn\n' | "$MJ" handover --close >/dev/null
"$MJ" start "second piece" --scope test >/dev/null
printf 'second piece under way\n' | "$MJ" checkpoint >/dev/null
rec="$(printf 'both pieces moved\n' | "$MJ" session close)"
[ -f "$rec" ] || { echo "    close printed a path that is not a file: $rec"; exit 1; }

expect_grep '^tasks:$' "$rec"
[ "$(grep -c '^  - "t-' "$rec")" = 2 ] || { echo "    expected two tasks in the envelope"; sed -n '/^tasks:/,/^issues:/p' "$rec"; exit 1; }

# --- it references bodies, it does not carry them
expect_no_grep 'the cause is in normalisation' "$rec"
expect_no_grep 'the fixture proved the comparison was innocent' "$rec"
expect_grep '^checkpoints:$' "$rec"
expect_grep '^  - "\.majordomus/state/checkpoints/' "$rec"
expect_grep '^  - "\.majordomus/state/handovers/' "$rec"
expect_grep '^decisions:$' "$rec"
expect_grep '^  - "Normalise before comparing"$' "$rec"
# the authored summary is the only prose in the file
expect_grep '^both pieces moved$' "$rec"

# --- every referenced path exists; a reference is only worth what it resolves to
sed -n 's/^  - "\(\.majordomus\/state\/[^"]*\)"$/\1/p' "$rec" | while IFS= read -r p; do
  [ -f "$p" ] || { echo "    envelope references a file that does not exist: $p"; exit 1; }
done

# --- the open record is gone and exactly one close event was appended
[ -f .majordomus/state/session-current.yaml ] && { echo "    close left the open record behind"; exit 1; }
[ "$(grep -c '"event":"session.closed"' "$LED")" = 1 ] || { echo "    expected one session.closed event"; exit 1; }

# --- absent and empty are different facts, so an empty list is written as one
expect_grep '^questions: \[\]$' "$rec"
expect_grep '^commits: \[\]$' "$rec"

# --- a second episode does not inherit the first one's records
"$MJ" session start --owner tester >/dev/null
printf 'a different sitting entirely\n' | "$MJ" checkpoint >/dev/null
rec2="$(printf 'second episode\n' | "$MJ" session close)"
[ "$rec2" != "$rec" ] || { echo "    the second close overwrote the first record"; exit 1; }
expect_no_grep 'Normalise before comparing' "$rec2"
[ "$(grep -c '^  - "\.majordomus/state/checkpoints/' "$rec2")" = 1 ] \
  || { echo "    the second episode claimed the first episode's checkpoints"; exit 1; }

# --- one task spanning two sessions is named by both. The first record is not rewritten to
#     mention the second; later information supersedes, it never edits.
this_task="$(sed -n 's/^id: //p' .majordomus/state/current.yaml)"
grep -q "\"$this_task\"" "$rec2" || { echo "    the second episode did not name the task it worked on"; exit 1; }
grep -q "\"$this_task\"" "$rec" || { echo "    the first episode did not name the task it worked on"; exit 1; }

# --- THE REGRESSION: an event stamped by another session is not this session's work, and
#     neither is an event stamped by no session at all. This is the defect the first real
#     run produced, and a time-window implementation passes every assertion above and fails
#     these two. The injected timestamps are far in the future on purpose: a first draft of
#     this case used past ones, and every time-window implementation passed it, because the
#     lines fell outside the window for the wrong reason. The mutation was run against both
#     drafts; only this one fails it.
"$MJ" session start --owner tester >/dev/null
printf '{"ts":"2099-01-01T00:00:00Z","event":"task.checkpoint","head":"deadbee","branch":"master","by":"majordomus/0.1.0","session":"s-99999999999999-ffff","task_id":"t-somebody-else","checkpoint_path":".majordomus/state/checkpoints/not-mine.md"}\n' >> "$LED"
printf '{"ts":"2099-01-01T00:00:01Z","event":"decision.recorded","head":"deadbee","branch":"master","by":"majordomus/0.1.0","task_id":"t-nobody","decision":"A decision no episode claimed"}\n' >> "$LED"
printf 'mine only\n' | "$MJ" checkpoint >/dev/null
rec3="$(printf 'attribution\n' | "$MJ" session close)"
expect_no_grep 't-somebody-else' "$rec3"
expect_no_grep 'not-mine.md' "$rec3"
expect_no_grep 'A decision no episode claimed' "$rec3"

# --- commits made during the episode are listed, shortest form
"$MJ" session start --owner tester >/dev/null
printf 'one\n' > file-one.txt
git add file-one.txt && git commit -q -m "one"
short="$(git rev-parse --short=7 HEAD)"
rec4="$("$MJ" session close)"
expect_grep "^  - \"$short\"\$" "$rec4"

# --- a summary that forges identity is refused; a body that looks checkable and is not
#     is worse than no body
"$MJ" session start --owner tester >/dev/null
expect_exit 10 sh -c 'printf "head: 0000000\n" | "$0" session close' "$MJ"
expect_grep 'must not contain identity fields'
[ -f .majordomus/state/session-current.yaml ] || { echo "    a refused close still removed the open record"; exit 1; }

# --- the record is private and the store is append-only in practice: closing again writes
#     a new file and never touches the last one
before="$(cksum < "$rec4")"
"$MJ" session close >/dev/null
[ "$(cksum < "$rec4")" = "$before" ] || { echo "    closing a session rewrote an earlier record"; exit 1; }
for f in "$SES"/*.md; do
  [ "$(file_mode "$f")" = 600 ] || { echo "    $f is $(file_mode "$f"), expected 600"; exit 1; }
done
