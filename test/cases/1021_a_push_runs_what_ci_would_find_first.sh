# claim: derived-data-current
# A push runs what CI would find first (project.a-push-runs-what-ci-would-find-first).
#
# On 2026-10-09 CI refused a release twice for what a local run finds in seconds: a structure
# gate and a one-second case. Nothing ran either before the push. `scripts/ci/run-plan
# --before-push`, which the pre-push hook calls, runs a budget of the plan's structure gates,
# chosen from the seconds CI recorded, and the suite's fast cases. This case drives its
# choice over a plan and durations it writes. It proves four things: a cheap gate runs and
# its failure refuses the push; a gate that reaches the network, one with no recorded
# duration, one over the per-gate limit and one past the budget are each left out with a line
# that says why; a missing durations file or an uncomputable plan refuses the push; and a case
# that reads stdin no longer hangs the runner.
. "$ROOT/test/lib.sh"

R="$ROOT/scripts/ci/run-plan"
fail() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- the choice
gate() { printf '{"id":"%s","job":"structure","runs":"%s","selected":true,"network":%s,"reason":"t"}' "$1" "$2" "$3"; }
plan_with() {
  printf '{"schema":1,"mode":"test","reason":"a fixture","selected":[],"gates":[%s]}' "$(IFS=,; echo "$*")" > "$T/plan.json"
}
printf 'cheap-ok\t2\ncheap-fails\t3\nnetworked\t1\ntoo-slow\t400\nbudget-a\t10\nbudget-b\t12\n' > "$T/durations.tsv"
: > "$T/cases.tsv"
export MJ_PUSH_DURATIONS="$T/durations.tsv" MJ_PUSH_CASES="$T/cases.tsv" MJ_PUSH_GATE_BUDGET=20
plan_with "$(gate cheap-ok true false)" "$(gate cheap-fails 'echo the reason it failed; false' false)" \
  "$(gate networked true true)" "$(gate unrecorded true false)" "$(gate too-slow true false)" \
  "$(gate budget-a true false)" "$(gate budget-b true false)"
expect_exit 1 "$R" --before-push --plan "$T/plan.json"
expect_grep '  ok     cheap-ok'
expect_grep '  FAILED cheap-fails .*: echo the reason it failed; false'
expect_grep '\| the reason it failed'
expect_grep 'left out networked: it reaches the network'
expect_grep 'left out unrecorded: no recorded duration'
expect_grep 'left out too-slow: CI recorded 400s, over the 15s a push spends on one gate'
expect_grep 'left out budget-b: 12s more would pass the 20s budget for gates'
expect_grep '  ok     budget-a'
expect_grep 'failed: cheap-fails. CI would refuse this push for the same reason'
expect_grep 'against a bound of 180s'
# without the failing gate, the push goes on
plan_with "$(gate cheap-ok true false)" "$(gate networked true true)"
expect_exit 0 "$R" --before-push --plan "$T/plan.json"
expect_grep 'nothing CI would find here first'
# over the bound is a report, never a refusal: a busy machine must not fail a push
MJ_PUSH_BOUND=-1 expect_exit 0 "$R" --before-push --plan "$T/plan.json"
expect_grep 'over it by [0-9]+s; slowest: cheap-ok [0-9]+s; .*the push is not refused for that'
expect_grep 'nothing CI would find here first'

# ---------------------------------------------------------------- fail closed
MJ_PUSH_DURATIONS="$T/no-such-file" expect_exit 12 "$R" --before-push --plan "$T/plan.json"
expect_grep 'is missing, so nothing can be budgeted and the push is refused'
unset MJ_PUSH_DURATIONS MJ_PUSH_CASES MJ_PUSH_GATE_BUDGET
expect_exit 12 "$R" --before-push --base no-such-ref-anywhere
expect_grep 'the plan could not be computed .* so the push is refused'

# ---------------------------------------------------------------- the model and the wiring
grep -qx 'scripts/ci/run-plan --before-push || exit \$?' "$ROOT/.githooks/pre-push" \
  || fail 'the pre-push hook does not call run-plan --before-push'
grep -q 'name: structure-gates-on-push' "$ROOT/.ai/repo/policy.yaml" \
  || fail 'the policy does not declare the pre-push enforcement, so doctor cannot hold it'
jq -e '.gates[] | select(.id == "github-projection") | .network == true' \
  <(cd "$ROOT" && scripts/ci-plan < /dev/null 2>/dev/null) >/dev/null \
  || fail 'the plan does not carry the network flag the model declares'

# ---------------------------------------------------------------- a case that reads stdin
# The runner, copied beside a case of its own that reads stdin, under a pipe that stays open
# for a minute: the case must see end-of-input at once and the runner report it, rather than
# wait as case 54 did. The pipe's writer outlives the runner by design, so completion is read
# from the runner's verdict line and not from the pipeline's exit.
mkdir -p runner/test/cases runner/.ai/repo/ci
cp "$ROOT/test/run.sh" runner/test/
printf 'cat > /dev/null\n' > runner/test/cases/zz_reads_stdin.sh
: > runner/.ai/repo/ci/suite-durations.tsv
( sleep 60 | bash runner/test/run.sh zz_reads_stdin > "$T/stdin.out" 2>&1 ) &
pipe=$!
for _ in $(seq 1 150); do grep -q 'zz_reads_stdin' "$T/stdin.out" 2>/dev/null && break; sleep 0.2; done
pkill -P "$pipe" 2>/dev/null || true; kill "$pipe" 2>/dev/null || true
grep -q '^ok   zz_reads_stdin' "$T/stdin.out" \
  || fail "a case that reads stdin did not finish under an open pipe: $(tail -3 "$T/stdin.out")"
