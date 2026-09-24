# majordomus-covers: none
# claims: rust-evidence-gates, rust-hot-path-benchmarks, rust-coverage-floor
# The Rust executable carries its evidence with it (rule project.rust-cli-evidence). The
# rule promises gates, not intentions: an undocumented public item does not build, the
# suites include the doc examples, coverage has a committed floor, the hot paths carry
# benchmarks, and one script runs what CI runs. A gate that lives only in a workflow file
# is one edit away from disappearing without a trace, so this case reads the wiring
# itself: the crate's attributes, scripts/rust-check, the threshold file, the CI jobs and
# the justfile must agree on every gate, in the same order. Then, with a toolchain
# present, it runs the two gates that are cheap enough here and that no other case runs,
# the doc examples and the benchmark build, and asks the built executable whether every
# executable capability declares its benchmark policy. The full suite, the lints and the
# coverage measurement are CI's rust and coverage jobs and `just rust-check`; this case
# proves they are wired, not that they pass on this machine.
#
# The structural half never skips. The behavioural half skips itself when there is
# neither cargo nor a MAJORDOMUS_BIN to drive, as the other Rust cases do.
. "$ROOT/test/lib.sh"
CRATE="$ROOT/apps/majordomus-cli"
RC="$ROOT/scripts/rust-check"
TH="$ROOT/scripts/rust-coverage-threshold"
WF="$ROOT/.github/workflows/validate.yml"
JF="$(just_declaration)"   # the root file plus the modules it imports
RULE="$ROOT/.ai/repo/rules/project/rust-cli-evidence.v1.md"
CLAIMS="$ROOT/docs/CLAIMS.yaml"

# --- an undocumented public item is a lint, and lints are errors: both crate roots say so
expect_grep '^#!\[warn\(missing_docs\)\]' "$CRATE/src/lib.rs"
expect_grep '^#!\[warn\(missing_docs\)\]' "$CRATE/src/main.rs"

# --- one script, every gate, in CI's order; each label is followed by the real command
[ -x "$RC" ] || { echo "    scripts/rust-check is missing or not executable"; exit 1; }
want='cargo fmt --check
cargo clippy -D warnings
cargo test
cargo doc -D warnings
cargo bench --no-run
cargo build
capabilities validate
generate --check
bench coverage --check
web validate
quality report
scripts/rust-coverage (crate $threshold%, session domain $domain%)
artifact $ARTIFACT'
got="$(grep -oE '^[[:space:]]*step "[^"]+"' "$RC" | sed -E 's/^[[:space:]]*step "//; s/"$//' | grep -v '^rust-check:')"
[ "$got" = "$want" ] || { printf '    scripts/rust-check does not run the gates in CI'"'"'s order\n    want:\n%s\n    got:\n%s\n' "$want" "$got"; exit 1; }
expect_grep 'cargo fmt --check$' "$RC"
expect_grep 'cargo clippy --all-targets --all-features -- -D warnings' "$RC"
expect_grep 'cargo test --no-fail-fast' "$RC"
# the baseline comparison belongs to the platform that has a baseline: on the Linux gate it
# compared nothing, could not fail, and ran the job out of time before the gates after it
expect_no_grep 'step "bench --profile' "$RC"
expect_grep "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps" "$RC"
expect_grep 'cargo bench --no-run' "$RC"
expect_grep 'cargo run --quiet -- capabilities validate' "$RC"
expect_grep 'cargo run --quiet -- generate --check' "$RC"
expect_grep 'cargo run --quiet -- bench coverage --check' "$RC"
expect_grep 'cargo run --quiet -- quality report' "$RC"
expect_grep 'scripts/rust-coverage-threshold' "$RC"
expect_grep 'scripts/rust-coverage' "$RC"
expect_grep 'session-coverage-threshold' "$RC"

# --- the coverage floor is one integer, committed, and not under the bound it ratchets from.
#     The bound is written once, as FLOOR_BOUND in scripts/ci/rust-command-check, the gate of
#     rule project.rust-command-tested-in-file; this case reads it there rather than restating
#     it, so lowering the bar is an edit to that gate, visible in review. The crate measured
#     83.10-83.34% lines on every recorded coverage job and no run ever met the 90 it replaced.
th="$(cat "$TH")"
case "$th" in ''|*[!0-9]*) echo "    scripts/rust-coverage-threshold is not one integer: '$th'"; exit 1 ;; esac
bound="$(sed -n 's/^FLOOR_BOUND=\([0-9][0-9]*\)$/\1/p' "$ROOT/scripts/ci/rust-command-check")"
case "$bound" in ''|*[!0-9]*) echo "    scripts/ci/rust-command-check declares no integer FLOOR_BOUND: '$bound'"; exit 1 ;; esac
{ [ "$th" -ge "$bound" ] && [ "$th" -le 100 ]; } \
  || { echo "    the coverage floor is $th; it may rise from $bound and never fall under it"; exit 1; }

