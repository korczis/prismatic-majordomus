# majordomus-covers: none
# claims: evidence-a-crate-binary-that-ran-nothing-is-not-a-pass, evidence-a-recording-is-a-typed-run
# A crate test binary states only what ran. `cargo test` prints `ok` on the result line of a
# binary that ran nothing at all, and a recorder that reads that word as a pass lets a claim
# read proven on the strength of a run that measured nothing. This case drives the recorder
# through the Rust executable over a coloured `cargo test` output, in a fixture repository of
# its own, and asserts what each binary is recorded as and the verdict each outcome reaches.
#
# What it proves, in order:
#   1  each binary is recorded as its counts say: one that ran no test, or only a filtered
#      subset, is a skip; one with a failed test is a failure; one that printed no result
#      line is an error; one that ran its whole set and failed none is a pass. Colour codes
#      change none of that. The crate's own unit tests and its doctests are not recorded, and
#      the text output lists each of them as dropped
#   2  the JSON output lists the same dropped entries, in the order the output held them,
#      each from the crate report
#   3  the outcomes reach the verdict: the pass is proven, the skip is not_run because the
#      test declined, the failure is failing, and the error is failing because the harness
#      could not run it
#   4  an output that names no binary at all, as a build that failed prints, records nothing
#      and leaves the ledger as it was, and its run record names the crate as absent
#   5  (evidence-a-recording-is-a-typed-run) the crate run's stamp carries the pinned toolchain,
#      and its run record counts the outcomes and lists the same dropped entries as the
#      recording, one list
#
# Every recording carries the measurement its run's stamp took of the fixture, naming the
# report, so the tree is the run's and the pass can read proven.
#
# The fixture is written into the current directory, so outside the harness's own fixture it
# would overwrite the claims matrix and the toolchain pin of whatever checkout the case was
# started from: it runs only where test/run.sh put it.
[ -n "${T:-}" ] && [ -n "${ROOT:-}" ] && [ "$(pwd -P)" = "$(cd "$T" && pwd -P)" ] \
  || { echo "    run this case through test/run.sh: it writes its fixture where it starts"; exit 1; }
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# The reports and the JSON answers live outside the repository under test: a file written
# inside it would be an untracked file, which is a dirty tree, and the verdicts below are
# read on a clean one.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj506.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}
outcome_of() {   # outcome_of <test id> — the one ledger row's outcome, or how many rows there are
  jq -r --arg t "$1" '[.executions[] | select(.test == $t)]
    | if length == 1 then .[0].outcome else "\(length) rows" end' "$LEDGER"
}
claim() {        # claim <id> — a jq path to that claim in `evidence show`
  printf '.claims[] | select(.id == "%s")' "$1"
}
LEDGER=".ai/repo/evidence/ledger.json"
BINARIES="alpha beta gamma delta eps zeta"

# ---------------------------------------------------------------- the fixture
# A repository with a layer, a claims matrix of its own and integration test sources of its
# own, one claim per binary. Only alpha is guaranteed; the rest are advisory, so that the
# verdicts below are about each claim and not about findings.
"$MJ" init >/dev/null
git check-ignore -q .ai/local/evidence/ledger.json \
  || { echo "    init does not ignore .ai/local/, which the local ledger relies on"; exit 1; }
mkdir -p docs lib apps/majordomus-cli/tests
{
  printf 'version: 1\nclaims:\n'
  for b in $BINARIES; do
    status=advisory; [ "$b" = alpha ] && status=guaranteed
    printf '  - id: crate-%s\n    claim: The %s binary holds\n' "$b" "$b"
    printf '    source: docs/CRATE.md\n    implementation: lib/crate.sh\n'
    printf '    test: apps/majordomus-cli/tests/%s.rs\n    status: %s\n' "$b" "$status"
  done
} > docs/CLAIMS.yaml
printf '# Crate\n' > docs/CRATE.md
printf '#!/usr/bin/env bash\n# crate\n' > lib/crate.sh
for b in $BINARIES; do printf '// the %s binary\n' "$b" > "apps/majordomus-cli/tests/$b.rs"; done
printf '[toolchain]\nchannel = "1.2.3"\n' > rust-toolchain.toml
git add -A >/dev/null && git commit -qm fixture
[ -z "$(git status --porcelain)" ] || { echo "    the fixture did not start on a clean tree"; git status --porcelain; exit 1; }

