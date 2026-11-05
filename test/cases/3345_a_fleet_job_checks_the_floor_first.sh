# majordomus-covers: none
# A job a machine of the fleet can take checks the disk floor first, and coverage builds there
# in a directory of its own that it removes (I4242).
#
# Run 41976073596, the first trial of lundra as validate's first wave, took lundra from 76 GiB
# free to none: its coverage job wrote 72 GB of instrumented build into the runner's shared
# build-dir, which persists between jobs on a self-hosted runner with no cache to bound it, and
# every user of the machine lost its disk. What must stay true:
#
#   1  the workflow declares the floor once, at its top: MJ_FLEET_FLOOR_GIB is the 35 GiB the
#      cooperation rule names ("none starts below 35 GiB free"), MJ_FLEET_TEST_FLOOR_GIB ten
#      more for a test or coverage build;
#   2  the first step of every job that takes the planned runner is the floor, guarded by
#      runner.environment == 'self-hosted', run outside the checkout, and reading the test
#      floor in suite, coverage, cockpit and mesh-lab and the build floor elsewhere;
#   3  that step refuses below its floor with exit 75 and a reason, and passes at or above it
#      (its own script, run with a df that answers a chosen number);
#   4  coverage sets CARGO_BUILD_BUILD_DIR under the runner's temp on a self-hosted runner
#      before it measures, and an always() step removes it before the last step.
#
# Read from the tracked workflow with awk; nothing is built, nothing reaches the network.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
R="$ROOT/.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md"
[ -f "$W" ] || { echo "    .github/workflows/validate.yml is missing"; exit 1; }
[ -f "$R" ] || { echo "    the cooperation rule is missing"; exit 1; }
EXPR="\${{ fromJSON(needs.plan.outputs.runner || '\"ubuntu-latest\"') }}"
TEST=" suite coverage cockpit mesh-lab "
fail=0

# 1
grep -q 'none starts below 35 GiB free' "$R" || { echo "    the cooperation rule no longer names a 35 GiB floor"; fail=1; }
top="$(awk '/^jobs:/ { exit } { print }' "$W")"
printf '%s\n' "$top" | grep -qx '  MJ_FLEET_FLOOR_GIB: 35' || { echo "    the workflow does not declare MJ_FLEET_FLOOR_GIB: 35 above its jobs"; fail=1; }
printf '%s\n' "$top" | grep -qx '  MJ_FLEET_TEST_FLOOR_GIB: 45' || { echo "    the workflow does not declare MJ_FLEET_TEST_FLOOR_GIB: 45 above its jobs"; fail=1; }

# the lines of one job, from its key to the next job's
job_lines() {
  awk -v j="$2" '
    /^jobs:/ { injobs = 1; next }
    !injobs { next }
    /^  [a-z0-9_-]+:[ \t]*$/ { name = $0; gsub(/[ :\t]/, "", name); on = (name == j); next }
    on { print }' "$1"
}
# the first step of a job, without comment lines
first_step() {
  job_lines "$1" "$2" | awk '
    /^    steps:/ { s = 1; next }
    !s { next }
    /^      #/ { next }
    /^      - / { n++ }
    n == 1 { print }
    n > 1 { exit }'
}
# the planned-runner jobs, read from the workflow rather than listed here
planned="$(awk -v e="$EXPR" '
  /^jobs:/ { injobs = 1; next }
  !injobs { next }
  /^  [a-z0-9_-]+:[ \t]*$/ { job = $0; gsub(/[ :\t]/, "", job); next }
  /^    runs-on:/ { if (index($0, e) > 0) print job }' "$W")"
[ "$(printf '%s\n' "$planned" | grep -c .)" -ge 5 ] || { echo "    fewer than five jobs take the planned runner"; fail=1; }

