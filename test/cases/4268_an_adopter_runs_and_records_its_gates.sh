# majordomus-covers: evidence
# An adopter runs every gate of its CI model and records it with one command (I4268). An
# adopter that declared .ai/repo/ci/gates.yaml saw every gate "never reported" until it wrote
# its own loop over `evidence --gate`, because the runner, scripts/ci-plan, ships only in
# this tool's source.
#
#   evidence --run-gates --all runs each gate's command and records a task.gate line per gate;
#   a failing gate makes it exit 10, and its exit is what is recorded;
#   a gate with no command is named and skipped; no model and no task are refused (12);
#   --run-gates refuses --gate, --exit and --covers, and --all belongs to it.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p src && echo a > src/a && git add . && git commit -qm base

expect_exit 12 "$MJ" evidence --run-gates --all
expect_grep 'no CI model'

mkdir -p .ai/repo/ci
cat > .ai/repo/ci/gates.yaml <<'Y'
version: 1
tiers:
  local: [feedback]
gates:
  - id: passes
    job: one
    lane: feedback
    always: true
    runs: "true"
    summary: always passes
  - id: fails
    job: two
    lane: feedback
    always: true
    runs: "exit 3"
    summary: always fails
  - id: commandless
    job: three
    lane: feedback
    always: true
    summary: names no command
classes:
  - id: all
    paths: ["**"]
    gates: full
Y
printf -- '---\nschema: context/v1\nid: ai.repo.ci\nkind: context\ntitle: CI\ndescription: The gates.\nstatus: active\nscope: subtree\nproviders: ["*"]\ncomposition: extend\norder: 100\n---\n\n# CI\n' > .ai/repo/ci/README.md
git add . && git commit -qm "a CI model"

expect_exit 12 "$MJ" evidence --run-gates --all
expect_grep 'no active task'
expect_exit 0 "$MJ" start "gate" --scope src
expect_exit 2 "$MJ" evidence --run-gates --gate passes --exit 0
expect_exit 2 "$MJ" evidence --all --gate passes --exit 0

expect_exit 10 "$MJ" evidence --run-gates --all
expect_grep 'passes +exit 0 +true'
expect_grep 'fails +exit 3 +exit 3'
expect_grep 'commandless +skipped'
grep -q '"event":"task.gate".*"gate":"passes".*"exit":0' .ai/local/state/ledger.jsonl \
  || { echo "    no task.gate line for the passing gate"; exit 1; }
grep -q '"event":"task.gate".*"gate":"fails".*"exit":3' .ai/local/state/ledger.jsonl \
  || { echo "    no task.gate line recording the failing gate's exit"; exit 1; }
echo "ok: every gate is run and recorded; a failure is recorded as itself and refuses"