# What `cargo test` prints when it thinks a person is watching: bold green `Running`, a green
# `ok`, a red `FAILED`. The unit tests of the crate come first and the doctests last, as they
# do in every run; eps started and never printed a result line. alpha's `ok` ends the way the
# test harness itself ends it on an xterm, `ESC ( B ESC [ m`: the reset selects the character
# set as well, and an escape read as a two-byte pair leaves its `B` behind, `okB`.
{
  printf '\033[1m\033[92m     Running\033[0m unittests src/lib.rs (target/debug/deps/majordomus_cli-1)\n'
  printf 'test result: \033[32mok\033[0m. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s\n'
  printf '\033[1m\033[92m     Running\033[0m tests/alpha.rs (target/debug/deps/alpha-1)\n'
  printf 'test result: \033[32mok\033(B\033[m. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s\n'
  printf '\033[1m\033[92m     Running\033[0m tests/beta.rs (target/debug/deps/beta-1)\n'
  printf 'test result: \033[32mok\033[0m. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
  printf '     Running tests/gamma.rs (target/debug/deps/gamma-1)\n'
  printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n'
  printf '     Running tests/delta.rs (target/debug/deps/delta-1)\n'
  printf 'test result: \033[31mFAILED\033[0m. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n'
  printf '     Running tests/eps.rs (target/debug/deps/eps-1)\n'
  printf '     Running tests/zeta.rs (target/debug/deps/zeta-1)\n'
  printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
  printf '\033[1m\033[92m   Doc-tests\033[0m majordomus_cli\n'
  printf 'test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s\n'
} > "$W/cargo.txt"
# a plain file here would prove nothing about stripping colour codes, and one without the
# harness's own reset would prove nothing about the escapes that are not control sequences
grep -q "$(printf '\033')" "$W/cargo.txt" \
  || { echo "    the output the case wrote carries no escape sequence"; exit 1; }
grep -qF "$(printf 'ok\033(B')" "$W/cargo.txt" \
  || { echo "    the output the case wrote carries no character-set reset after an ok"; exit 1; }

# ---------------------------------------------------------------- 1. what each binary ran
# Proves `evidence-a-crate-binary-that-ran-nothing-is-not-a-pass`: the skips first, because
# reading `ok` as a pass is the defect this claim exists to close.
run_quiet "$W/stamp.err" "$MJB" evidence --repo "$T" stamp --producer crate --report "$W/cargo.txt" --out "$W/pc.json" > /dev/null
run_quiet "$W/rec.err" "$MJB" evidence --repo "$T" record --crate-output "$W/cargo.txt" \
  --provenance "crate=$W/pc.json" --run-record "$W/run.json" > "$W/rec.txt"
for pair in beta:skip gamma:skip delta:fail eps:error alpha:pass zeta:pass; do
  b="${pair%%:*}"; want="${pair#*:}"
  got="$(outcome_of "crate:$b")"
  [ "$got" = "$want" ] || {
    echo "    crate:$b was recorded as '$got', not '$want'"; jq -c '.executions' "$LEDGER" | head -c 2000; echo
    exit 1
  }
done
jq -e '[.executions[] | select(.test == "crate:alpha")][0].seconds == 1' "$LEDGER" >/dev/null \
  || { echo "    alpha's 1.40s was not recorded as its whole second"; exit 1; }
# nothing but the binaries under tests/: no row for the crate's unit tests or its doctests
jq -e '[.executions[].test] | sort == ["crate:alpha","crate:beta","crate:delta","crate:eps","crate:gamma","crate:zeta"]' \
  "$LEDGER" >/dev/null \
  || { echo "    the ledger holds a row for something other than the six binaries"; jq -c '[.executions[].test]' "$LEDGER"; exit 1; }
expect_grep '^recorded +6 execution\(s\), 2 passing' "$W/rec.txt"
expect_grep '^dropped +unittests src/lib\.rs \(the crate.s own unit tests' "$W/rec.txt"
expect_grep '^dropped +doc-tests majordomus_cli \(doctests' "$W/rec.txt"

