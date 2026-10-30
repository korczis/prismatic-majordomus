# majordomus-covers: none
#
# The entry preflight reads the one verdict (I1965, ADR 0087 decisions 3 and 4): its tests check
# judges each recorded run through the freshness function `evidence show` judges with, instead
# of a filter of its own. Entered the way a shell does, over a fixture whose ledger is written
# by hand:
#
#   1. a pass recorded at HEAD on a clean tree, the test unchanged: verified
#   2. the same run, skipped: not run, so stale and never failed
#   3. the same run, failed: failed
#   4. a pass recorded on a commit HEAD does not contain, the tree identical: stale, where the
#      filter it replaces, which compared only the trees, called it verified
#
# The enforcement half (verified only when the rules corpus verdict is proven) and every row
# over hand-built observations are apps/majordomus-cli/tests/preflight.rs.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p test/cases && printf 'echo ok\n' > test/cases/01_ok.sh
git add . && git commit -qm base
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

tests_verdict() {
  "$RB" env preflight --repo "$T" --format json | python3 -c 'import json,sys
d=json.load(sys.stdin)
print({c["id"]:c["verdict"] for s in d["sections"] for c in s["checks"]}["verification.tests"])'
}
ledger() {   # <commit> <outcome>
  local digest
  digest="sha256:$(mj_sha256_hex test/cases/01_ok.sh)"
  mkdir -p .ai/repo/evidence
  printf '{"version":1,"executions":[{"test":"suite:01_ok","runner":"suite","source":"test/cases/01_ok.sh","outcome":"%s","seconds":1,"commit":"%s","working_tree":"clean","digest":"%s","at":"2026-10-21T00:00:00Z","origin":"local","command":"bash test/run.sh 01_ok"}]}\n' \
    "$2" "$1" "$digest" > .ai/repo/evidence/ledger.json
}
expect_tests() {   # <verdict> <why>
  local got; got="$(tests_verdict)" || { echo "    no preflight"; exit 1; }
  [ "$got" = "$1" ] || { echo "    verification.tests is '$got', expected '$1': $2"; exit 1; }
}

head="$(git rev-parse HEAD)"

# --- 1. measured here
ledger "$head" pass
expect_tests verified "a pass at HEAD on a clean tree, the test unchanged, is proven"
echo "    ok: a pass measured at this tree verifies the tests"

# --- 2. a skip is not run, never a failure
ledger "$head" skip
expect_tests stale "a skip is not run; it may not read as a failure"
echo "    ok: a skip leaves the tests unproven, not failed"

# --- 3. a failure is a failure
ledger "$head" fail
expect_tests failed "a recorded failure fails the check"
echo "    ok: a recorded failure fails the check"

# --- 4. another history, the same tree
side="$(git commit-tree "HEAD^{tree}" -p HEAD -m 'a side commit')"
[ "$(git rev-parse "$side^{tree}")" = "$(git rev-parse "HEAD^{tree}")" ] \
  || { echo "    the side commit's tree differs; the case proves nothing"; exit 1; }
ledger "$side" pass
expect_tests stale "a run on a commit HEAD does not contain measured another history"
echo "    ok: a pass recorded on a commit HEAD does not contain is stale, the trees identical"
