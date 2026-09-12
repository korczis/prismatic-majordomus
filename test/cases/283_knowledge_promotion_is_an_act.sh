# majordomus-covers: knowledge
# majordomus-negative: knowledge
# claim: knowledge-promotion-is-an-act
# `verified` is written by a person with evidence, never by the deriver; rejection is a reason.
#
# The deriver writes candidates. What turns a candidate into something the repository
# asserts is an act: `promote` with evidence on standard input, which moves the record to
# the curated store and rewrites its provenance, or `reject` with a reason, which supersedes
# it in place. Each leaves a ledger line, and the deriver skips both afterwards so that the
# next episode boundary undoes neither. An act without evidence is an assertion, so an empty
# stdin is refused; evidence that is a conversation is refused for the same reason the
# record's body is.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
CUR=.ai/repo/knowledge/curated
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
event_count() { grep -c "\"event\":\"$1\"" "$LEDGER" 2>/dev/null || true; }
# derive by hand inside an open episode, and return the path the run wrote
derive_written() { "$MJ" knowledge derive 2>"$S/derive.err" | sed -n 's/^written //p' | head -n 1; }

"$MJ" session start >/dev/null
SID="$(sed -n 's/^session_id: //p' "$STATE/session-current.yaml" | head -n 1)"
"$MJ" start "promote and reject" --scope lib >/dev/null
TASK="$(sed -n 's/^id: //p' "$STATE/current.yaml" | head -n 1)"

# ---------------------------------------------------------------- one candidate
"$MJ" decision add "Promotion is an act with evidence" --why "an act without evidence is an assertion" >/dev/null
P1="$(derive_written)"
[ -n "$P1" ] || { echo "    the deriver wrote nothing:"; sed 's/^/    | /' "$S/derive.err"; exit 1; }
ID1="$(basename "$P1" .md)"
expect_file "$P1"
must "the deriver wrote something other than a candidate" grep -q '^status: candidate$' "$P1"
must "the candidate does not carry the decision's task" grep -q "^    - decision:$TASK$" "$P1"

# ---------------------------------------------------------------- refused
expect_exit 10 sh -c "printf '' | \"$MJ\" knowledge promote $ID1"
expect_grep 'evidence'
expect_exit 10 sh -c "printf 'assistant: yes, promote it\n' | \"$MJ\" knowledge promote $ID1"
expect_grep 'conversation|transcript'
expect_exit 12 sh -c "printf 'evidence\n' | \"$MJ\" knowledge promote no-such-record"
expect_exit 10 sh -c "printf 'evidence\n' | \"$MJ\" knowledge promote $ID1 --class opinion"
expect_file "$P1"
[ -f "$CUR/$ID1.md" ] && { echo "    a refused promotion still wrote a curated record"; exit 1; }
[ "$(event_count knowledge.promoted)" = 0 ] || { echo "    a refused promotion left a knowledge.promoted line"; exit 1; }
must "a refused promotion rewrote the candidate" grep -q '^status: candidate$' "$P1"

# ---------------------------------------------------------------- promoted
# The class is set here, by the person: the deriver writes every decision as a convention,
# and the act is where somebody says it is a constraint.
out="$(printf 'Verified by running the case that proves it.\n' | "$MJ" knowledge promote "$ID1" --class constraint 2>"$S/promote.err")" || {
  echo "    promote with evidence was refused:"; sed 's/^/    | /' "$S/promote.err"; exit 1; }
LAST="$(printf '%s\n' "$out" | tail -n 1)"
[ "$LAST" = "$CUR/$ID1.md" ] || { printf '    the last line is not the curated path: %s\n' "$LAST"; exit 1; }
expect_file "$CUR/$ID1.md"
[ -f "$P1" ] && { echo "    the candidate file is still there after promotion"; exit 1; }
C1="$CUR/$ID1.md"
printf '    promoted:\n'; sed 's/^/    | /' "$C1"
must "the curated record is not verified" grep -q '^status: verified$' "$C1"
must "the curated record did not take the class the person set" grep -q '^class: constraint$' "$C1"
must "the curated record lost the evidence it came from (decision:$TASK)" grep -q "^    - decision:$TASK$" "$C1"
must "the curated record lost the episode it came from" grep -q "^    - session:$SID$" "$C1"
must "the curated record does not carry the evidence under # Evidence" grep -q '^# Evidence$' "$C1"
must "the evidence text is not in the record" grep -qF 'Verified by running the case that proves it.' "$C1"
must "the id changed on promotion" grep -q "^id: $ID1$" "$C1"
[ "$(event_count knowledge.promoted)" = 1 ] || { printf '    %s knowledge.promoted line(s), expected 1\n' "$(event_count knowledge.promoted)"; exit 1; }
must "the knowledge.promoted line does not name the id and the curated path" \
  grep -q "\"event\":\"knowledge.promoted\".*\"id\":\"$ID1\".*\"path\":\"$CUR/$ID1.md\"" "$LEDGER"
