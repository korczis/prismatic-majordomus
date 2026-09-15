# majordomus-covers: none
# The check that proves every public surface is covered is itself run by something.
#
# `scripts/ci/surface-coverage` has existed since 2026-09-10 and its own first line calls it
# "one mandatory gate proves every public surface is covered". Until this case, nothing ran
# it: a repository-wide search found exactly one reference, in a historical session record.
# It passed, every time anybody ran it by hand, and it protected nothing — the class this
# repository keeps rediscovering, where a thing is declared and no producer enforces it.
#
# Two claims, and they fail for different reasons:
#
#   1. the gate model runs it — otherwise the paragraph above is true again tomorrow;
#   2. it can still refuse — a wired check that cannot fail is a green tick with a schedule.
#
# §2 is the half that costs something to write, and it is the half worth having. It is
# asserted by mutation in a fixture rather than in the checkout, because the subject is
# `.ai/repo/ci/gates.yaml` and eight sessions are working in this repository tonight.
. "$ROOT/test/lib.sh"

GATES="$ROOT/.ai/repo/ci/gates.yaml"
SC="$ROOT/scripts/ci/surface-coverage"
[ -f "$SC" ]    || { echo "    scripts/ci/surface-coverage does not exist"; exit 1; }
[ -f "$GATES" ] || { echo "    no gate model at .ai/repo/ci/gates.yaml"; exit 1; }

# --- 1. the gate model runs it
# Read as a gate entry rather than grepped for the word: the string appears in this case, in
# the script itself and in prose, and a check that passes because its own name is mentioned
# somewhere is the defect it was written to catch.
python3 - "$GATES" <<'PY' || exit 1
import sys, yaml
model = yaml.safe_load(open(sys.argv[1], encoding='utf-8'))
gates = model.get('gates') or []
mine = [g for g in gates if 'surface-coverage' in str(g.get('runs', ''))]
if not mine:
    print("    no gate in .ai/repo/ci/gates.yaml runs scripts/ci/surface-coverage;"
          " the check exists and nothing invokes it")
    sys.exit(1)
for g in mine:
    if not g.get('always') and not g.get('paths') and not g.get('classes'):
        print(f"    the gate {g.get('id')} runs surface-coverage but is neither unconditional"
              " nor selected by any path class, so no plan would ever schedule it")
        sys.exit(1)
print(f"    the gate model runs surface-coverage ({', '.join(str(g.get('id')) for g in mine)})")
PY

# --- 2. and it still refuses when a surface loses the thing that covers it
# The fixture is the smallest tree the script reads: itself, the gate model, and the case
# files and Rust tests it names. `ROOT` inside the script is derived from its own location,
# so it must be run from inside the fixture — pointing an environment variable at the fixture
# would leave it measuring this checkout, which is how a mutation silently proves nothing.
F="$T/fixture"
mkdir -p "$F/scripts/ci" "$F/.ai/repo/ci" "$F/test/cases" "$F/apps/majordomus-cli/tests"
cp "$SC" "$F/scripts/ci/surface-coverage"; chmod +x "$F/scripts/ci/surface-coverage"
cp "$GATES" "$F/.ai/repo/ci/gates.yaml"
for c in 11_site_derivation 12_site_build 104_published_site 99_plan_capabilities 98_traceability; do
  cp "$ROOT/test/cases/$c.sh" "$F/test/cases/$c.sh" 2>/dev/null || printf 'x\n' > "$F/test/cases/$c.sh"
done
for r in metadata_contract cli hot_path generated_documents cockpit product; do
  cp "$ROOT/apps/majordomus-cli/tests/$r.rs" "$F/apps/majordomus-cli/tests/$r.rs" 2>/dev/null \
    || printf 'x\n' > "$F/apps/majordomus-cli/tests/$r.rs"
done

run_fixture() { rc=0; ( cd "$F" && scripts/ci/surface-coverage ) > "$T/sc.out" 2> "$T/sc.err" || rc=$?; }

run_fixture
[ "$rc" = 0 ] || {
  echo "    the fixture does not reproduce a covered repository (exit $rc); the case cannot"
  echo "    distinguish a real refusal from a broken fixture:"
  grep -E '^FAIL' "$T/sc.out" | head -4; exit 1; }
echo "    a repository whose surfaces are covered passes"

# 2a. a surface loses its mandatory gate: the published-site check stops being required
python3 - "$F/.ai/repo/ci/gates.yaml" <<'PY' || exit 1
import sys, re
p = sys.argv[1]; s = open(p, encoding='utf-8').read()
out, n = re.subn(r'(?m)^(\s*runs:\s*).*pages-check.*$', r'\1true', s)
if n == 0:
    print("    the fixture's gate model names no pages-check to remove"); sys.exit(1)
open(p, 'w', encoding='utf-8').write(out)
PY
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
