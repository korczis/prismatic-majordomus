# majordomus-covers: check finish evidence handover
# majordomus-negative: finish evidence
# Completed is earned, not claimed. A disposable repository turns on
# verification.completed_means_complete, gets a CI model, the planner and the dispatcher CI
# runs, and a task whose change selects two gates. `completed` is refused while the
# completion report says `complete` is false, naming each question owed;
# `evidence --run-gates` runs the selected gates through the dispatcher and records each
# one; an obligation the change implies is declared by discharging it; and `completed` is
# accepted exactly when the report says complete. A weaker outcome is never refused and
# records the stage it stopped at.
#
# The shipped policy (share/completion.yaml) also asks questions nothing in the report
# reaches (`elsewhere:` sources, which answer `unknown`), the closing readings and the
# repository's convergence, so no fixture can be complete under it; the last section
# proves exactly that refusal. Everything before it reads a copy of the distribution whose
# policy keeps the questions this fixture answers: the obligations, the gates and the change.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj880.XXXXXX")"; trap 'rm -rf "$S"' EXIT
export MAJORDOMUS_BIN="$BIN"
git config user.email a@b.c; git config user.name a

# the distribution, with a policy of the questions this fixture can answer: every stage,
# and the questions whose source is an obligation, the gates or the change set
cp -R "$ROOT/share" "$S/share"
awk '
  /^questions:/ { print; q = 1; next }
  !q { print; next }
  /^  - id: / { flush(); buf = $0 "\n"; next }
  /^[^ ]/ { flush(); q = 0; print; next }
  { buf = buf $0 "\n"; if ($0 ~ /^    source: /) src = $2 }
  function flush() {
    if (buf != "" && (src ~ /^obligation:/ || src == "gates" || src == "change:test-path")) printf "%s", buf
    buf = ""; src = ""
  }
  END { flush() }
' "$ROOT/share/completion.yaml" > "$S/share/completion.yaml"
grep -q 'source: elsewhere:' "$S/share/completion.yaml" && { echo "    the fixture policy kept a question nothing answers"; exit 1; }
grep -q 'source: gates$' "$S/share/completion.yaml" || { echo "    the fixture policy lost the gates question"; exit 1; }
export MAJORDOMUS_SHARE="$S/share"

"$MJ" init >/dev/null
# the bit this case is about: off in the skeleton, with the reason, and on here; and the
# completion-gates doctrine at finish, which is where the bit is read
grep -q '^  completed_means_complete: false$' .ai/repo/policy.yaml \
  || { echo "    the skeleton policy carries no completed_means_complete key, or ships it on"; exit 1; }
sed -i.bak -e 's/^  completed_means_complete: false$/  completed_means_complete: true/' \
  -e 's/^    - note_present$/    - note_present\
    - gates_passed/' .ai/repo/policy.yaml && rm .ai/repo/policy.yaml.bak
grep -q '^    - gates_passed$' .ai/repo/policy.yaml || { echo "    gates_passed was not added to finish_requires"; exit 1; }
"$MJ" update >/dev/null

# the fixture's own CI model, planner and dispatcher: the same scripts CI runs, so a gate
# recorded here is the command CI would run. The planner sources the tool's lib/ through
# its own root, so the fixture borrows the tool's lib/ and bin/ and keeps its sources in src/.
mkdir -p .ai/repo/ci src test scripts/ci
ln -s "$ROOT/lib" lib; ln -s "$ROOT/bin" bin; printf 'lib\nbin\n' >> .gitignore
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
  - id: unit
    job: suite
    runs: sh test/unit.sh
    summary: the unit tests pass
  - id: docs-check
    job: structure
    runs: "true"
    summary: every reference resolves
