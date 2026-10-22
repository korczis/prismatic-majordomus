# majordomus-covers: none
# claims: none
# A step that runs out of time says so itself, rather than being killed by its job and
# reported as a cancellation.
#
# The defect this pins is not slowness. GitHub reports a job killed by its own
# `timeout-minutes` as `cancelled`, and every reader in this repository — the gates, the
# summaries, and three sessions on the night this was found — takes `cancelled` for "not a
# failure". Measured on master: the site job's "audit the UI in a browser" step was killed
# at exactly its job's 50-minute bound at 6a8d47a47 (01:14 -> 02:04), at 70685d473
# (23:13 -> 00:03), and on both runs of the integration branch. Three runs to the second,
# nothing red, nobody told.
#
# The cause was in the instrument: scripts/ci/timed measured the elapsed time *after* the
# command returned, so a command that never returned was never measured. A verdict with no
# measurement behind it.
. "$ROOT/test/lib.sh"

TIMED="$ROOT/scripts/ci/timed"
CHECK="$ROOT/scripts/ci/step-bound-check"
expect_file "$TIMED"
expect_file "$CHECK"

rows="$T/rows.tsv"

# --- a bound is not optional. A step with no bound is the defect, so it is refused at the
#     one place that could have reported it.
expect_exit 2 "$TIMED" g s "$rows" true
expect_grep 'a step with no bound cannot report that it ran out of time'

# --- a command inside its bound is untouched, and its row is written
expect_exit 0 "$TIMED" --bound 30 gate step "$rows" true
grep -q "$(printf 'gate\tstep\t')" "$rows" || { echo "    no timing row for a command that finished"; exit 1; }

# --- a command over its bound fails, names the bound, and STILL writes its row.
#     The row is the half that matters: before this, the measurement was lost exactly when
#     it was the only evidence of what went wrong.
before="$(grep -c . "$rows")"
expect_exit 124 "$TIMED" --bound 1 gate slow "$rows" sleep 20
expect_grep 'This is a failure, not a cancellation'
after="$(grep -c . "$rows")"
[ "$after" -gt "$before" ] || { echo "    the timing row was lost when the step ran out of time, which is when it is needed"; exit 1; }
grep -q "$(printf 'gate\tslow\t')" "$rows" || { echo "    the row does not name the step that ran out of time"; exit 1; }

# --- the relation the gate holds: every timed step declares a bound, and every bound is
#     smaller than the bound of the job that runs it. A step whose own bound is smaller
#     always loses the race to its own message, which is the whole invariant.
expect_exit 0 "$CHECK" "$ROOT/.github/workflows/validate.yml"
expect_grep 'every timed step is bounded under its job'

# --- and it fails on each half, against fixtures rather than against the real workflow, so
#     the case does not depend on a defect being present in the tree it runs in
cat > "$T/no-bound.yml" <<'Y'
jobs:
  site:
    timeout-minutes: 50
    steps:
      - run: scripts/ci/timed ui-audit ui-audit timings.tsv scripts/ui audit
Y
expect_exit 10 "$CHECK" "$T/no-bound.yml"
expect_grep 'carries no --bound'

cat > "$T/too-big.yml" <<'Y'
jobs:
  site:
    timeout-minutes: 50
    steps:
      - run: scripts/ci/timed --bound 3000 ui-audit ui-audit timings.tsv scripts/ui audit
Y
expect_exit 10 "$CHECK" "$T/too-big.yml"
expect_grep 'is not smaller than the job bound'

cat > "$T/no-job-bound.yml" <<'Y'
jobs:
  site:
    steps:
      - run: scripts/ci/timed --bound 3000 ui-audit ui-audit timings.tsv scripts/ui audit
Y
expect_exit 10 "$CHECK" "$T/no-job-bound.yml"
expect_grep 'declares no timeout-minutes'

echo "    a step that runs out of time reports it, and no step can be killed by its job first"
