# majordomus-covers: none
# A case near its budget is named before it fails (I2068).
#
# On 2026-10-08 95_executable_reference cost 3465 s against a 3450 s per-case budget on a slow
# runner and failed a suite whose every case passed; its cost had sat within a few percent of
# the budget for days and nothing said so. scripts/ci/suite-budget now names, by case, every
# case whose cost in this run or whose recorded cost in suite-durations.tsv lies within
# budget.near_percent of the per-case budget, as NEAR, and NEAR never changes the exit status:
# the budget is still the verdict, and no budget or bound is loosened (case 417 holds those).
. "$ROOT/test/lib.sh"

B="$ROOT/scripts/ci/suite-budget"
M="$ROOT/.ai/repo/ci/suite.yaml"
[ -x "$B" ] || { echo "    scripts/ci/suite-budget is missing or not executable"; exit 1; }

# ---------------------------------------------------------------- the margin is declared, at 5%
near="$(sed -n 's/^ *near_percent: *\([0-9][0-9]*\).*/\1/p' "$M" | head -1)"
[ "$near" = 5 ] || { echo "    .ai/repo/ci/suite.yaml declares near_percent '$near', not the 5 I2068 accepts"; exit 1; }

# a model of round numbers, so that the margins below are exact: 950 s is the first cost
# within 5% of a 1000 s case budget, 9500 s of a 10000 s suite
model="$T/suite.yaml"
printf 'version: 1\nbound_minutes: 225\ncase_bound_seconds: 2000\nbudget:\n  cases_seconds: 10000\n  slowest_case_seconds: 1000\n  near_percent: 5\n' > "$model"
run() { # report [durations] -> output in $T/out, status in $st
  st=0
  if [ -n "${2:-}" ]; then "$B" --model "$model" --report "$1" --durations "$2" > "$T/out" 2>&1 || st=$?
  else "$B" --model "$model" --report "$1" --durations "$T/none.tsv" > "$T/out" 2>&1 || st=$?; fi
}

# ---------------------------------------------------------------- this run's cost
printf 'cheap\tok\t10\tparallel\nedge\tok\t950\tparallel\nfar\tok\t949\tparallel\n' > "$T/near.tsv"
run "$T/near.tsv"
[ "$st" = 0 ] || { echo "    a run with every case inside its budget exited $st; NEAR is a warning, not a verdict"; cat "$T/out"; exit 1; }
grep -q '^NEAR    edge costs 950 s in this run, within 5% of the 1000 s one case may cost (50 s of room)$' "$T/out" \
  || { echo "    a case at 950 s of a 1000 s budget was not named as NEAR"; cat "$T/out"; exit 1; }
grep -qE '^NEAR    (far|cheap) ' "$T/out" && { echo "    a case outside the margin was named as NEAR"; cat "$T/out"; exit 1; }
grep -q 'within budget' "$T/out" || { echo "    a run inside its budget no longer says so"; cat "$T/out"; exit 1; }

# a case exactly at its budget is still within it, and still named
printf 'atbudget\tok\t1000\tparallel\n' > "$T/at.tsv"
run "$T/at.tsv"
[ "$st" = 0 ] && grep -q '^NEAR    atbudget costs 1000 s .*(0 s of room)$' "$T/out" \
  || { echo "    a case at exactly its budget: exit $st, or not named"; cat "$T/out"; exit 1; }

# ---------------------------------------------------------------- the recorded cost, before any run crosses it
# this run is comfortably inside, but the record says the case has run close: named anyway
printf 'slow\tok\t400\tparallel\n' > "$T/fine.tsv"
printf 'slow\t960\nquick\t100\nrunaway\t1200\n' > "$T/durations.tsv"
run "$T/fine.tsv" "$T/durations.tsv"
[ "$st" = 0 ] || { echo "    a recorded cost near the budget changed the exit status to $st"; cat "$T/out"; exit 1; }
grep -q "^NEAR    slow is recorded at 960 s in $T/durations.tsv, within 5% of the 1000 s one case may cost (40 s of room)$" "$T/out" \
  || { echo "    a case recorded at 960 s of a 1000 s budget was not named"; cat "$T/out"; exit 1; }
grep -q "^NEAR    runaway is recorded at 1200 s .*over the 1000 s one case may cost" "$T/out" \
  || { echo "    a case recorded over its budget was not named"; cat "$T/out"; exit 1; }