# what was promoted validates as a curated record
expect_exit 0 "$MJ" knowledge check
expect_no_grep '^FAIL'

# the deriver does not undo the act: the same evidence now yields a skip, not a candidate
expect_exit 0 "$MJ" knowledge derive
expect_grep "^skipped $CAND/$ID1\.md \(promoted\)"
[ -f "$P1" ] && { echo "    the deriver rewrote a candidate that had been promoted"; exit 1; }
# a second promotion of the same id has nothing to promote
expect_exit 12 sh -c "printf 'evidence\n' | \"$MJ\" knowledge promote $ID1"

# ---------------------------------------------------------------- rejected
"$MJ" decision add "This restates ADR 0010" --why "a record that restates a source is not knowledge" >/dev/null
P2="$(derive_written)"
[ -n "$P2" ] || { echo "    the deriver wrote no second candidate:"; sed 's/^/    | /' "$S/derive.err"; exit 1; }
ID2="$(basename "$P2" .md)"
[ "$ID2" != "$ID1" ] || { echo "    two decisions yielded one id"; exit 1; }

expect_exit 2 "$MJ" knowledge reject "$ID2"
expect_grep 'reason'
expect_exit 2 "$MJ" knowledge reject "$ID2" --reason ""
expect_exit 12 "$MJ" knowledge reject no-such-record --reason "restates ADR 0010"
expect_exit 10 "$MJ" knowledge reject "$ID2" --reason "replaced" --by no-such-record
must "a refused rejection rewrote the candidate" grep -q '^status: candidate$' "$P2"
[ "$(event_count knowledge.rejected)" = 0 ] || { echo "    a refused rejection left a knowledge.rejected line"; exit 1; }

expect_exit 0 "$MJ" knowledge reject "$ID2" --reason "restates ADR 0010"
LAST="$(printf '%s\n' "$LAST_OUT" | tail -n 1)"
[ "$LAST" = "$P2" ] || { printf '    the last line is not the rewritten path: %s\n' "$LAST"; exit 1; }
printf '    rejected:\n'; sed 's/^/    | /' "$P2"
must "the rejected record is not superseded" grep -q '^status: superseded$' "$P2"
must "the rejected record does not carry # Rejected" grep -q '^# Rejected$' "$P2"
must "the reason is not in the record" grep -qF 'restates ADR 0010' "$P2"
[ -f "$CUR/$ID2.md" ] && { echo "    a rejection moved the record to curated/"; exit 1; }
must "the knowledge.rejected line does not name the id and the reason" \
  grep -q "\"event\":\"knowledge.rejected\".*\"id\":\"$ID2\".*\"reason\":\"restates ADR 0010\"" "$LEDGER"
expect_exit 0 "$MJ" knowledge check
expect_no_grep '^FAIL'

expect_exit 0 "$MJ" knowledge derive
expect_grep "^skipped $CAND/$ID2\.md \(superseded\)"
must "the deriver rewrote a superseded record" grep -q '^status: superseded$' "$P2"
# rejecting a record that is no longer a candidate is refused
expect_exit 10 "$MJ" knowledge reject "$ID2" --reason "twice"
expect_exit 10 sh -c "printf 'evidence\n' | \"$MJ\" knowledge promote $ID2"

# the review queue is empty: promoted and superseded records are not candidates
expect_exit 0 "$MJ" knowledge candidates
expect_grep '^knowledge candidates: 0 awaiting review'

# --by names the record that replaces one, and it must exist
"$MJ" decision add "A third decision, replaced by the first" --why "to prove --by" >/dev/null
P3="$(derive_written)"; ID3="$(basename "$P3" .md)"
expect_exit 0 "$MJ" knowledge reject "$ID3" --reason "the promoted record says it" --by "$ID1"
must "superseded_by does not name the replacing record" grep -q "^superseded_by: $ID1$" "$P3"
must "the knowledge.rejected line does not carry by" grep -q "\"event\":\"knowledge.rejected\".*\"id\":\"$ID3\".*\"by\":\"$ID1\"" "$LEDGER"
expect_exit 0 "$MJ" knowledge check
