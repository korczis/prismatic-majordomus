# majordomus-covers: check finish evidence handover
# majordomus-negative: finish
# claim: completion-is-proved
# Completion is proved, not claimed (ADR 0057, rule project.completion-is-proved). A
# disposable repository gets a CI model, the planner and the dispatcher CI runs, and a task
# whose change selects two gates. `completed` is refused while the completion report says
# `complete` is false, naming each question owed; `evidence --run-gates` runs the selected
# gates through the dispatcher and records each one; an obligation the change implies is
# discharged and declared by doing so; and `completed` is accepted exactly when the report
# says complete. A weaker outcome is not refused and records the stage it stopped at.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
command -v jq >/dev/null 2>&1 || { echo "    skip: jq is not installed"; exit 0; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj280.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git config user.email a@b.c; git config user.name a

"$MJ" init >/dev/null
# the bit this case is about: on here, off in the skeleton with the reason
sed -i.bak 's/^  completed_means_complete: false$/  completed_means_complete: true/' .ai/repo/policy.yaml && rm .ai/repo/policy.yaml.bak
grep -q '^  completed_means_complete: true$' .ai/repo/policy.yaml || { echo "    the skeleton policy carries no completed_means_complete key"; exit 1; }
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
# a remote with a default branch, so that push and target can be established live
git init -q --bare "$S/remote.git"; git remote add origin "$S/remote.git"
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -q -u origin "$branch"; git remote set-head origin "$branch"

completion() { "$BIN" run gates.completion --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
q() { completion | jq -r --arg q "$1" '.questions[] | select(.id == $q) | .status'; }
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

# ---------------------------------------------------------------- refused while owed
expect_exit 0 "$MJ" start "prove it" --scope src,test --requires tests --issue none
echo 'b() { :; }' >> src/a.sh; echo 'exit 0 # touched' >> test/unit.sh
# where the task stands is derived and printed, never written
expect_exit 0 "$MJ" check
expect_grep 'INFO +stage .*(Implementation|Tests and validation) — pending'
[ "$(completion | jq -r .complete)" = false ] || { echo "    a task with owed questions is complete"; exit 1; }
[ "$(completion | jq -r .stage.complete)" = false ] || { echo "    the stage fold says complete over owed questions"; exit 1; }
# refused, and every owed question is named with the command that settles it
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL +done +ci — queued'
expect_grep 'majordomus evidence --run-gates'
expect_grep 'FAIL +done +committed'
expect_grep 'finish: refused'
grep -q '"task.finished"' .ai/local/state/ledger.jsonl && { echo "    a refused finish wrote a finish record"; exit 1; }

# ---------------------------------------------------------------- run and record the gates
expect_exit 0 "$MJ" evidence --run-gates
expect_grep '==> lint: ok'
expect_grep '==> unit: ok'
n="$(grep -c '"task.gate"' .ai/local/state/ledger.jsonl)"
[ "$n" -ge 2 ] || { echo "    --run-gates recorded $n gate run(s), not the two it ran"; exit 1; }
[ "$(q ci)" = pass ] || { echo "    every selected gate passed and the ci question is $(q ci)"; exit 1; }
[ "$(completion | jq -r .verified)" = true ] || { echo "    every gate reported and the report is not verified"; exit 1; }
# a failing gate recorded the same way refuses again
echo 'exit 3' > test/unit.sh
expect_exit 1 "$MJ" evidence --run-gates
expect_grep '==> unit: FAILED'
[ "$(q ci)" = fail ] || { echo "    a failing gate did not refuse"; exit 1; }
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

# ---------------------------------------------------------------- commit, push, handover: complete
git add -A >/dev/null && git commit -qm 'feat: prove it' && git push -q origin "$branch"
printf '# Objective\n\nprove it\n\n# Current State\n\ndone\n\n# Next Action\n\nfinish\n' | "$MJ" handover >/dev/null
expect_exit 0 "$MJ" check
for id in implemented tested regression-tested committed pushed ci integrated handover; do
  [ "$(q "$id")" = pass ] || { echo "    $id is $(q "$id") on a tree where it should pass"; exit 1; }
done
for id in documented generated openapi parity version changelog published deployed deployment-verified issue; do
  [ "$(q "$id")" = exempt ] || { echo "    $id is $(q "$id"); this change does not reach it"; exit 1; }
done
[ "$(completion | jq -r .complete)" = true ] || { echo "    every question passes or is exempt and the report is not complete: $(completion | jq -c '[.questions[] | select(.status != "pass" and .status != "exempt") | .id]')"; exit 1; }
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
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

# ---------------------------------------------------------------- the policy resolves, and what this fixture never asks is named
"$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -e '.output.problems == null or (.output.problems | length) == 0' >/dev/null \
  || { echo "    the shipped policy has problems against its own vocabulary: $("$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c .output.problems)"; exit 1; }
# the fixture's model declares neither projection-closure nor release-check: not a problem
# of the policy, a question this repository never asks, listed by name
"$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.unanswered_gates[]' | grep -q '^changelog:release-check$' \
  || { echo "    a gate question the model never asks is not listed"; exit 1; }
cp -R "$ROOT/share" "$S/share"
sed -i.bak 's/^    source: obligation:pages$/    source: obligation:no-such-token/' "$S/share/completion.yaml" && rm "$S/share/completion.yaml.bak"
MAJORDOMUS_SHARE="$S/share" "$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.problems[]' | grep -q "no-such-token" \
  || { echo "    a question naming a token the vocabulary lacks is not reported as a problem"; exit 1; }
echo "    ok"
