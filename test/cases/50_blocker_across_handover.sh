# What happens to a blocking question when the work moves to a new task.
#
# READ THIS BEFORE CHANGING ANYTHING BELOW. The assertions in the section marked THE GAP
# pin the behaviour that exists today, and that behaviour is the defect M001 exists to
# decide about. They are here so the gap is executable rather than a sentence in a claim
# note. When I0103 implements the decision, those assertions become wrong and must be
# rewritten to the chosen behaviour — a red case here is the fix working, not a regression.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

# --- a question opened against a task refuses a completed finish for that task
mkdir -p lib
"$MJ" start "first piece of work" --scope lib,note.md,note2.md >/dev/null
"$MJ" question add "which of the two callback URLs is canonical" >/dev/null
expect_exit 0 "$MJ" question list
expect_grep 'which of the two callback URLs'
expect_exit 10 "$MJ" finish --outcome completed
expect_grep 'blocker_resolution'

# --- the same task can still be finished as blocked, which is the documented escape hatch
printf '# Objective\n\nfirst piece\n\n# Current State\n\nblocked on a person\n\n# Next Action\n\nask them\n' > note.md
"$MJ" handover < note.md >/dev/null
expect_exit 0 "$MJ" finish --outcome blocked
expect_grep 'finish: t-.* blocked'

# ---------------------------------------------------------------- THE GAP
# The question is still unresolved in the store...
grep -q '^- \[unresolved\] ' .majordomus/state/open-questions.md \
  || { echo "    the question was resolved by the handover; the gap has moved"; exit 1; }
# ... and the next task neither sees it nor is stopped by it.
"$MJ" start "second piece of work" --scope lib,note.md,note2.md >/dev/null
expect_exit 0 "$MJ" question list
expect_grep 'no open questions for'
printf '# Objective\n\nsecond piece\n\n# Current State\n\ndone\n\n# Next Action\n\nnothing\n' > note2.md
"$MJ" handover < note2.md >/dev/null
# THE GAP: a completed finish is accepted while a question raised by this line of work is
# still unresolved. The blocker gate reads only questions belonging to the active task.
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'no unresolved entry|none open'
grep -q '^- \[unresolved\] ' .majordomus/state/open-questions.md \
  || { echo "    the unresolved question disappeared; re-read this case"; exit 1; }
# ---------------------------------------------------------------- END OF THE GAP

# --- the record of both events survives, which is what makes the gap recoverable at all
expect_exit 0 "$MJ" history --validate
expect_exit 0 "$MJ" question list --all
expect_grep 'which of the two callback URLs'
