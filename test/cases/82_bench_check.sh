# majordomus-exclusive: compares measured durations with thresholds; a busy pool would add the noise it refuses
# majordomus-covers: bench
# majordomus-negative: bench
#
# The regression gate: --check runs fresh and compares p50, p95 and p99 per target and mode
# against the baseline with the policy's fractions; a regression is a FAIL naming the metric,
# both values and the threshold, and exit 10; no baseline is 12; a baseline of another schema
# is not comparable and exits 15 rather than producing a number; a target the baseline does
# not know is reported, not compared.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq is required to edit the baseline"; exit 1; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
B=.ai/repo/benchmarks/baseline.json

# --- no baseline
expect_exit 12 "$MJ" bench version --samples 1 --warmup 0 --check --no-save
expect_grep "FAIL bench +baseline .* no baseline at $B"
# --- not comparable
mkdir -p .ai/repo/benchmarks && echo '{"schema":"majordomus/benchmark-result/v1","results":[]}' > "$B"
expect_exit 15 "$MJ" bench version --samples 1 --warmup 0 --check --no-save
expect_grep 'not comparable: schema is not majordomus/benchmark-baseline/v1'
rm -f "$B"

# --- within thresholds: the same command measured twice on one machine
expect_exit 0 "$MJ" bench version --samples 3 --warmup 1 --write-baseline --no-save
git add "$B" && git commit -qm baseline
expect_exit 0 "$MJ" bench version --samples 3 --warmup 1 --check --no-save
expect_grep 'OK +bench +version cold .* within thresholds'
expect_grep 'OK +bench +version warm .* within thresholds'

# --- a regression: the baseline claims the command took one millisecond
jq -c '.results |= map(.p50_ms = 1 | .p95_ms = 1 | .p99_ms = 1)' "$B" > "$B.tmp" && mv "$B.tmp" "$B"
expect_exit 10 "$MJ" bench version --samples 3 --warmup 1 --check --no-save
expect_grep 'FAIL bench +version warm .* regression: p50 1 -> [0-9]+ ms \(\+[0-9]+%, threshold 50%\)'
expect_grep 'FAIL bench +version warm .* regression: p95 1 -> [0-9]+ ms \(\+[0-9]+%, threshold 50%\)'
expect_grep 'FAIL bench +version warm .* regression: p99 1 -> [0-9]+ ms \(\+[0-9]+%, threshold 60%\)'
expect_grep 'FAIL bench +version cold .* regression: p50'
# --- the threshold is the policy's, not the code's
git checkout -q "$B"
jq -c '.results |= map(.p50_ms = 1)' "$B" > "$B.tmp" && mv "$B.tmp" "$B"
sed -i.bak 's/^    p50: 0.5$/    p50: 100000/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 0 "$MJ" bench version --samples 3 --warmup 1 --check --no-save
expect_grep 'within thresholds'
reset_policy; "$MJ" update >/dev/null; git checkout -q "$B"

# --- a target the baseline does not know is reported, never compared
expect_exit 0 "$MJ" bench version doctor --samples 1 --warmup 0 --mode cold --check --no-save
expect_grep 'INFO +bench +doctor cold .* not in the baseline; no comparison'
expect_grep 'OK +bench +version cold .* within thresholds'
echo "    no baseline 12, not comparable 15, regression 10 naming metric, values and threshold, threshold from the policy"
