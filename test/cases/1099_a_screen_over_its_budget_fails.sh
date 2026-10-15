# majordomus-covers: none
# A screen of the UI over its latency budget fails `bench --check` (I2489, ADR 0142,
# project.no-screen-waits-for-a-source), through the real executable over a fixture:
#
#   over      a budget no screen can meet fails the check with exit 10, with no baseline to
#             compare, and the report names the screen, its value and its bound
#   within    a budget every screen meets passes, and says so
#   declared  the repository's own policy declares the UI's budgets, for the pinned and the
#             live view, so the first two are about a bound that is really held
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

policy() {
  mkdir -p .ai/repo/benchmarks/rust
  printf 'version: 1\nbudgets:\n  - prefix: system.http.ui.\n    cache: warm\n    metric: p95\n    milliseconds: %s\n    minimum_samples: 1\n' "$1" \
    > .ai/repo/benchmarks/rust/policy.yaml
  git add -A >/dev/null && git commit -qm "a budget of $1 ms" >/dev/null
}
check() { "$RB" bench system.http.ui.index --transport system --profile quick --no-write --check 2>/dev/null; }

# ---------------------------------------------------------------- over
policy 0.0001
rc=0; out="$(check)" || rc=$?
[ "$rc" = 10 ] || { echo "    a screen over its budget exited $rc, not 10:"; printf '%s\n' "$out" | tail -n 5 | sed 's/^/    | /'; exit 1; }
printf '%s' "$out" | grep -q 'system.http.ui.index .*OVER' \
  || { echo "    the report does not name the screen that is over its budget"; printf '%s\n' "$out" | tail -n 5 | sed 's/^/    | /'; exit 1; }
printf '%s' "$out" | grep -q 'bench --check: over budget' || { echo "    the report does not say the run is over budget"; exit 1; }
printf '%s' "$out" | grep -q 'no baseline for' || { echo "    the fixture has a baseline, so this did not prove the budget needs none"; exit 1; }

# ---------------------------------------------------------------- within
policy 600000
rc=0; out="$(check)" || rc=$?
[ "$rc" = 0 ] || { echo "    a screen within its budget exited $rc:"; printf '%s\n' "$out" | tail -n 5 | sed 's/^/    | /'; exit 1; }
printf '%s' "$out" | grep -q 'system.http.ui.index .*PASS' || { echo "    the report does not show the screen within its budget"; exit 1; }
printf '%s' "$out" | grep -q 'bench --check: within budget' || { echo "    the report does not say the run is within budget"; exit 1; }

# ---------------------------------------------------------------- declared
for prefix in system.http.ui. system.http.live.ui.; do
  grep -q "prefix: $prefix\$" "$ROOT/.ai/repo/benchmarks/rust/policy.yaml" \
    || { echo "    the repository's benchmark policy declares no budget for $prefix"; exit 1; }
done

echo "    a screen over its budget fails the check and is named; one within it passes; the policy declares both views"