# 2
for job in $planned; do
  step="$(first_step "$W" "$job")"
  want=MJ_FLEET_FLOOR_GIB; case "$TEST" in *" $job "*) want=MJ_FLEET_TEST_FLOOR_GIB ;; esac
  printf '%s\n' "$step" | grep -q "^      - name: the fleet's disk floor$" \
    || { echo "    the first step of $job is not the fleet's disk floor"; fail=1; continue; }
  printf '%s\n' "$step" | grep -q "^        if: runner.environment == 'self-hosted'$" \
    || { echo "    $job's floor is not guarded by runner.environment"; fail=1; }
  printf '%s\n' "$step" | grep -q '^        working-directory: \${{ runner.temp }}$' \
    || { echo "    $job's floor runs inside a checkout that does not exist yet"; fail=1; }
  printf '%s\n' "$step" | grep -q "^          FLOOR_GIB: \${{ env.$want }}\$" \
    || { echo "    $job's floor does not read $want"; fail=1; }
done
[ "$fail" = 0 ] || exit 1
echo "    every job of the planned runner checks the declared floor first"

# 3: the step's own script, with a df that answers a chosen number of GiB
script="$(first_step "$W" coverage | awk '/^        run: \|/ { r = 1; next } r && /^          / { sub(/^          /, ""); print; next } r { exit }')"
[ -n "$script" ] || { echo "    coverage's floor has no run block"; exit 1; }
T="$(mktemp -d "${TMPDIR:-/tmp}/mj-floor.XXXXXX")"; trap 'rm -rf "$T"' EXIT
mkdir "$T/bin"
cat > "$T/bin/df" <<'STUB'
#!/bin/sh
printf 'Avail\n%sG\n' "$FAKE_FREE"
STUB
chmod +x "$T/bin/df"
printf '%s\n' "$script" > "$T/floor.sh"
out="$(cd "$T" && FAKE_FREE=44 FLOOR_GIB=45 RUNNER_NAME=lundra-3 PATH="$T/bin:$PATH" bash "$T/floor.sh" 2>&1)" && rc=0 || rc=$?
[ "$rc" = 75 ] || { echo "    44 GiB under a 45 GiB floor exited $rc, not 75"; exit 1; }
printf '%s\n' "$out" | grep -q '^::error::lundra-3 has 44 GiB free, under this job.s floor of 45 GiB' \
  || { echo "    the refusal does not name the runner, the free space and the floor: $out"; exit 1; }
(cd "$T" && FAKE_FREE=45 FLOOR_GIB=45 RUNNER_NAME=lundra-3 PATH="$T/bin:$PATH" bash "$T/floor.sh" >/dev/null 2>&1) \
  || { echo "    45 GiB at a 45 GiB floor was refused"; exit 1; }
echo "    the floor refuses under it with exit 75 and the reason, and passes at it"

# 4
cov="$(job_lines "$W" coverage | grep -v '^ *#')"
set_at="$(printf '%s\n' "$cov" | grep -n 'CARGO_BUILD_BUILD_DIR=\$RUNNER_TEMP/' | head -n 1 | cut -d: -f1)"
measure_at="$(printf '%s\n' "$cov" | grep -n '^      - id: measure$' | cut -d: -f1)"
rm_at="$(printf '%s\n' "$cov" | grep -n '^      - name: the private build directory is removed$' | cut -d: -f1)"
last_at="$(printf '%s\n' "$cov" | grep -n '^      - name: a failed branch run ends here$' | cut -d: -f1)"
[ -n "$set_at" ] && [ -n "$measure_at" ] && [ "$set_at" -lt "$measure_at" ] \
  || { echo "    coverage does not set a private build directory under the runner's temp before it measures"; exit 1; }
[ -n "$rm_at" ] && [ -n "$last_at" ] && [ "$rm_at" -lt "$last_at" ] \
  || { echo "    coverage does not remove its private build directory before its last step"; exit 1; }
printf '%s\n' "$cov" | sed -n "${rm_at},\$p" | sed -n '2p' | grep -q "if: always() && runner.environment == 'self-hosted'" \
  || { echo "    the removal of the private build directory does not run always"; exit 1; }
printf '%s\n' "$cov" | grep -q 'build-shared' && { echo "    coverage names build-shared"; exit 1; }
echo "    coverage builds in a private directory on the fleet and always removes it"
