# majordomus-covers: knowledge
# majordomus-skill: evolve
# The skill `evolve` is a procedure, not a system: every command it names exists, and
# following its steps in a disposable repository turns what a session decided into reviewed
# knowledge — one candidate promoted with evidence, one rejected with a reason, none left —
# through the commands the repository already had before the skill was written.
#
# What is proved: the skill is valid and invoked by the task lifecycle; each
# `majordomus knowledge <subcommand>` its text names is one the tool lists; and the procedure
# itself runs: candidates derived from an episode are listed, a promotion without evidence
# is refused, a promotion with evidence writes a verified curated record, a rejection
# records its reason, the review leaves no candidate behind, and `knowledge check` passes.
. "$ROOT/test/lib.sh"

SKILL="$ROOT/.ai/repo/skills/evolve/SKILL.md"
[ -f "$SKILL" ] || { echo "    .ai/repo/skills/evolve/SKILL.md is missing"; exit 1; }

# ---------------------------------------------------------------- it is invoked, and names what exists
grep -q 'majordomus://skill/evolve' "$ROOT/.ai/repo/workflows/task-lifecycle.md" \
  || { echo "    the task lifecycle does not invoke majordomus://skill/evolve, so the skill is an orphan"; exit 1; }
help="$("$MJ" knowledge --help 2>&1 || true)"
named="$(grep -oE 'majordomus knowledge [a-z]+' "$SKILL" | awk '{ print $3 }' | LC_ALL=C sort -u)"
[ -n "$named" ] || { echo "    the skill names no knowledge command at all"; exit 1; }
for sub in $named; do
  printf '%s\n' "$help" | grep -qE "^  $sub( |\$)" \
    || { echo "    the skill names 'majordomus knowledge $sub', which the tool does not list"; exit 1; }
done
for sub in candidates observe derive promote reject check; do
  printf '%s\n' "$named" | grep -qx "$sub" \
    || { echo "    the skill's procedure does not use 'majordomus knowledge $sub'"; exit 1; }
done
# no second store: the skill sends a lesson only to places that already hold its kind
if grep -nE '\.ai/repo/(evolution|lessons|evolve)/' "$SKILL"; then
  echo "    the skill names a store of its own for lessons"; exit 1
fi

# ---------------------------------------------------------------- the procedure, run
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH
CAND=.ai/repo/knowledge/candidates
CUR=.ai/repo/knowledge/curated
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.evolve.XXXXXX")"

"$MJ" session start >/dev/null
"$MJ" start "learn two things" --scope lib >/dev/null
"$MJ" decision add "A build behind a lock is bounded" --why "a silent wait reads as a slow build" >/dev/null
"$MJ" decision add "A lesson lives where its kind lives" --why "a note nothing reads changes nothing" >/dev/null

# step 1: gather
"$MJ" knowledge derive >"$S/derive.out" 2>"$S/derive.err" \
  || { echo "    derive failed:"; sed 's/^/    | /' "$S/derive.err"; exit 1; }
ids="$(sed -n 's/^written //p' "$S/derive.out" | while IFS= read -r p; do basename "$p" .md; done)"
n="$(printf '%s\n' "$ids" | grep -c . || true)"
[ "$n" = 2 ] || { echo "    two decisions derived $n candidate(s), expected 2:"; cat "$S/derive.out"; exit 1; }
ID1="$(printf '%s\n' "$ids" | sed -n 1p)"
ID2="$(printf '%s\n' "$ids" | sed -n 2p)"
expect_exit 0 "$MJ" knowledge candidates
expect_grep "$ID1"
expect_grep "$ID2"

# step 2: decide each — an act without evidence is refused, then one of two things
expect_exit 10 sh -c "printf '' | \"$MJ\" knowledge promote $ID1"
printf 'Proved by test/cases/3002: a blocked build is moved at the bound.\n' \
  | "$MJ" knowledge promote "$ID1" --class lesson >"$S/promote.out" 2>"$S/promote.err" \
  || { echo "    promote with evidence was refused:"; sed 's/^/    | /' "$S/promote.err"; exit 1; }
expect_file "$CUR/$ID1.md"
grep -q '^status: verified$' "$CUR/$ID1.md" || { echo "    the promoted record is not verified"; exit 1; }
grep -q '^class: lesson$' "$CUR/$ID1.md" || { echo "    the promoted record did not take its class"; exit 1; }
"$MJ" knowledge reject "$ID2" --reason "already said by the skill itself" >"$S/reject.out" 2>"$S/reject.err" \
  || { echo "    reject with a reason was refused:"; sed 's/^/    | /' "$S/reject.err"; exit 1; }
grep -q '^status: superseded$' "$CAND/$ID2.md" || { echo "    the rejected record is not superseded"; exit 1; }

# the review left nothing awaiting review, and what it wrote is valid
"$MJ" knowledge candidates >"$S/after.out" 2>&1 || { echo "    candidates failed after the review"; cat "$S/after.out"; exit 1; }
if grep -qE "$ID1|$ID2" "$S/after.out"; then
  echo "    a reviewed candidate is still listed as awaiting review:"; sed 's/^/    | /' "$S/after.out"; exit 1
fi
expect_exit 0 "$MJ" knowledge check
expect_no_grep '^FAIL'
