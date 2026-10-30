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

echo "    a derive records each stage in milliseconds, the lock wait apart, and the record can be turned off"
