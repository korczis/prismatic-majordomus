# majordomus-covers: session knowledge
# majordomus-negative: session
# The next episode on a branch carries what a trunk episode staged in the outbox, once.
#
# ADR 0091, amendment of 2026-10-15 (I1984, slice 2). Slice 1 (case 3060) keeps the trunk
# clean: a closed session record and its derived candidates wait in .ai/local/state/outbox/.
# This is the other half. The next episode boundary on any other branch of the clone moves
# them into .ai/repo/sessions/ and .ai/repo/knowledge/candidates/ unchanged — byte for byte,
# so each still names the session that wrote it and not the one that carried it — and
# empties the outbox, after which `context` and `doctor` have nothing to name.
#
# Exactly once: a second carrying episode moves nothing, and a record the tracked section
# already holds is not written twice. A file in the outbox that is not a record is reported
# and stays where it is: it is neither moved into a tracked section nor silently dropped.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "the layer" >/dev/null 2>&1 || true

# a remote whose default branch is main, and a local main tracking it
REMOTE="$(mktemp -d "${TMPDIR:-/tmp}/case3063.XXXXXX")"
trap 'rm -rf "$REMOTE"' EXIT
git init -q --bare "$REMOTE/origin.git"
git branch -M main
git remote add origin "$REMOTE/origin.git"
git push -q -u origin main >/dev/null 2>&1
git remote set-head origin main >/dev/null 2>&1
[ "$(git symbolic-ref -q refs/remotes/origin/HEAD)" = refs/remotes/origin/main ] \
  || { echo "    the fixture has no recorded trunk"; exit 1; }

OUTBOX=.ai/local/state/outbox
SESSIONS=.ai/repo/sessions
CAND=.ai/repo/knowledge/candidates

# How many tracked-section records name episode $1.
records_of() { grep -rl --exclude='.tmp.*' "^session_id: $1\$" "$SESSIONS" 2>/dev/null | grep -c . || true; }
# Every file waiting in the outbox, one per line.
outbox_files() { find "$OUTBOX/sessions" "$OUTBOX/candidates" -type f 2>/dev/null || true; }

# ---------------------------------------------------------------- the trunk stages
expect_exit 0 "$MJ" session start --owner tester
SID="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml | head -n 1)"
[ -n "$SID" ] || { echo "    no episode opened"; exit 1; }
expect_exit 0 "$MJ" decision add "A branch carries the outbox" --why "only a pull request changes the trunk"
expect_exit 0 "$MJ" session close < /dev/null

staged="$(grep -rl "^session_id: $SID\$" "$OUTBOX/sessions" 2>/dev/null | head -n 1 || true)"
[ -n "$staged" ] || { echo "    the trunk close staged no record of $SID"; exit 1; }
cand="$(find "$OUTBOX/candidates" -maxdepth 1 -name "$SID-*.md" 2>/dev/null | head -n 1)"
[ -n "$cand" ] || { echo "    the trunk close staged no candidate of $SID"; exit 1; }
[ "$(outbox_files | grep -c .)" = 2 ] \
  || { echo "    the outbox should hold one record and one candidate:"; outbox_files | sed 's/^/    | /'; exit 1; }
SAVED="$REMOTE/saved"; mkdir -p "$SAVED"
cp "$staged" "$SAVED/record.md"; cp "$cand" "$SAVED/candidate.md"
rec_name="$(basename "$staged")"; cand_name="$(basename "$cand")"

# ---------------------------------------------------------------- a branch episode carries it
git checkout -q -b feature/carry
expect_exit 0 "$MJ" session start --owner tester
SID2="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml | head -n 1)"
[ -n "$SID2" ] && [ "$SID2" != "$SID" ] || { echo "    the branch episode did not open"; exit 1; }
expect_exit 0 "$MJ" session close < /dev/null

# the record is tracked once, unchanged: it names its own episode and the trunk it closed on
[ "$(records_of "$SID")" = 1 ] \
  || { echo "    $SESSIONS holds $(records_of "$SID") record(s) of $SID, not 1"; exit 1; }
