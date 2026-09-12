# majordomus-covers: evidence check finish
# majordomus-negative: evidence finish
# Completion is a state the repository decides. This case gives a disposable repository a
# CI model of its own, starts a task, and walks the states a gate can be in: never reported
# (queued, which is not pass), failing (refused), passing (accepted), and passing-then-stale
# once the code moves underneath it (refused again). It also proves the derived half — a
# source change implies tests, a document change implies docs, a deployment is owed only by
# a task that declares one — and that a reopened task inherits nothing from the task before.
# Finally it asserts that the executable's plan and scripts/ci-plan select the same gates
# over this repository's own model, since both are readers of one declaration.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
S="$(mktemp -d "${TMPDIR:-/tmp}/mj131.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null
# This case measures the gates, not the definition of done: `completed` is earned by every
# question of share/completion.yaml (case 280), and here a task is finished over a fixture
# with no handover and no issue on purpose, so the policy's stricter bit is off.
sed -i.bak 's/^  completed_means_complete: true$/  completed_means_complete: false/' .ai/repo/policy.yaml && rm .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

# the fixture's own CI model: one gate every plan runs, one per path class, and one that
# cannot run without another, so `blocked` has something to be about
mkdir -p .ai/repo/ci lib docs test
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
    summary: every reference in the documents resolves
  - id: generated
    job: structure
    runs: "true"
    summary: every projection is current
  - id: probe
    job: site
    requires: [generated]
    runs: "true"
    summary: every route answers
