# majordomus-covers: none
# A derive records what each stage cost, in milliseconds, with the lock wait apart (I2521).
# scripts/derive printed whole seconds per stage on stderr and kept none of them, so there was
# no baseline to judge an incremental derive against. It now appends one JSON line per finished
# derive to a file of this machine's: the commit, the branch, the clock it read, the load, which
# binary it used, the total, the lock wait, and every stage with its duration.
#
# A whole derive is too slow for a case, so this runs the script's own timing functions, read
# out of scripts/derive itself, so the case cannot drift from them:
#
#   1  two stages and a record: the line parses, names both stages in order with whole
#      millisecond durations, and carries the lock wait and the commit
#   2  MAJORDOMUS_DERIVE_TIMINGS set empty writes nothing
#   3  `scripts/derive --baseline <commit>` summarises that commit's records into
#      derive-<commit>.json: sample count, machine, load, each stage's median and spread, and
#      the stage that was the largest share in most samples; the same samples give the same
#      bytes, and fewer than three samples are refused (exit 12) with nothing written
. "$ROOT/test/lib.sh"

block="$T/timing.sh"
awk '/^# Milliseconds as well/{f=1} f{print} f&&/^timings_record\(\)/{t=1} t&&/^}$/{exit}' \
  "$ROOT/scripts/derive" > "$block"
grep -q '^timings_record()' "$block" && grep -q '^step_done()' "$block" \
  || { echo "    scripts/derive no longer defines its timing block where this case reads it"; exit 1; }

run_block() {   # run_block <timings file or empty>
  ( set -eu; ROOT="$ROOT"; export MAJORDOMUS_DERIVE_TIMINGS="$1"
    # shellcheck disable=SC1090
    . "$block"
    # shellcheck disable=SC2034  # read by timings_record in the sourced block
    LOCK_WAIT_MS=1234
    step "stage A: one"; sleep 0.05; step_done
    step "stage B: two"; step_done
    timings_record ) 2> "$T/stderr"
}

# ---------------------------------------------------------------- 1. a record
rec="$T/timings.jsonl"
run_block "$rec" || { echo "    the timing block failed"; cat "$T/stderr"; exit 1; }
[ "$(wc -l < "$rec" | tr -d ' ')" = 1 ] || { echo "    expected one record line"; cat "$rec"; exit 1; }
jq -e --arg head "$(git -C "$ROOT" rev-parse HEAD)" '
  .schema == 1 and .commit == $head and .lock_wait_ms == 1234
  and ([.stages[].stage] == ["stage A: one", "stage B: two"])
  and (.stages | all(.ms | type == "number" and . >= 0 and floor == .))
  and (.total_ms | type == "number") and (.clock == "ms" or .clock == "s")
  and (.binary == "built" or .binary == "borrowed")' "$rec" >/dev/null \
  || { echo "    the record does not carry what it should:"; cat "$rec"; exit 1; }
grep -qE '==> derive: stage A: one: [0-9]+s \([0-9]+ms\)' "$T/stderr" \
  || { echo "    stderr does not report the stage in seconds and milliseconds:"; cat "$T/stderr"; exit 1; }

# ---------------------------------------------------------------- 2. turned off
run_block "" || { echo "    the timing block failed with the record turned off"; cat "$T/stderr"; exit 1; }
[ "$(wc -l < "$rec" | tr -d ' ')" = 1 ] || { echo "    an empty MAJORDOMUS_DERIVE_TIMINGS still wrote a record"; exit 1; }

# ---------------------------------------------------------------- 3. the committed summary
head="$(git -C "$ROOT" rev-parse HEAD)"
fx="$T/fixture.jsonl"; out="$T/baseline"
sample() {   # sample <load> <lock wait> <A ms> <B ms>
  jq -cn --arg c "$head" --arg load "$1 1.00 1.00" --argjson w "$2" --argjson a "$3" --argjson b "$4" \
    '{schema: 1, at: "2026-11-08T00:00:00Z", commit: $c, branch: "x", clock: "ms", load: $load,
      binary: "built", total_ms: ($w + $a + $b), lock_wait_ms: $w,
      stages: [{stage: "stage A", ms: $a}, {stage: "stage B", ms: $b}]}'
}
{ sample 2.00 10 100 900; sample 4.00 20 300 700; } > "$fx"
rc=0; MAJORDOMUS_DERIVE_TIMINGS="$fx" MAJORDOMUS_DERIVE_BASELINE_DIR="$out" \
  "$ROOT/scripts/derive" --baseline HEAD 2> "$T/stderr" || rc=$?
[ "$rc" = 12 ] && [ ! -e "$out/derive-${head:0:11}.json" ] \
  || { echo "    two samples were not refused with exit 12 (got $rc)"; cat "$T/stderr"; exit 1; }
{ sample 3.00 30 200 600; sample 1.00 0 0 0 | jq -c '.commit = "other"'; } >> "$fx"
for i in 1 2; do
  MAJORDOMUS_DERIVE_TIMINGS="$fx" MAJORDOMUS_DERIVE_BASELINE_DIR="$out" \
    "$ROOT/scripts/derive" --baseline HEAD 2> "$T/stderr" \
    || { echo "    --baseline failed on three samples"; cat "$T/stderr"; exit 1; }
  cp "$out/derive-${head:0:11}.json" "$T/run$i.json"
done
cmp -s "$T/run1.json" "$T/run2.json" || { echo "    the same samples gave different bytes"; exit 1; }
jq -e --arg head "$head" '
  .commit == $head and .samples == 3 and (.machines | length) == 1
  and (.machines[0].machine | length) > 0 and .load_1m.median == 3
  and .lock_wait == {min: 10, median: 20, p90: 30, max: 30}
  and .work.median == 1000
  and ([.stages[] | {stage, samples, median}]
       == [{stage: "stage A", samples: 3, median: 200}, {stage: "stage B", samples: 3, median: 700}])
  and .largest == {stage: "stage B", largest_in: 3, of: 3, share_median: 0.75}' \
  "$T/run1.json" >/dev/null \
  || { echo "    the summary does not carry what it should:"; cat "$T/run1.json"; exit 1; }

echo "    a derive records each stage in milliseconds, the lock wait apart, the record can be turned off, and one commit's records summarise into a committed baseline"