# --- CI runs the same script: the rust job calls scripts/rust-check itself, --ci for every
#     gate when the crate can be affected (the benchmark check against the platform's
#     baseline included) or --integration for the registry checks alone, and asks it for the
#     executable as an artifact; the coverage job reads the same threshold file the script
#     reads; the bench job runs the benchmark check where the committed baseline is
rust_job="$(awk '/^  rust:/{f=1} /^  coverage:/{f=0} f' "$WF")"
[ -n "$rust_job" ] || { echo "    validate.yml has no rust job"; exit 1; }
printf '%s\n' "$rust_job" | grep -qE '^ +run: (MJ_CI_TIMINGS=[^ ]+ )?scripts/rust-check "\$MODE" --artifact ' \
  || { echo "    the rust job does not run scripts/rust-check with the plan's mode and an artifact"; exit 1; }
printf '%s\n' "$rust_job" | grep -q "rust_check == 'true' && '--ci' || '--integration'" \
  || { echo "    the rust job does not choose --ci or --integration from the plan"; exit 1; }
printf '%s\n' "$rust_job" | grep -q 'components: rustfmt, clippy' || { echo "    the rust job installs no rustfmt and clippy"; exit 1; }
printf '%s\n' "$rust_job" | grep -q 'upload-artifact' || { echo "    the rust job publishes no executable artifact"; exit 1; }
grep -qE "^  (step \"cargo doc -D warnings\";|.*)RUSTDOCFLAGS='-D warnings' cargo doc" "$RC" || { echo "    scripts/rust-check builds the docs without -D warnings"; exit 1; }
cov_job="$(awk '/^  coverage:/{f=1} /^  bench:/{f=0} f' "$WF")"
[ -n "$cov_job" ] || { echo "    validate.yml has no coverage job"; exit 1; }
printf '%s\n' "$cov_job" | grep -qF 'scripts/rust-coverage' \
  || { echo "    the coverage job does not run scripts/rust-coverage"; exit 1; }
# The measurement subtracts test code from both sides of the fraction, which is the whole
# reason it is a script of this repository's rather than a cargo-llvm-cov invocation: a
# `#[cfg(test)] mod tests` lives in the same file as the code it tests, and no filename regex
# can reach it. A job that went back to calling cargo-llvm-cov directly would be measuring
# the crate plus its tests again, and the threshold would move whenever a test was written.
printf '%s\n' "$cov_job" | grep -qF 'cargo llvm-cov --all-targets --summary-only' \
  && { echo "    the coverage job measures the crate together with its own test code again"; exit 1; }
# and the second threshold is a file of its own with a domain beside it, so that the domain
# cannot be narrowed to raise the number without the narrowing being a visible edit
SD="$ROOT/scripts/session-coverage-threshold"
expect_file "$SD"
expect_file "$ROOT/scripts/session-coverage-domain"
sd="$(tr -d ' \n' < "$SD")"
case "$sd" in ''|*[!0-9]*) echo "    scripts/session-coverage-threshold is not one integer: '$sd'"; exit 1 ;; esac
# The same ratchet for the session/continuity domain. It was held to 100 and measured 95.73%
# lines, 100% functions and 96.68% regions with test code excluded; the coverage job never said
# so because it stopped earlier. The floor is now the measured value, and may only rise.
{ [ "$sd" -ge 95 ] && [ "$sd" -le 100 ]; } || { echo "    the session domain floor is $sd; it may rise from 95 and never fall under it"; exit 1; }
grep -q 'continuity.rs' "$ROOT/scripts/session-coverage-domain" \
  || { echo "    the session/continuity domain does not name continuity.rs"; exit 1; }
while IFS= read -r d; do
  case "$d" in ''|\#*) continue ;; esac
  [ -f "$ROOT/$d" ] || { echo "    the session/continuity domain names $d, which is not a file; a threshold over a file nothing compiles is a threshold over nothing"; exit 1; }
