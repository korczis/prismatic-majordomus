# A blocking question survives a handover.
#
# The gate used to read only questions belonging to the active task, so opening a question,
# handing over and starting a new task left it unresolved and blocking nothing — the one
# mechanism for "stopped until a person answers" could be walked past by finishing the task
# that asked. This case was written first as a characterisation of that gap (M001/I0101) and
# rewritten here to assert the behaviour that replaced it (M001/I0103).
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

# --- a question opened against a task refuses a completed finish for that task
mkdir -p lib
"$MJ" start "first piece of work" --scope lib,note.md,note2.md,tool >/dev/null
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

# ---------------------------------------------------------------- across the boundary
# The question is still unresolved in the store...
grep -q '^- \[unresolved\] ' .ai/local/state/open-questions.md \
  || { echo "    the question was resolved by the handover; this case is testing nothing"; exit 1; }

# ... and the next task sees it and is stopped by it, though it did not open it.
"$MJ" start "second piece of work" --scope lib,note.md,note2.md,tool >/dev/null
expect_exit 0 "$MJ" question list
expect_grep 'which of the two callback URLs'
printf '# Objective\n\nsecond piece\n\n# Current State\n\ndone\n\n# Next Action\n\nnothing\n' > note2.md
"$MJ" handover < note2.md >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep '1 unresolved question\(s\) on this branch'
expect_grep 'which of the two callback URLs'
expect_grep 'blocker_resolution'

# --- and the second task can clear it, though it did not open it. A gate nobody can clear
#     is a gate that gets worked around.
expect_exit 0 "$MJ" question resolve "callback" --answer "the /auth/cb one; the other is a legacy alias"
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'finish: t-.* completed'

# --- the record of who asked survives resolution: the entry keeps the task that opened it
expect_exit 0 "$MJ" question list --all
expect_grep 'resolved.*which of the two callback URLs.*legacy alias'
expect_exit 0 "$MJ" history --validate

# ---------------------------------------------------------------- the template is not an entry
# Every fresh install ships open-questions.md with a commented example that matches the
# unresolved pattern. A scan that did not skip the comment block would block the first
# completed finish anybody attempted, on every installation.
grep -q '\[unresolved\] <task id>' .ai/local/state/open-questions.md \
  || { echo "    the shipped template no longer contains the example line; this check is stale"; exit 1; }
"$MJ" start "third piece of work" --scope lib,note.md,note2.md,tool >/dev/null
printf '# Objective\n\nthird\n\n# Current State\n\ndone\n\n# Next Action\n\nnothing\n' > note.md
"$MJ" handover < note.md >/dev/null
expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'none open'

# ---------------------------------------------------------------- the mutation
# Narrow the gate back to the active task and the escape returns. Without this the case
# would pass against the behaviour it exists to refuse.
mkdir -p "$T/tool"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$T/tool/"
sed 's/mj_question_unresolved_any "\$q"/mj_question_unresolved "$id" "$q"/' "$ROOT/lib/check.sh" > "$T/tool/lib/check.sh"
grep -q 'mj_question_unresolved "$id" "$q"' "$T/tool/lib/check.sh" \
  || { echo "    the probe did not take: the gate no longer calls mj_question_unresolved_any"; exit 1; }
M2="$T/tool/bin/majordomus"
"$M2" start "fourth piece of work" --scope lib,note.md,note2.md,tool >/dev/null
"$M2" question add "a question the next task will inherit" >/dev/null
printf '# Objective\n\nfourth\n\n# Current State\n\nasked\n\n# Next Action\n\nanswer\n' > note.md
"$M2" handover < note.md >/dev/null
"$M2" finish --outcome blocked >/dev/null
"$M2" start "fifth piece of work" --scope lib,note.md,note2.md,tool >/dev/null
printf '# Objective\n\nfifth\n\n# Current State\n\ndone\n\n# Next Action\n\nnothing\n' > note.md
"$M2" handover < note.md >/dev/null
expect_exit 0 "$M2" finish --outcome completed --verify-command "true"
# ... the narrowed gate let it through, which is the defect. The real one must not.
"$MJ" start "sixth piece of work" --scope lib,note.md,note2.md,tool >/dev/null
printf '# Objective\n\nsixth\n\n# Current State\n\ndone\n\n# Next Action\n\nnothing\n' > note.md
"$MJ" handover < note.md >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
expect_grep 'unresolved question\(s\) on this branch'