[ -f "$SESSIONS/$rec_name" ] || { echo "    the record was not carried under its own name $rec_name"; exit 1; }
cmp -s "$SAVED/record.md" "$SESSIONS/$rec_name" \
  || { echo "    the carried record differs from the one the trunk staged:"; diff "$SAVED/record.md" "$SESSIONS/$rec_name" | sed 's/^/    | /'; exit 1; }
expect_grep "^session_id: $SID\$" "$SESSIONS/$rec_name"
expect_grep '^branch: main$' "$SESSIONS/$rec_name"
# the carrying episode has a record of its own, and it is not the carried one
[ "$(records_of "$SID2")" = 1 ] || { echo "    the branch episode has no record of its own"; exit 1; }

# the candidate is tracked, unchanged, still derived from the trunk's episode
[ -f "$CAND/$cand_name" ] || { echo "    the candidate was not carried into $CAND"; exit 1; }
cmp -s "$SAVED/candidate.md" "$CAND/$cand_name" || { echo "    the carried candidate differs"; exit 1; }
expect_grep "^    - session:$SID\$" "$CAND/$cand_name"
expect_exit 0 "$MJ" knowledge candidates
expect_grep "${cand_name%.md}"

# the outbox is empty, and context and doctor stop naming it
[ -z "$(outbox_files)" ] || { echo "    the outbox was not emptied:"; outbox_files | sed 's/^/    | /'; exit 1; }
expect_exit 0 "$MJ" context
expect_no_grep '^outbox '
LAST_OUT="$("$MJ" doctor 2>&1 || true)"
expect_no_grep 'INFO +clone +outbox'

# ---------------------------------------------------------------- exactly once
# A copy of a record the section already holds (a crash between the copy and the removal,
# or a second clone staging the same episode) is retired, not written a second time.
mkdir -p "$OUTBOX/sessions" "$OUTBOX/candidates"
cp "$SAVED/record.md" "$OUTBOX/sessions/$rec_name"
cp "$SAVED/candidate.md" "$OUTBOX/candidates/$cand_name"
expect_exit 0 "$MJ" session start --owner tester
expect_exit 0 "$MJ" session close < /dev/null
[ "$(records_of "$SID")" = 1 ] \
  || { echo "    a second drain wrote $SID again: $(records_of "$SID") records"; exit 1; }
[ "$(find "$CAND" -maxdepth 1 -name "$SID-*.md" | grep -c .)" = 1 ] \
  || { echo "    a second drain duplicated the candidate"; exit 1; }
cmp -s "$SAVED/candidate.md" "$CAND/$cand_name" || { echo "    a second drain changed the candidate"; exit 1; }
[ -z "$(outbox_files)" ] || { echo "    the already-tracked copies were left in the outbox"; exit 1; }

# with nothing waiting, a drain says nothing and moves nothing
expect_exit 0 "$MJ" session start --owner tester
expect_exit 0 "$MJ" session close < /dev/null
expect_no_grep 'outbox'
[ "$(records_of "$SID")" = 1 ] || { echo "    an empty outbox's drain wrote $SID again"; exit 1; }

# ---------------------------------------------------------------- a malformed file
printf 'not a session record\n' > "$OUTBOX/sessions/broken.md"
printf -- '---\nschema: knowledge/v1\nid: someone-else\n---\n' > "$OUTBOX/candidates/mislabelled.md"
expect_exit 0 "$MJ" session start --owner tester
expect_exit 0 "$MJ" session close < /dev/null
expect_grep "outbox.*sessions/broken\\.md.*not a session record"
expect_grep "outbox.*candidates/mislabelled\\.md.*not a knowledge record"
[ -f "$OUTBOX/sessions/broken.md" ] || { echo "    the malformed session file was dropped"; exit 1; }
[ -f "$OUTBOX/candidates/mislabelled.md" ] || { echo "    the malformed candidate was dropped"; exit 1; }
[ ! -e "$SESSIONS/broken.md" ] && [ ! -e "$CAND/mislabelled.md" ] \
  || { echo "    a malformed outbox file was moved into a tracked section"; exit 1; }
# and what stays is still named
expect_exit 0 "$MJ" context
expect_grep "^outbox +1 session record\\(s\\), 1 knowledge candidate\\(s\\)"