done < "$ROOT/scripts/session-coverage-domain"
bench_job="$(awk '/^  bench:/{f=1} /^  site:/{f=0} f' "$WF")"
printf '%s\n' "$bench_job" | grep -qF 'cargo run --quiet -- bench --profile ci --check --no-write' \
  || { echo "    the bench job does not run the benchmark check against the baseline"; exit 1; }
# the jobs run on pull requests and on master, not on a schedule or by hand only
awk '/^on:/{f=1} /^jobs:/{f=0} f' "$WF" | grep -qE '^  (push|pull_request):' || { echo "    validate.yml does not run on push"; exit 1; }

# --- a person is routed to the same gates
#
# The recipes are declared across the justfile and the files it imports, so which file a
# recipe is written in is not a fact this case may assert: `just --dump` is the whole set
# after the imports are spliced, and it carries the dependencies and the body. Grepping one
# file would be a justfile parser, and a second one at that.
[ -f "$JF" ] || { echo "    the justfile is missing"; exit 1; }
# `|| true`, because the case runs under `bash -eu`: without it a `just` that is not
# installed exits 127, the assignment inherits it, and the case dies before the guard on the
# next line can say anything at all. That is how this case spent its CI life failing in one
# second with no output — the runner had no `just` — while the sentence explaining what was
# wrong sat one line below, unreachable.
dump="$(cd "$ROOT" && just --dump --dump-format json 2>/dev/null || true)"
[ -n "$dump" ] || { echo "    just --dump produced nothing: just is not installed, or the justfile does not parse"; exit 1; }

recipe_exists() {
  printf '%s' "$dump" | jq -e --arg r "$1" '.recipes | has($r)' >/dev/null 2>&1 \
    || { echo "    just $1 is not a recipe"; exit 1; }
}
recipe_body() {
  printf '%s' "$dump" | jq -r --arg r "$1" '[.recipes[$r].body // [] | .. | strings] | join(" ")'
}

printf '%s' "$dump" | jq -e '[.recipes.test.dependencies[]?.recipe] | index("rust-check")' >/dev/null 2>&1 \
  || { echo "    just test does not depend on rust-check"; exit 1; }
recipe_exists rust-check
recipe_body rust-check | grep -q 'scripts/rust-check' \
  || { echo "    just rust-check does not run scripts/rust-check"; exit 1; }
recipe_exists coverage
recipe_body coverage | grep -q 'scripts/rust-coverage' \
  || { echo "    just coverage does not run scripts/rust-coverage"; exit 1; }
recipe_exists test-rust
recipe_exists bench

