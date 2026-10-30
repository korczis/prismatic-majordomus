# majordomus-covers: none
# claims: none
# Every gate the CI model selects has a step in its job that says it ran, and that is read
# from the workflow before a run rather than from the verdict after one.
#
# scripts/ci/verdict asks every selected gate for a receipt. On run 39379416881 the site job
# was green and the `ci` check red: "gate site-languages was selected and its job site
# succeeded, but no step left a receipt for it". Its step ran the probe bare instead of
# through scripts/ci/timed, which is what writes the receipt, and only a whole CI run could
# say so. scripts/ci/step-bound-check --gates now holds, for each gate of
# .ai/repo/ci/gates.yaml, that its job dispatches it through scripts/ci/run-plan without
# --skip, runs it through scripts/ci/timed, or writes its id to $MJ_CI_RECEIPTS.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
GATES="$ROOT/.ai/repo/ci/gates.yaml"
WF="$ROOT/.github/workflows/validate.yml"
expect_file "$CHECK"
expect_file "$GATES"
expect_file "$WF"

# --- the good shape: this repository's workflow against its own model
expect_exit 0 "$CHECK" --gates "$GATES" "$WF"
expect_grep 'every gate of gates.yaml leaves a receipt'

# --- the broken shape of run 39379416881: the site-languages step un-timed, nothing else moved
sed 's#scripts/ci/timed --bound [0-9]* site-languages site-languages timings.tsv node#node#' "$WF" > bare.yml
cmp -s "$WF" bare.yml && { echo "    the mutation did not apply: no timed site-languages step in validate.yml to un-time"; exit 1; }
expect_exit 10 "$CHECK" --gates "$GATES" bare.yml
expect_grep 'gate site-languages runs in job site, and no step of it'
expect_no_grep 'gate site-build '   # the other gates of the job still leave theirs

# --- a gate that run-plan is told to skip owes a receipt of its own: the rust job skips
#     rust-check and writes it per lane; drop that line and the gate is silent
grep -v "printf 'rust-check" "$WF" > skipped.yml
cmp -s "$WF" skipped.yml && { echo "    the mutation did not apply: validate.yml writes no rust-check receipt of its own"; exit 1; }
expect_exit 10 "$CHECK" --gates "$GATES" skipped.yml
expect_grep 'gate rust-check runs in job rust'

# --- a gate whose job the workflow does not have is a finding, not a pass over nothing
cat > model.yaml <<'Y'
version: 1
gates:
  - id: lonely
    job: nowhere
    runs: true
Y
cat > tiny.yml <<'Y'
jobs:
  site:
    timeout-minutes: 10
    steps:
      - run: echo hello
Y
expect_exit 10 "$CHECK" --gates model.yaml tiny.yml
expect_grep 'gate lonely runs in job nowhere, which this workflow does not have'

# --- each of the three ways of leaving a receipt is accepted
cat > model.yaml <<'Y'
version: 1
gates:
  - id: planned
    job: a
    runs: true
  - id: clocked
    job: b
    runs: true
  - id: written
    job: c
    runs: true
Y
cat > three.yml <<'Y'
jobs:
  a:
    timeout-minutes: 10
    steps:
      - run: scripts/ci/run-plan --plan plan.json --job a
  b:
    timeout-minutes: 10
    steps:
      - run: scripts/ci/timed --bound 60 clocked clocked timings.tsv true
  c:
    timeout-minutes: 10
    steps:
      - run: printf '%s\t%s\tc\n' "written" "success" >> "$MJ_CI_RECEIPTS"
Y
expect_exit 0 "$CHECK" --gates model.yaml three.yml

echo "    every gate of the model leaves a receipt in its job, and a bare gated step is refused before CI"
