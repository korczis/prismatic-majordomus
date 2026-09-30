# majordomus-covers: none
# The check that proves every public surface is covered is itself run by something.
#
# `scripts/ci/surface-coverage` existed from 2026-09-10 and its own first line calls it "one
# mandatory gate proves every public surface is covered". Until this case nothing ran it: no
# gate, script, workflow or hook named it, and the automation inventory recorded it as dead
# for want of a caller. It passed every time anybody ran it by hand and protected nothing.
#
# Two claims, and they fail for different reasons:
#
#   1. the planner schedules it, on a change that plans nothing else: otherwise the paragraph
#      above is true again tomorrow;
#   2. it can still refuse: a wired check that cannot fail is a green tick with a schedule.
#
# §2 is asserted by mutation in a fixture rather than in the checkout, because its subject is
# `.ai/repo/ci/gates.yaml` and other sessions work in this repository at the same time.
. "$ROOT/test/lib.sh"

GATES="$ROOT/.ai/repo/ci/gates.yaml"
SC="$ROOT/scripts/ci/surface-coverage"
PLAN="$ROOT/scripts/ci-plan"
[ -f "$SC" ]    || { echo "    scripts/ci/surface-coverage does not exist"; exit 1; }
[ -f "$GATES" ] || { echo "    no gate model at .ai/repo/ci/gates.yaml"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is required: scripts/ci-plan cannot plan without it"; exit 1; }

# --- 1. the planner schedules it
# Asked of the planner rather than grepped from the model: the name appears in this case, in
# the script and in prose, and a check that passes because its own name is mentioned somewhere
# is the defect it was written to catch. README.md alone is a change no surface gate classes.
rc=0; printf 'README.md\n' | "$PLAN" --files - > plan.json 2> plan.err || rc=$?
[ "$rc" = 0 ] || { echo "    scripts/ci-plan refused (exit $rc):"; head -3 plan.err; exit 1; }
sel="$(jq -r '[.gates[] | select(.runs == "scripts/ci/surface-coverage" and .selected == true) | .id] | join(",")' plan.json)"
[ -n "$sel" ] || {
  echo "    no gate the plan selects for a README.md change runs scripts/ci/surface-coverage;"
  echo "    the check exists and nothing schedules it"; exit 1; }
echo "    the plan for a change that touches no surface schedules surface-coverage ($sel)"

# --- 2. and it still refuses when a surface loses the thing that covers it
# The fixture is the smallest tree the script reads: itself, the gate model, and the case files
# and Rust tests it names. The script derives its ROOT from its own location, so it runs from
# inside the fixture; pointing a variable at the fixture would leave it measuring the checkout,
# which is how a mutation silently proves nothing.
F="$T/fixture"
mkdir -p "$F/scripts/ci" "$F/.ai/repo/ci" "$F/test/cases" "$F/apps/majordomus-cli/tests"
cp "$SC" "$F/scripts/ci/surface-coverage"; chmod +x "$F/scripts/ci/surface-coverage"
cp "$GATES" "$F/.ai/repo/ci/gates.yaml"
for c in 11_site_derivation 12_site_build 104_published_site 99_plan_capabilities 98_traceability; do
  [ -f "$ROOT/test/cases/$c.sh" ] || { echo "    the case $c the script names is gone from the checkout"; exit 1; }
  cp "$ROOT/test/cases/$c.sh" "$F/test/cases/$c.sh"
done
for r in metadata_contract cli hot_path generated_documents cockpit product; do
  [ -f "$ROOT/apps/majordomus-cli/tests/$r.rs" ] || { echo "    the Rust test $r.rs the script names is gone"; exit 1; }
  cp "$ROOT/apps/majordomus-cli/tests/$r.rs" "$F/apps/majordomus-cli/tests/$r.rs"
done

run_fixture() { rc=0; ( cd "$F" && scripts/ci/surface-coverage ) > "$T/sc.out" 2> "$T/sc.err" || rc=$?; }

run_fixture
[ "$rc" = 0 ] || {
  echo "    the fixture does not reproduce a covered repository (exit $rc); the case cannot"
  echo "    distinguish a real refusal from a broken fixture:"
  grep -E '^FAIL' "$T/sc.out" | head -4; exit 1; }
echo "    a repository whose surfaces are covered passes"

# 2a. a surface loses its mandatory gate: the published-site check stops being run
awk '/^[[:space:]]*runs:.*pages-check/ { sub(/runs:.*/, "runs: true"); n++ } { print }
     END { if (!n) exit 3 }' "$GATES" > "$F/.ai/repo/ci/gates.yaml" || {
  echo "    the gate model names no pages-check to remove"; exit 1; }
run_fixture
[ "$rc" != 0 ] || {
  echo "    a gate model that no longer runs the published-site check still passed;"
  echo "    surface-coverage cannot notice a surface losing its gate"; exit 1; }
grep -q '^FAIL *pages' "$T/sc.out" || {
  echo "    it refused, but not about pages:"; grep '^FAIL' "$T/sc.out" | head -3; exit 1; }
echo "    a surface whose mandatory gate is removed is refused, and named"
cp "$GATES" "$F/.ai/repo/ci/gates.yaml"

# 2b. a surface loses the test that covers it
rm -f "$F/test/cases/104_published_site.sh"
run_fixture
[ "$rc" != 0 ] || {
  echo "    a repository missing the published-site case still passed"; exit 1; }
grep -q '^FAIL *pages' "$T/sc.out" || {
  echo "    it refused, but not about the missing case:"; grep '^FAIL' "$T/sc.out" | head -3; exit 1; }
echo "    a surface whose behavioural case is deleted is refused, and named"

echo "    the surface-coverage gate is wired, and it can still fail"