# --- the floors hold, measured with test code out of the denominator. scripts/rust-coverage
#     is run, unmodified and copied into a fixture, over synthetic `cargo llvm-cov --json`
#     exports (--from), so what is proved is the script's arithmetic and verdict, not this
#     machine's coverage. The fixture carries this repository's two floors. Every product line
#     is its own region entry followed by a count-less segment, so a line is covered exactly
#     when its own count is non-zero; a src file closes with an inline #[cfg(test)] module and
#     a file under tests/ rides along, both fully executed, as test code always is.
if command -v python3 >/dev/null 2>&1; then
  CV="$(mktemp -d "${TMPDIR:-/tmp}/mj77cov.XXXXXX")"; CV="$(cd "$CV" && pwd)"
  mkdir -p "$CV/scripts" "$CV/apps/majordomus-cli/src/capability/builtin" "$CV/apps/majordomus-cli/tests"
  cp "$ROOT/scripts/rust-coverage" "$CV/scripts/"
  cp "$TH" "$SD" "$CV/scripts/"
  printf '# the domain\napps/majordomus-cli/src/capability/builtin/continuity.rs\n' > "$CV/scripts/session-coverage-domain"
  LIB=apps/majordomus-cli/src/lib.rs DOM=apps/majordomus-cli/src/capability/builtin/continuity.rs IT=apps/majordomus-cli/tests/it.rs
  # 100 product lines, then a 50-line inline test module
  { for i in $(seq 1 100); do printf 'pub fn f%s() {}\n' "$i"; done
    printf '#[cfg(test)]\nmod tests {\n'
    for i in $(seq 1 47); do printf '    fn t%s() {}\n' "$i"; done
    printf '}\n'; } > "$CV/$LIB"
  for i in $(seq 1 100); do printf 'pub fn g%s() {}\n' "$i"; done > "$CV/$DOM"
  for i in $(seq 1 100); do printf 'fn it%s() {}\n' "$i"; done > "$CV/$IT"
  # export LIB_COVERED DOM_COVERED [DOMAIN_FILE_PRESENT] > file
  cov_export() {
    python3 - "$CV" "$LIB" "$DOM" "$IT" "$1" "$2" "${3:-1}" <<'PY'
import json, sys
root, lib, dom, it, lib_cov, dom_cov, dom_present = sys.argv[1:]
def segs(n, covered, first=1):
    out = []
    for i in range(n):
        line = first + i
        out.append([line, 1, 1 if i < covered else 0, True, True, False])
        out.append([line, 5, 0, False, False, False])
    return out
files = [
    {"filename": root + "/" + lib, "segments": segs(100, int(lib_cov)) + segs(50, 50, first=101)},
    {"filename": root + "/" + it, "segments": segs(100, 100)},
]
if dom_present == "1":
    files.append({"filename": root + "/" + dom, "segments": segs(100, int(dom_cov))})
json.dump({"data": [{"files": files, "functions": []}]}, sys.stdout)
PY
  }
  cov_run() { bash "$CV/scripts/rust-coverage" --from "$1"; }
  # the crate is both src files, 200 product lines; the domain is continuity.rs alone
  lib_at=$((2 * th - sd)); lib_below=$((2 * th - 1 - 100)); dbelow=$((sd - 1))
  { [ "$lib_at" -ge 0 ] && [ "$lib_at" -le 100 ] && [ "$lib_below" -ge 0 ]; } \
    || { echo "    the synthetic export cannot place the crate at the floors $th/$sd"; exit 1; }
  # at both floors: green
  cov_export "$lib_at" "$sd" > "$CV/at.json"
  expect_exit 0 cov_run "$CV/at.json"
  # half a point under the crate floor: refused with the measured figure and the floor's file,
  # although the same run with its test code counted stands above the floor
  cov_export "$lib_below" 100 > "$CV/crate-below.json"
  expect_exit 10 cov_run "$CV/crate-below.json"
  excl="$(python3 -c "print('%.2f' % (100.0 * ($lib_below + 100) / 200))")"
  incl="$(python3 -c "print('%.2f' % (100.0 * ($lib_below + 100 + 150) / 350))")"
  expect_grep "the crate is at ${excl}% lines, below the threshold of ${th}\.00% \(scripts/rust-coverage-threshold\)"
  expect_grep "the crate, test code excluded +${excl}%"
  expect_grep "the crate, test code included \(the old gate's subject\) +${incl}%"
  python3 -c "import sys; sys.exit(0 if float('$incl') >= $th else 1)" \
    || { echo "    the synthetic export does not separate the two measurements ($incl% with tests)"; exit 1; }
  # one line under the domain floor, the crate above its own: refused by the domain's floor
  cov_export 100 "$dbelow" > "$CV/domain-below.json"
  expect_exit 10 cov_run "$CV/domain-below.json"
  expect_grep "the session/continuity domain is at ${dbelow}\.00% lines, below the threshold of ${sd}\.00% \(scripts/session-coverage-threshold\)"
  expect_no_grep 'the crate is at'
  # a domain naming a file the export does not carry is refused, not measured as 100% of nothing
  cov_export 100 100 0 > "$CV/domain-absent.json"
  expect_exit 10 cov_run "$CV/domain-absent.json"
  expect_grep 'NOT IN THE EXPORT'
  # --report measures and holds nothing
  expect_exit 0 bash "$CV/scripts/rust-coverage" --report --from "$CV/crate-below.json"
  rm -rf "$CV"
else
  echo "    skip: python3 not installed (the coverage arithmetic is scripts/rust-coverage's python)"
fi
# and the gate runs on every change that can reach the crate and on every master push: the rust
# and share classes of the gate model select it, and a push is planned in full, which selects it
awk '/^  - id: rust$/{f=1} f&&/^    gates:/{print; exit}' "$ROOT/.ai/repo/ci/gates.yaml" | grep -qE '[[, ]rust-coverage[],]' \
  || { echo "    the rust class of .ai/repo/ci/gates.yaml does not select rust-coverage"; exit 1; }
awk '/^  - id: rust$/{f=1} f&&/^    paths:/{print; exit}' "$ROOT/.ai/repo/ci/gates.yaml" | grep -qF 'apps/majordomus-cli/**' \
  || { echo "    the rust class of .ai/repo/ci/gates.yaml does not cover apps/majordomus-cli/**"; exit 1; }