grep -q '^NEAR    quick ' "$T/out" && { echo "    a case recorded far from its budget was named"; cat "$T/out"; exit 1; }
# an absent record is not a failure: only this run's costs are held to the margin
run "$T/fine.tsv"
[ "$st" = 0 ] && ! grep -q '^NEAR' "$T/out" || { echo "    an absent durations file: exit $st or a NEAR line"; cat "$T/out"; exit 1; }

# ---------------------------------------------------------------- the suite total
printf 'a\tok\t900\tparallel\nb\tok\t900\tparallel\n' > "$T/t1.tsv"
for i in 1 2 3 4 5 6 7 8 9; do printf 'c%s\tok\t860\tparallel\n' "$i"; done >> "$T/t1.tsv"
run "$T/t1.tsv"   # 1800 + 7740 = 9540 s of a 10000 s suite
[ "$st" = 0 ] && grep -q '^NEAR    the suite costs 9540 s, within 5% of the 10000 s it may cost (460 s of room)$' "$T/out" \
  || { echo "    a suite at 9540 s of 10000: exit $st, or not named"; cat "$T/out"; exit 1; }

# ---------------------------------------------------------------- the verdict is unchanged
printf 'over\tok\t1001\tparallel\nedge\tok\t960\tparallel\n' > "$T/over.tsv"
run "$T/over.tsv"
[ "$st" = 10 ] || { echo "    a case over its budget exited $st, expected 10: NEAR must not soften a verdict"; cat "$T/out"; exit 1; }
grep -q '^FAILED  over costs 1001 s' "$T/out" || { echo "    the refusal no longer names the case"; cat "$T/out"; exit 1; }
grep -q '^NEAR    over' "$T/out" && { echo "    a case over its budget was called NEAR rather than refused"; cat "$T/out"; exit 1; }
# a model with no margin cannot say what NEAR means: it is not measured, and not a pass
grep -v near_percent "$model" > "$T/nomargin.yaml"
st=0; "$B" --model "$T/nomargin.yaml" --report "$T/near.tsv" > "$T/out" 2>&1 || st=$?
[ "$st" = 12 ] || { echo "    a model without near_percent exited $st, expected 12"; cat "$T/out"; exit 1; }

# ---------------------------------------------------------------- the deal's record
# The shards are dealt from the record, so a case it lacks, or one that grew past it, is
# named; --write-durations writes the record this run measured. Neither changes the verdict.
printf 'cheap\t10\nedge\t100\nidle\t42\n' > "$T/rec.tsv"
printf 'cheap\tok\t10\tparallel\nedge\tok\t950\tparallel\nnew\tok\t5\tparallel\nidle\tskip\t1\tparallel\n' > "$T/drift.tsv"
run "$T/drift.tsv" "$T/rec.tsv"
[ "$st" = 0 ] || { echo "    STALE or DRIFT changed the verdict: exit $st"; cat "$T/out"; exit 1; }
grep -q '^STALE   1 case(s) of this run are not in ' "$T/out" && grep -qx '          new' "$T/out" \
  || { echo "    the case the record lacks was not named"; cat "$T/out"; exit 1; }
grep -q '^DRIFT   edge cost 950 s in this run, recorded at 100 s' "$T/out" \
  || { echo    "    a case at 9.5 times its record was not named as DRIFT"; cat "$T/out"; exit 1; }
grep -q '^DRIFT   cheap' "$T/out" && { echo "    a case at its record was named as DRIFT"; cat "$T/out"; exit 1; }
st=0; "$B" --model "$model" --report "$T/drift.tsv" --durations "$T/rec.tsv" --write-durations "$T/w.tsv" > "$T/out" 2>&1 || st=$?
[ "$st" = 0 ] || { echo "    --write-durations changed the verdict: exit $st"; cat "$T/out"; exit 1; }
printf 'cheap\t10\nedge\t950\nidle\t42\nnew\t5\n' | cmp -s - "$T/w.tsv" \
  || { echo "    the written record is not this run's passed costs over the recorded rest (a skipped idle keeps its 42):"; cat "$T/w.tsv"; exit 1; }

# ---------------------------------------------------------------- this repository's own model and record
st=0; (cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/near.tsv" > "$T/own.out" 2>&1) || st=$?
[ "$st" = 0 ] || { echo "    the repository's own model and record refused a cheap run: $st"; cat "$T/own.out"; exit 1; }

echo "    a case within ${near}% of its budget, in this run or in the record, is NEAR by name; the verdict, the budgets and the bound are unchanged"