classes:
  - id: src
    paths: [lib/**, test/**]
    gates: [unit]
  - id: docs
    paths: [docs/**]
    gates: [docs-check]
  - id: layer
    paths: [.ai/repo/**, AGENTS.md, CLAUDE.md, .gitignore]
    gates: [generated, probe]
YAML
echo 'a() { :; }' > lib/a.sh; echo '# doc' > docs/guide.md; echo 'exit 0' > test/unit.sh
git add -A >/dev/null && git commit -qm base

# gates.completion, as this process sees it; every surface reads this same document
# A managed repository is not a distribution: it carries no share/ of its own, so the
# executable is told where the tool's is — the same directory `majordomus` itself reads the
# shipped vocabulary from, and the same fallback lib/gates.sh applies for the validator.
export MAJORDOMUS_SHARE="$ROOT/share"
# `.output` is the capability's own document; the envelope around it is the execution
completion() { "$BIN" run gates.completion --input '{}' --quiet --format json --repo . 2>/dev/null | jq -c '.output'; }
status_of() { completion | jq -r --arg g "$1" '.gates[] | select(.id == $g) | .status'; }

# ---------------------------------------------------------------- recording needs a gate
expect_exit 2 "$MJ" evidence --gate unit
expect_grep 'needs --exit'
expect_exit 12 "$MJ" evidence --gate unit --exit 0
expect_grep 'no active task'
expect_exit 0 "$MJ" start "gated work" --scope lib/ --scope test/
expect_exit 2 "$MJ" evidence --gate nonsense --exit 0
expect_grep "no gate 'nonsense'"
expect_exit 2 "$MJ" evidence --gate unit --exit maybe
expect_grep 'as a number'
expect_exit 2 "$MJ" evidence --gate unit --covers tests --exit 0
expect_grep 'a gate or an obligation, not both'

# ---------------------------------------------------------------- never reported is queued
echo 'b() { :; }' >> lib/a.sh
git add -A >/dev/null && git commit -qm change
[ "$(status_of unit)" = queued ] || { echo "    a planned gate nobody ran is $(status_of unit), not queued"; exit 1; }
[ "$(status_of docs-check)" = exempt ] || { echo "    docs-check should be exempt for a lib/ change, is $(status_of docs-check)"; exit 1; }
[ "$(completion | jq -r .finishable)" = true ] || { echo "    silence refused completion; it must be reported, not refused"; exit 1; }
expect_exit 0 "$MJ" check
expect_grep "required gate\(s\) have never reported: .*unit"
expect_grep 'never-reported is not green'

# ---------------------------------------------------------------- missing test: derived
# a source change implies `tests` from the token's own inputs; the task did not declare it
completion | jq -e '.obligations[] | select(.id == "tests") | .applicable and (.declared | not)' >/dev/null \
  || { echo "    a lib/ change did not imply the tests obligation"; exit 1; }
expect_grep "obligation:tests .*the change implies 'tests'"

# ---------------------------------------------------------------- failing test: refused
expect_exit 0 "$MJ" evidence --gate unit --exit 1 --command 'sh test/unit.sh'
expect_grep 'reported exit 1'
[ "$(status_of unit)" = fail ] || { echo "    a failing run reads $(status_of unit)"; exit 1; }
[ "$(completion | jq -r .finishable)" = false ] || { echo "    a known failure left the task finishable"; exit 1; }
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate +unit — fail: exit 1'
# an honest outcome is never refused over a gate
expect_exit 0 "$MJ" check
expect_grep 'gate +unit — fail: exit 1 .*\(not refused'

# ---------------------------------------------------------------- blocked by a prerequisite
echo 'x' > .ai/repo/marker
git add -A >/dev/null && git commit -qm layer
expect_exit 0 "$MJ" evidence --gate generated --exit 11 --command 'majordomus generate --check'
[ "$(status_of generated)" = fail ] || { echo "    dirty generated artifacts read $(status_of generated)"; exit 1; }
[ "$(status_of probe)" = blocked ] || { echo "    a gate whose prerequisite failed reads $(status_of probe), not blocked"; exit 1; }
completion | jq -e '.blocking | index("generated") and index("probe")' >/dev/null \
  || { echo "    blocking does not name both the failure and what it blocks"; exit 1; }
expect_exit 0 "$MJ" evidence --gate generated --exit 0 --command 'majordomus generate --check'
expect_exit 0 "$MJ" evidence --gate probe --exit 0 --command 'scripts/probe'

# ---------------------------------------------------------------- clean pass: accepted
expect_exit 0 "$MJ" evidence --gate unit --exit 0 --command 'sh test/unit.sh'
expect_exit 0 "$MJ" evidence --gate lint --exit 0 --command 'true'
[ "$(status_of unit)" = pass ] || { echo "    a passing run reads $(status_of unit); the shell and the executable disagree about the inputs hash"; exit 1; }
[ "$(completion | jq -r .finishable)" = true ] || { echo "    every gate passed and the task is not finishable"; exit 1; }
expect_exit 0 "$MJ" check
expect_grep 'OK   gate +.*every gate the change selects has reported over this tree'

# ---------------------------------------------------------------- the point: staleness
# the code moves underneath a passing run; the run proves nothing about the new tree
echo 'c() { :; }' >> lib/a.sh
git add -A >/dev/null && git commit -qm moved
[ "$(status_of unit)" = stale ] || { echo "    a passing run over an old tree reads $(status_of unit), not stale"; exit 1; }
[ "$(status_of docs-check)" = exempt ] || { echo "    a lib/ edit touched docs-check"; exit 1; }
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL gate +unit — stale: the run was taken over inputs'
# an uncommitted edit is part of the change set too: staleness does not wait for a commit
expect_exit 0 "$MJ" evidence --gate unit --exit 0 --command 'sh test/unit.sh'
expect_exit 0 "$MJ" evidence --gate lint --exit 0 --command 'true'
[ "$(status_of unit)" = pass ] || { echo "    re-recording over the new tree did not discharge it"; exit 1; }
echo 'd() { :; }' >> lib/a.sh
[ "$(status_of unit)" = stale ] || { echo "    an uncommitted edit left the run $(status_of unit)"; exit 1; }
git checkout -q -- lib/a.sh
[ "$(status_of unit)" = pass ] || { echo "    reverting the edit did not restore the run"; exit 1; }

# ---------------------------------------------------------------- the done invariant
# One question per declaration of share/completion.yaml — the one definition of done —
# each with the source that answered it and the evidence it read. The count is read from
# the policy, never written here: a question added there is a question asked here.
declared="$(grep -c '^  - id: ' "$ROOT/share/completion.yaml")"
stages="$(awk '/^stages:/{s=1;next} /^questions:/{s=0} s && /^  - id: /{n++} END{print n}' "$ROOT/share/completion.yaml")"
declared=$((declared - stages))
[ "$(completion | jq -r '.questions | length')" = "$declared" ] \
  || { echo "    the done invariant is $(completion | jq -r '.questions | length') questions, and the policy declares $declared"; exit 1; }
completion | jq -e '[.questions[] | select((.stage | length) == 0)] | length == 0' >/dev/null \
  || { echo "    a question belongs to no stage"; exit 1; }
completion | jq -e '[.questions[] | select((.source | length) == 0 or (.evidence | length) == 0)] | length == 0' >/dev/null \
  || { echo "    a question carries no source or no evidence"; exit 1; }
# every gate has reported and passed, so the CI question passes and says what it read
[ "$(completion | jq -r '.questions[] | select(.id == "ci") | .status')" = pass ] \
  || { echo "    every gate passed and the ci question does not"; exit 1; }
completion | jq -r '.questions[] | select(.id == "ci") | .evidence' | grep -q 'never reported' \
  || { echo "    the ci question does not state its denominator"; exit 1; }
# this task touched lib/ and test/ paths, so regression is answered from the change set
[ "$(completion | jq -r '.questions[] | select(.id == "regression-tested") | .status')" = queued ] \
  || { echo "    a change with no test path answered regression-tested with $(completion | jq -r '.questions[] | select(.id == "regression-tested") | .status')"; exit 1; }
# a question answered by a gate this fixture's model does not declare is not applicable,
# and says so by name: not asked is a different fact from asked and silent
completion | jq -e '.questions[] | select(.id == "parity") | .status == "exempt"' >/dev/null \
  || { echo "    transport parity was answered by a gate the model does not declare"; exit 1; }
completion | jq -r '.questions[] | select(.id == "parity") | .evidence' | grep -q 'declares no gate' \
  || { echo "    an unanswerable question does not say why"; exit 1; }
# and a repository that has published nothing owes no version
completion | jq -e '.questions[] | select(.id == "version") | .status == "exempt"' >/dev/null \
  || { echo "    a repository with no release was asked for a version bump"; exit 1; }
# the task named no issue: owed, and the evidence says how to name one
[ "$(completion | jq -r '.questions[] | select(.id == "issue") | .status')" = queued ] \
  || { echo "    a task naming no issue was not told it owes one"; exit 1; }
# and where the task stands is derived from the answers, never written down
completion | jq -e '.stage | (.id | length) > 0 and (.state | length) > 0 and (.complete == false)' >/dev/null \
  || { echo "    the stage is not derived"; exit 1; }
completion | jq -e '.verified == true and .finishable == true' >/dev/null \
  || { echo "    every gate reported and passed, and the report does not say verified"; exit 1; }
# nothing is ever passed without something behind it
completion | jq -e '[.questions[] | select(.status == "pass")] | length >= 1' >/dev/null \
  || { echo "    no question passed at all on a tree where every gate passes"; exit 1; }
expect_exit 0 "$MJ" check
expect_grep 'done .*the done invariant is not yet answered'

# ---------------------------------------------------------------- deployment: derived
completion | jq -e '.obligations[] | select(.id == "deploy") | .applicable == false' >/dev/null \
  || { echo "    a task that deploys nothing was told it owes a deployment"; exit 1; }
completion | jq -r '.obligations[] | select(.id == "deploy") | .reason' | grep -q 'outside this tree' \
  || { echo "    not-applicable did not say why"; exit 1; }

# ---------------------------------------------------------------- finished, then reopened
expect_exit 0 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'finish: .* completed'
# a new task on the same tree inherits no verdict: the runs belonged to the task before
expect_exit 0 "$MJ" start "reopened, and deploys" --scope docs/ --requires deploy
echo '# more' >> docs/guide.md
[ "$(status_of docs-check)" = queued ] || { echo "    a reopened task inherited a verdict: docs-check reads $(status_of docs-check)"; exit 1; }
[ "$(status_of unit)" = exempt ] || { echo "    a docs/ task planned unit: $(status_of unit)"; exit 1; }
completion | jq -e '.obligations[] | select(.id == "docs") | .applicable' >/dev/null \
  || { echo "    a document change did not imply the docs obligation"; exit 1; }
completion | jq -e '.obligations[] | select(.id == "deploy") | .applicable and .declared' >/dev/null \
  || { echo "    a task that declares a deployment was not told it owes one"; exit 1; }
# deployment required, and nothing here can establish it: finish refuses on the obligation
expect_exit 10 "$MJ" finish --outcome completed --verify-command true --note "$S/note.md"
expect_grep 'FAIL obligation +deploy'

# ---------------------------------------------------------------- no model, no verdict
git rm -q .ai/repo/ci/gates.yaml && git commit -qm 'no model'
expect_exit 0 "$MJ" check
expect_grep 'no readable CI model'

# ---------------------------------------------------------------- one declaration, two readers
# The executable's plan and scripts/ci-plan read the same file and must select the same set.
# Over this repository's own model, for paths that fall in one class, several, and none.
for paths in 'apps/majordomus-cli/src/lib.rs' 'docs/CLI.md' 'site/templates/index.html lib/check.sh' \
             'share/obligations.yaml' 'nowhere/declared.txt' '.ai/repo/ci/gates.yaml'; do
  sh_set="$(printf '%s\n' $paths | (cd "$ROOT" && scripts/ci-plan --files - --format json) \
    | jq -r '.gates[] | select(.selected) | .id' | LC_ALL=C sort | tr '\n' ' ')"
  input="$(printf '%s\n' $paths | jq -R . | jq -sc '{changed: .}')"
  rs_set="$("$BIN" run gates.completion --input "$input" --quiet --format json --repo "$ROOT" 2>/dev/null \
    | jq -r '.output.plan.selected | keys[]' | LC_ALL=C sort | tr '\n' ' ')"
  [ -n "$rs_set" ] && [ "$sh_set" = "$rs_set" ] || {
    echo "    the two readers of gates.yaml disagree for: $paths"
    echo "      ci-plan:    $sh_set"
    echo "      executable: $rs_set"
    exit 1
  }
done
exit 0