awk '/^  - id: share$/{f=1} f&&/^    gates:/{print; exit}' "$ROOT/.ai/repo/ci/gates.yaml" | grep -qE '[[, ]rust-coverage[],]' \
  || { echo "    the share class of .ai/repo/ci/gates.yaml does not select rust-coverage"; exit 1; }
printf '%s\n' "$(awk '/^  plan:/{f=1} /^  [a-z-]+:$/&&!/^  plan:/{f=0} f' "$WF")" | grep -qF '*) scripts/ci-plan --full "$EVENT on $REF"' \
  || { echo "    a push to master is not planned in full"; exit 1; }
"$ROOT/scripts/ci-plan" --full "push on master" 2>/dev/null | jq -e '[.gates[] | select(.id == "rust-coverage" and .selected)] | length == 1' >/dev/null \
  || { echo "    a full plan does not select the rust-coverage gate"; exit 1; }
printf '%s\n' "$cov_job" | grep -qF "needs.plan.outputs.rust_coverage == 'true'" \
  || { echo "    the coverage job does not run when the plan selects rust-coverage"; exit 1; }

# --- the hot paths carry benchmarks: every file under benches/ is a declared criterion
#     target without the default harness, and every path the rule names is measured
expect_grep '^criterion = ' "$CRATE/Cargo.toml"
for f in "$CRATE"/benches/*.rs; do
  name="$(basename "$f" .rs)"
  grep -A2 '^\[\[bench\]\]' "$CRATE/Cargo.toml" | grep -A1 "^name = \"$name\"" | grep -q '^harness = false' \
    || { echo "    benches/$name.rs has no [[bench]] entry with harness = false"; exit 1; }
done
for path in 'frontmatter split' 'yaml subset' 'glob match' 'index build' 'registry build' 'openapi document' 'resources/list'; do
  cat "$CRATE"/benches/*.rs | grep -qF -- "$path" || { echo "    no benchmark measures '$path'"; exit 1; }
done

# --- the rule is in the layer, blocking and active, names this case as its proof, and at
#     least one claim is proved by it, so neither the rule nor the case can be orphaned
expect_grep '^id: project\.rust-cli-evidence$' "$RULE"
expect_grep '^status: active$' "$RULE"
expect_grep '^class: blocking$' "$RULE"
expect_grep '77_rust_evidence' "$RULE"
expect_grep '^    test: test/cases/77_rust_evidence\.sh$' "$CLAIMS"

# --- with a toolchain: the doc examples pass, the benchmarks build, and the executable
#     says every executable capability has a benchmark policy
MANIFEST="$CRATE/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj77.XXXXXX")"; trap 'rm -rf "$S"' EXIT
if command -v cargo >/dev/null 2>&1; then
  RUSTFLAGS='' cargo test -q --manifest-path "$MANIFEST" --doc >"$S/doc.log" 2>&1 \
    || { tail -40 "$S/doc.log"; echo "    the doc examples do not pass"; exit 1; }
  grep -qE 'test result: ok\. [1-9][0-9]* passed' "$S/doc.log" || { cat "$S/doc.log"; echo "    no doc example ran"; exit 1; }
  RUSTFLAGS='' cargo bench -q --no-run --manifest-path "$MANIFEST" >"$S/bench.log" 2>&1 \
    || { tail -40 "$S/bench.log"; echo "    the benchmarks do not build"; exit 1; }
else
  echo "    skip: cargo not installed (the doc examples and the benchmark build are cargo's)"
fi
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
expect_exit 0 "$RB" capabilities validate
"$RB" capabilities list --format json >"$S/caps.json" 2>/dev/null || { echo "    capabilities list failed"; exit 1; }
jq -e '[.capabilities[] | select(.kind != "resource")] | length > 0 and all(.benchmark.policy == "required" or (.benchmark.policy == "required_when" and ((.benchmark.precondition // "") | length) > 0) or (.benchmark.policy == "waived" and ((.benchmark.reason // "") | length) > 0))' "$S/caps.json" >/dev/null \
  || { echo "    an executable capability has no benchmark policy, a waiver without a reason, or a condition without a precondition"; jq -c '.capabilities[] | select(.kind != "resource") | {id, benchmark}' "$S/caps.json"; exit 1; }
