# majordomus-covers: bench
# majordomus-negative: bench
# bench compare puts any two persisted runs side by side and names the numbers that matter
# (I0838, milestone M004).
#
# `bench --check` compares a fresh run with the baseline; nothing compared two runs a person
# already has, and the report was a wall of numbers in which the one regression that matters
# looks like every other line. `bench compare <run-a> <run-b>` reads two run files — an id
# under .ai/local/benchmarks/runs/, a path, `latest` or `baseline` — prints the table --check
# prints, and ends with the tail a reader looks for first: the largest p95 regressions, the
# largest improvements, and the slowest targets of the second run. It writes nothing.
#
# The runs here are fixtures in the shape `bench` saves, so the ranking is decided by numbers
# this case chose rather than by how fast this machine is.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

R=.ai/local/benchmarks/runs
mkdir -p "$R"
# run <file> <command mode p50 p95 p99>...
run() {
  local f="$1" first=1; shift
  { printf '{"schema":"majordomus/benchmark-result/v1","results":['
    while [ $# -ge 5 ]; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"command":"%s","mode":"%s","class":"read-only","scenario":"default","status":"ok","samples":5,"min_ms":1,"p50_ms":%s,"p90_ms":%s,"p95_ms":%s,"p99_ms":%s,"max_ms":%s,"mean_ms":%s,"stddev_ms":1}' \
        "$1" "$2" "$3" "$4" "$4" "$5" "$5" "$3"
      shift 5
    done
    printf ']}\n'; } > "$f"
}
run "$R/r1.json" doctor warm 100 120 130  check warm 40 50 55  plan warm 80 90 95  status warm 10 12 14
run "$R/r2.json" doctor warm 130 150 160  check warm 44 55 60  plan warm 60 70 75  status warm 10 12 14
before="$(find .ai/local/benchmarks -type f | LC_ALL=C sort | xargs cksum)"
# compare_ok <args>: a comparison that must succeed, its output kept in $OUT for the checks
OUT="$T/compare.out"
compare_ok() { expect_exit 0 "$MJ" "$@"; printf '%s\n' "$LAST_OUT" > "$OUT"; }

# ---------------------------------------------------------------- 1. a run against itself
compare_ok bench compare r1 r1
expect_grep 'doctor +warm  100/120/130 -> 100/120/130'
[ "$(grep -A1 'largest p95 regressions' "$OUT" | tail -n 1)" = '  none' ] \
  || { echo "    a run compared with itself reports a regression:"; sed 's/^/    | /' "$OUT"; exit 1; }
[ "$(grep -A1 'largest p95 improvements' "$OUT" | tail -n 1)" = '  none' ] \
  || { echo "    a run compared with itself reports an improvement:"; sed 's/^/    | /' "$OUT"; exit 1; }

# ---------------------------------------------------------------- 2. the ranked tail
compare_ok bench compare r1 "$R/r2.json"
reg="$(sed -n '/largest p95 regressions/,/largest p95 improvements/p' "$OUT" | grep '^  +')"
[ "$(printf '%s\n' "$reg" | head -n 1)" = '  +30.0 ms  doctor warm' ] \
  && [ "$(printf '%s\n' "$reg" | sed -n 2p)" = '  +5.0 ms  check warm' ] \
  && [ "$(printf '%s\n' "$reg" | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    the regressions are not ranked largest first, or include what did not regress:"; sed 's/^/    | /' "$OUT"; exit 1; }
grep -A1 'largest p95 improvements' "$OUT" | grep -q '^  -20.0 ms  plan warm$' \
  || { echo "    the improvement is not named:"; sed 's/^/    | /' "$OUT"; exit 1; }
grep -A1 'slowest by p95 in' "$OUT" | grep -q '^  150 ms  doctor warm$' \
  || { echo "    the slowest target of the second run is not first:"; sed 's/^/    | /' "$OUT"; exit 1; }
grep -E '^  \+.*status' "$OUT" && { echo "    an unchanged command is listed as a regression"; exit 1; }

# ---------------------------------------------------------------- 3. json carries the deltas
compare_ok --json bench compare r1 r2
grep -q '"command":"doctor","mode":"warm","a":"100/120/130","b":"130/150/160","p95_delta_ms":30.0' "$OUT" \
  || { echo "    --json does not carry the p95 delta:"; sed 's/^/    | /' "$OUT"; exit 1; }

# ---------------------------------------------------------------- 4. what it refuses
expect_exit 2 "$MJ" bench compare r1
expect_exit 12 "$MJ" bench compare r1 nosuch
expect_grep "no run 'nosuch'"
expect_exit 12 "$MJ" bench compare r1 baseline

# ---------------------------------------------------------------- 5. it writes nothing
after="$(find .ai/local/benchmarks -type f | LC_ALL=C sort | xargs cksum)"
[ "$before" = "$after" ] || { echo "    bench compare changed the benchmark store"; diff <(echo "$before") <(echo "$after") | sed 's/^/    | /'; exit 1; }
echo "    ok: compare ranks regressions and improvements, a run against itself shows none, and it writes nothing"