classes:
  - id: src
    paths: [src/**, test/**]
    gates: [unit]
  - id: docs
    paths: [docs/**]
    gates: [docs-check]
  - id: layer
    paths: [.ai/repo/**, AGENTS.md, CLAUDE.md, .bb/**, .gitignore, scripts/**]
    gates: []
YAML
cp "$ROOT/scripts/ci-plan" scripts/ci-plan; cp "$ROOT/scripts/ci/run-plan" scripts/ci/run-plan
echo 'a() { :; }' > src/a.sh; echo 'exit 0' > test/unit.sh
git add -A >/dev/null && git commit -qm base
# a remote with a default branch, so that the facts git holds can be settled live
git init -q --bare "$S/remote.git"; git remote add origin "$S/remote.git"
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -q -u origin "$branch"; git remote set-head origin "$branch"

completion() { "$BIN" run gates.completion --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
q() { completion | jq -r --arg q "$1" '.questions[] | select(.id == $q) | .status'; }
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

# ---------------------------------------------------------------- refused while owed
expect_exit 0 "$MJ" start "prove it" --scope src,test --requires tests
echo 'b() { :; }' >> src/a.sh; echo 'exit 0 # touched' >> test/unit.sh
# where the task stands is derived and printed, never written
expect_exit 0 "$MJ" check
expect_grep 'INFO +stage +[^ ]+ — .* — (pending|blocked)'
[ "$(completion | jq -r .complete)" = false ] || { echo "    a task with owed questions is complete"; exit 1; }
# refused, and every owed question is named with the command that settles it
expect_exit 10 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
expect_grep 'FAIL +done +ci — queued'
expect_grep 'majordomus evidence --run-gates'
expect_grep 'FAIL +done +committed — queued'
grep -q '"task.finished"' .ai/local/state/ledger.jsonl && { echo "    a refused finish wrote a finish record"; exit 1; }

# ---------------------------------------------------------------- run and record the gates
expect_exit 0 "$MJ" evidence --run-gates
expect_grep '==> lint: ok'
expect_grep '==> unit: ok'
n="$(grep -c '"task.gate"' .ai/local/state/ledger.jsonl)"
[ "$n" -ge 2 ] || { echo "    --run-gates recorded $n gate run(s), not the two it ran"; exit 1; }
[ "$(q ci)" = pass ] || { echo "    every selected gate passed and the ci question is $(q ci)"; exit 1; }
# a failing gate recorded the same way refuses again
echo 'exit 3' > test/unit.sh
expect_exit 1 "$MJ" evidence --run-gates
expect_grep '==> unit: FAILED'
[ "$(q ci)" = fail ] || { echo "    a failing gate recorded through --run-gates reads $(q ci)"; exit 1; }
echo 'exit 0 # touched' > test/unit.sh
expect_exit 0 "$MJ" evidence --run-gates

# ---------------------------------------------------------------- an implied obligation is declared by discharging it
# `implementation` was never declared; the change implies it (its inputs are everything)
[ "$(q implemented)" = queued ] || { echo "    an implied, undeclared obligation is $(q implemented), not queued"; exit 1; }
expect_exit 0 "$MJ" evidence --covers implementation --command 'git diff --stat'
expect_grep 'declared on the task record'
grep -q '^  - implementation$' .ai/local/state/current.yaml || { echo "    discharging an implied token did not declare it"; exit 1; }
[ "$(q implemented)" = pass ] || { echo "    the discharged token is $(q implemented)"; exit 1; }
# and one the change does not imply is still refused
expect_exit 15 "$MJ" evidence --covers docs --command 'true'
expect_grep 'does not imply it'
expect_exit 0 "$MJ" evidence --covers tests --command 'sh test/unit.sh'

# ---------------------------------------------------------------- committed: complete
git add -A >/dev/null && git commit -qm 'feat: prove it'
printf '# Objective\n\nprove it\n\n# Current State\n\ndone\n\n# Next Action\n\nfinish\n' | "$MJ" handover >/dev/null
expect_exit 0 "$MJ" check
for id in implemented tested regression-tested committed ci; do
  [ "$(q "$id")" = pass ] || { echo "    $id is $(q "$id") on a tree where it should pass"; exit 1; }
done
# a fact git holds is judged live whether or not the task declared it, and answers a
# question only when the task or the change asks for it: nobody asked for a push here
[ "$(q pushed)" = exempt ] || { echo "    pushed is $(q pushed); neither the task nor the change asks for it"; exit 1; }
for id in documented published deployed; do
  [ "$(q "$id")" = exempt ] || { echo "    $id is $(q "$id"); this change does not reach it"; exit 1; }
done
[ "$(completion | jq -r .complete)" = true ] || { echo "    every question passes or is exempt and the report is not complete: $(completion | jq -c '[.questions[] | select(.status != "pass" and .status != "exempt") | .id]')"; exit 1; }
expect_exit 0 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
grep '"task.finished"' .ai/local/state/ledger.jsonl | tail -1 | grep -q '"complete":true' \
  || { echo "    the finish record does not carry complete:true"; exit 1; }

# ---------------------------------------------------------------- a weaker outcome records its stage
expect_exit 0 "$MJ" start "stop short" --scope src
echo 'c() { :; }' >> src/a.sh
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nrun the gates\n' | "$MJ" handover >/dev/null
expect_exit 0 "$MJ" finish --outcome partial --note "$S/note.md"
grep '"task.finished"' .ai/local/state/ledger.jsonl | tail -1 | grep -q '"stage":"' \
  || { echo "    a partial finish does not record the stage it stopped at"; exit 1; }
grep '"task.finished"' .ai/local/state/ledger.jsonl | tail -1 | grep -q '"complete":false' \
  || { echo "    a partial finish claims completion"; exit 1; }

# ---------------------------------------------------------------- the shipped policy keeps completed out of reach here
# A question the report cannot reach is unknown and never a pass, so under the shipped
# policy this fixture is refused completed by name, and partial is still accepted.
export MAJORDOMUS_SHARE="$ROOT/share"
expect_exit 0 "$MJ" start "under the shipped policy" --scope src
echo 'd() { :; }' >> src/a.sh
git add -A >/dev/null && git commit -qm 'feat: one more'
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command 'sh test/unit.sh' --note "$S/note.md"
expect_grep 'FAIL +done +[a-z-]+ — unknown'
expect_exit 0 "$MJ" finish --outcome partial --note "$S/note.md"
echo "    ok"
