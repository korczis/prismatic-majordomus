# majordomus-covers: bench
# majordomus-negative: bench
#
# The benchmark command: its targets are the public commands of share/commands.yaml and
# follow a mutation of the registry; a run records cold and warm distributions separately,
# with the nine statistics, as a versioned JSON document persisted under the local half of
# the layer and never treated as a baseline; a mutating command gets only a cold
# distribution; usage errors and an unknown target are refused by exit code.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq is required to read the run"; exit 1; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# --- the targets are the registry's public commands, minus the harness itself
expect_exit 0 "$MJ" bench --list
public="$(MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" bash -c '. "$MJ_LIB_DIR/common.sh"; . "$MJ_LIB_DIR/commands.sh"; mj_cmdreg_load; mj_cmdreg_public' | grep -vx bench)"
for c in $public; do expect_grep "^$c +" ; done
expect_no_grep '^bench +'
listed="$(printf '%s\n' "$LAST_OUT" | awk 'NR > 1 { print $1 }' | sort)"
[ "$listed" = "$(printf '%s\n' "$public" | sort)" ] || { echo "    --list does not equal the registry's public commands"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_exit 0 "$MJ" bench --list --format json
printf '%s\n' "$LAST_OUT" | jq -e '.targets | map(.command) | index("doctor")' >/dev/null || { echo "    --list --format json does not carry the targets"; exit 1; }

# --- a registry mutation moves the target list without any edit here: a copy of the tool
#     gains a public command, and the copy's bench lists it
C="$(mktemp -d "${TMPDIR:-/tmp}/mj-bench-copy.XXXXXX")"; trap 'rm -rf "$C"' EXIT
fixture_repo "$C" test/fixtures/commands
python3 - "$C/share/commands.yaml" <<'PY' 2>/dev/null || { echo "    python3 absent; skipping the registry mutation"; exit 0; }
import sys
p = sys.argv[1]; s = open(p).read()
i = s.index('  - id: help\n')
entry = '''  - id: futurecmd
    title: futurecmd
    summary: A command added to the registry alone.
    category: system
    stage: inspect
    visibility: public
    class: read-only
    requires_repository: true
    requires_installed: true
    requires_task: none
    json: false
    syntax: majordomus futurecmd
    reads: []
    writes: []
    exit_codes: [0]
    demo: none
    note: Exists only for this case.

'''
open(p, 'w').write(s[:i] + entry + s[i:])
PY
grep -q '^  - id: futurecmd$' "$C/share/commands.yaml" || { echo "    the registry probe did not take"; exit 1; }
expect_exit 0 "$C/bin/majordomus" bench --list
expect_grep '^futurecmd +read-only'

# --- a run: cold and warm distributions, nine statistics, persisted as a versioned document
expect_exit 0 "$MJ" bench version --samples 3 --warmup 1
expect_grep '^version +cold +ok +1 '
expect_grep '^version +warm +ok +3 '
expect_grep 'run b-[0-9T]*Z-[0-9a-f]* saved as \.ai/local/benchmarks/runs/'
[ -f .ai/local/benchmarks/latest.json ] || { echo "    latest.json was not written"; exit 1; }
[ "$(wc -l < .ai/local/benchmarks/history.jsonl | tr -d ' ')" = 1 ] || { echo "    history.jsonl does not carry one line per run"; exit 1; }
jq -e '.schema == "majordomus/benchmark-result/v1"' .ai/local/benchmarks/latest.json >/dev/null || { echo "    the run does not carry its schema"; exit 1; }
jq -e '.repository.commit | length == 40' .ai/local/benchmarks/latest.json >/dev/null || { echo "    the run does not record the commit"; exit 1; }
jq -e '.results | length == 2' .ai/local/benchmarks/latest.json >/dev/null || { echo "    expected one cold and one warm result"; exit 1; }
jq -e '.results[] | select(.mode == "warm") | .samples == 3 and .min_ms <= .p50_ms and .p50_ms <= .p95_ms and .p95_ms <= .p99_ms and .p99_ms <= .max_ms and .stddev_ms >= 0' .ai/local/benchmarks/latest.json >/dev/null \
  || { echo "    the warm statistics are not ordered min <= p50 <= p95 <= p99 <= max"; cat .ai/local/benchmarks/latest.json; exit 1; }
jq -e '.results[] | select(.mode == "cold") | .samples == 1' .ai/local/benchmarks/latest.json >/dev/null || { echo "    a read-only command has one cold sample"; exit 1; }
# local evidence is ignored by git: nothing to commit after a run
[ -z "$(git status --porcelain)" ] || { echo "    a run changed the tracked tree"; git status --porcelain; exit 1; }

# --- a mutating command: every sample is its own repository and every sample is cold
expect_exit 0 "$MJ" bench start --samples 2 --warmup 0 --no-save --format json
printf '%s\n' "$LAST_OUT" | jq -e '.results | length == 1 and .[0].mode == "cold" and .[0].samples == 2 and .[0].class == "state-mutating"' >/dev/null \
  || { echo "    a mutating command must record two cold samples and no warm distribution"; printf '%s\n' "$LAST_OUT"; exit 1; }
[ "$(wc -l < .ai/local/benchmarks/history.jsonl | tr -d ' ')" = 1 ] || { echo "    --no-save wrote to the history"; exit 1; }

# --- refusals by exit code
expect_exit 12 "$MJ" bench nosuch
expect_grep 'not a public command of the registry'
expect_exit 2 "$MJ" bench --samples x
expect_exit 2 "$MJ" bench --mode lukewarm
expect_exit 2 "$MJ" bench --bogus
expect_exit 12 "$MJ" bench version --samples 1 --warmup 0 --no-save --check
expect_grep 'no baseline'

# --- the percentile implementation, on a dataset small enough to check by hand:
#     nearest rank over 1..10 gives p50 5, p90 9, p95 10, p99 10
stats="$(MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" bash -c '. "$MJ_LIB_DIR/common.sh"; . "$MJ_LIB_DIR/bench.sh"; seq 1 10 | mj_bench_stats')"
[ "$stats" = "$(printf '10\t1\t5\t9\t10\t10\t10\t5.5\t2.9')" ] || { echo "    percentiles over 1..10 are wrong: $stats"; exit 1; }
echo "    targets follow the registry; cold and warm are separate; the run is persisted local evidence"
