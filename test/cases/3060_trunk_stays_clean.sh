# majordomus-covers: session knowledge
# majordomus-negative: session
# An episode closed on the trunk leaves the trunk clean: its record waits in the outbox.
#
# ADR 0091, amendment of 2026-10-15 (I1984, slice 1). A trunk that changes only through pull
# requests cannot commit what an episode boundary writes there. OSCILLA's `main` held
# untracked .ai/repo/knowledge/candidates/ it could never commit, every later checkpoint
# recorded the tree dirty, and clean-tree release gates refused. So on the trunk the closed
# session record and the derived candidates go to .ai/local/state/outbox/, which is
# checkout-local, and `context` and `doctor` name what waits there.
#
# The trunk is the remote's default branch as git records it (refs/remotes/origin/HEAD),
# never a branch name written down: the fixture's trunk is `main`. The control half closes
# an episode on a feature branch of the same clone, where the record is written into the
# tracked section exactly as before.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "the layer" >/dev/null 2>&1 || true

# a remote whose default branch is main, and a local main tracking it
REMOTE="$(mktemp -d "${TMPDIR:-/tmp}/case3060.XXXXXX")"
trap 'rm -rf "$REMOTE"' EXIT
git init -q --bare "$REMOTE/origin.git"
git branch -M main
git remote add origin "$REMOTE/origin.git"
git push -q -u origin main >/dev/null 2>&1
git remote set-head origin main >/dev/null 2>&1
[ "$(git symbolic-ref -q refs/remotes/origin/HEAD)" = refs/remotes/origin/main ] \
  || { echo "    the fixture has no recorded trunk"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "    the fixture is not clean before the episode:"; git status --porcelain | sed 's/^/    | /'; exit 1; }

OUTBOX=.ai/local/state/outbox
SESSIONS=.ai/repo/sessions
CAND=.ai/repo/knowledge/candidates

# ---------------------------------------------------------------- the trunk
expect_exit 0 "$MJ" session start --owner tester
SID="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml | head -n 1)"
[ -n "$SID" ] || { echo "    no episode opened"; exit 1; }
expect_exit 0 "$MJ" decision add "A trunk stays clean" --why "only a pull request changes it"
expect_exit 0 "$MJ" session close < /dev/null

# the trunk is clean: no untracked record, no modified tracked file
dirty="$(git status --porcelain)"
[ -z "$dirty" ] || { echo "    closing an episode on the trunk left it dirty:"; printf '%s\n' "$dirty" | sed 's/^/    | /'; exit 1; }

# the outbox holds the closed record, and the tracked section does not
rec="$(grep -rl "^session_id: $SID\$" "$OUTBOX/sessions" 2>/dev/null | head -n 1 || true)"
[ -n "$rec" ] || { echo "    the outbox holds no record of $SID"; ls -R "$OUTBOX" 2>/dev/null | sed 's/^/    | /'; exit 1; }
expect_grep '^branch: main$' "$rec"
expect_grep '^schema: session/v1$' "$rec"
if grep -rlq "^session_id: $SID\$" "$SESSIONS" 2>/dev/null; then
  echo "    the trunk episode's record was also written into $SESSIONS"; exit 1
fi

# the derived candidate is in the outbox, named after its episode, and not in the section
cand="$(find "$OUTBOX/candidates" -maxdepth 1 -name "$SID-*.md" 2>/dev/null | head -n 1)"
[ -n "$cand" ] || { echo "    the outbox holds no candidate of $SID"; ls -R "$OUTBOX" 2>/dev/null | sed 's/^/    | /'; exit 1; }
expect_grep '^status: candidate$' "$cand"
expect_grep 'A trunk stays clean' "$cand"
[ -z "$(find "$CAND" -maxdepth 1 -name "$SID-*.md" 2>/dev/null)" ] \
  || { echo "    the trunk episode's candidate was also written into $CAND"; exit 1; }

# context and doctor name the non-empty outbox
expect_exit 0 "$MJ" context
expect_grep "^outbox +1 session record\\(s\\), 1 knowledge candidate\\(s\\) in $OUTBOX/"
# doctor's verdict on this fixture is not this case's; only the line it names the outbox in
LAST_OUT="$("$MJ" doctor 2>&1 || true)"
expect_grep "INFO +clone +outbox .*1 session record\\(s\\) and 1 knowledge candidate\\(s\\)"

# ---------------------------------------------------------------- the control: a branch
git checkout -q -b feature/elsewhere
expect_exit 0 "$MJ" session start --owner tester
SID2="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml | head -n 1)"
expect_exit 0 "$MJ" session close < /dev/null
grep -rlq "^session_id: $SID2\$" "$SESSIONS" 2>/dev/null \
  || { echo "    a branch episode's record was not written into $SESSIONS"; exit 1; }
if grep -rlq "^session_id: $SID2\$" "$OUTBOX/sessions" 2>/dev/null; then
  echo "    a branch episode's record went to the outbox"; exit 1
fi
LAST_OUT="$(git status --porcelain --untracked-files=all)"
expect_grep "$SESSIONS/"