# ---------------------------------------------------------------- 2. dropped, as a document
ev rec record --crate-output "$W/cargo.txt" --provenance "crate=$W/pc.json"
jqe rec '[.dropped[].what] == ["unittests src/lib.rs", "doc-tests majordomus_cli"]' \
  "the JSON output does not list the unit tests and the doctests, in order, as dropped"
jqe rec '[.dropped[].producer] | length == 2 and all(. == "crate")' \
  "a dropped entry does not name the crate report it came from"
jqe rec '[.dropped[].reason] | all(test("no claim can name"))' \
  "a dropped entry does not say why it is not recorded"
jqe rec '.recorded == 6 and .passed == 2' "the JSON output does not count the six binaries and the two passes"

# ---------------------------------------------------------------- 3. the verdict
# The outcomes are what the freshness rows read: a skip declined, an error could not run.
ev verdict show
jqe verdict "$(claim crate-alpha) | .state == \"proven\"" "the binary that ran its whole set is not proven"
jqe verdict "$(claim crate-zeta) | .state == \"proven\"" "the binary after the one with no result is not proven"
jqe verdict "$(claim crate-beta) | .state == \"not_run\" and (.detail | test(\"declined\"))" \
  "the binary that ran no test is not not_run with a detail saying it declined"
jqe verdict "$(claim crate-gamma) | .state == \"not_run\" and (.detail | test(\"declined\"))" \
  "the binary that ran a filtered subset is not not_run with a detail saying it declined"
jqe verdict "$(claim crate-delta) | .state == \"failing\"" "the binary with a failed test is not failing"
jqe verdict "$(claim crate-eps) | .state == \"failing\" and (.detail | test(\"could not run\"))" \
  "the binary that printed no result is not failing with a detail saying it could not run"

# ---------------------------------------------------------------- 4. a build that failed
# No binary ran, so nothing is recorded and the ledger is exactly what it was.
before="$(sha256_of_file "$LEDGER")"
printf 'error: could not compile `majordomus-cli` (lib) due to 1 previous error\n' > "$W/none.txt"
run_quiet "$W/stamp-none.err" "$MJB" evidence --repo "$T" stamp --producer crate --report "$W/none.txt" --out "$W/pn.json" > /dev/null
expect_exit 0 "$MJB" evidence --repo "$T" record --crate-output "$W/none.txt" --provenance "crate=$W/pn.json"
expect_grep '^recorded +0 execution\(s\), 0 passing'
expect_grep '^absent +crate \(the output named no integration test binary\)'
expect_no_grep '^dropped '
ev none record --crate-output "$W/none.txt" --provenance "crate=$W/pn.json"
jqe none '[.run_record.absent[] | select(.producer == "crate") | .reason] == ["the output named no integration test binary"]' \
  "the run record of an output that named no binary does not name the crate as absent"
[ "$(sha256_of_file "$LEDGER")" = "$before" ] \
  || { echo "    a crate output that named no binary changed the ledger"; exit 1; }

# ---------------------------------------------------------------- 5. the run, typed
# Proves `evidence-a-recording-is-a-typed-run`: the crate run's measurement carries the
# toolchain the fixture pins, and the run record is the recording's own account of it.
jq -e '.toolchain == {"name": "rustc", "version": "1.2.3", "source": "pinned"}' "$W/pc.json" >/dev/null \
  || { echo "    the crate stamp does not carry the pinned toolchain"; jq -c .toolchain "$W/pc.json"; exit 1; }
jq -e '.totals.outcomes == {"error": 1, "fail": 1, "pass": 2, "skip": 2} and .totals.runners == {"crate": 6}' \
  "$W/run.json" >/dev/null \
  || { echo "    the run record does not count the six binaries' outcomes"; jq -c .totals "$W/run.json"; exit 1; }
jq -e --slurpfile run "$W/run.json" '.dropped == $run[0].dropped and .run_record.dropped == .dropped' \
  "$W/rec.json" >/dev/null \
  || { echo "    the run record's dropped list is not the recording's"; jq -c .dropped "$W/run.json"; exit 1; }
