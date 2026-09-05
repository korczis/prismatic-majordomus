# majordomus-covers: none
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
JF="$ROOT/justfile"
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
bench --profile ci --check
cargo llvm-cov --fail-under-lines $threshold
artifact $ARTIFACT'
got="$(grep -oE '^[[:space:]]*step "[^"]+"' "$RC" | sed -E 's/^[[:space:]]*step "//; s/"$//' | grep -v '^rust-check:')"
[ "$got" = "$want" ] || { printf '    scripts/rust-check does not run the gates in CI'"'"'s order\n    want:\n%s\n    got:\n%s\n' "$want" "$got"; exit 1; }
expect_grep 'cargo fmt --check$' "$RC"
expect_grep 'cargo clippy --all-targets --all-features -- -D warnings' "$RC"
expect_grep 'cargo test --no-fail-fast' "$RC"
expect_grep "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps" "$RC"
expect_grep 'cargo bench --no-run' "$RC"
expect_grep 'cargo run --quiet -- capabilities validate' "$RC"
expect_grep 'cargo run --quiet -- generate --check' "$RC"
expect_grep 'cargo run --quiet -- bench coverage --check' "$RC"
expect_grep 'cargo llvm-cov --all-targets --summary-only --fail-under-lines "[$]threshold"' "$RC"
expect_grep 'scripts/rust-coverage-threshold' "$RC"

# --- the coverage floor is one integer, committed, and not under the figure it was set at;
#     lowering it is an edit to this case, visible in review, not a quiet change to a number
th="$(cat "$TH")"
case "$th" in ''|*[!0-9]*) echo "    scripts/rust-coverage-threshold is not one integer: '$th'"; exit 1 ;; esac
{ [ "$th" -ge 90 ] && [ "$th" -le 100 ]; } || { echo "    the coverage floor is $th; the rule expects 90 to 100"; exit 1; }

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
printf '%s\n' "$cov_job" | grep -qF 'cargo llvm-cov --all-targets --summary-only --fail-under-lines "$(cat ../../scripts/rust-coverage-threshold)"' \
  || { echo "    the coverage job does not fail under scripts/rust-coverage-threshold"; exit 1; }
bench_job="$(awk '/^  bench:/{f=1} /^  site:/{f=0} f' "$WF")"
printf '%s\n' "$bench_job" | grep -qF 'cargo run --quiet -- bench --profile ci --check --no-write' \
  || { echo "    the bench job does not run the benchmark check against the baseline"; exit 1; }
# the jobs run on pull requests and on master, not on a schedule or by hand only
awk '/^on:/{f=1} /^jobs:/{f=0} f' "$WF" | grep -qE '^  (push|pull_request):' || { echo "    validate.yml does not run on push"; exit 1; }

# --- a person is routed to the same gates
expect_grep '^test:.* rust-check( |$)' "$JF"
expect_grep '^rust-check:' "$JF"
grep -A1 '^rust-check:' "$JF" | grep -q 'scripts/rust-check' || { echo "    just rust-check does not run scripts/rust-check"; exit 1; }
expect_grep '^coverage:' "$JF"
grep -A1 '^coverage:' "$JF" | grep -q 'rust-coverage-threshold' || { echo "    just coverage does not read scripts/rust-coverage-threshold"; exit 1; }
expect_grep '^test-rust \*args:' "$JF"
expect_grep '^bench \*name:' "$JF"

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
jq -e '[.capabilities[] | select(.kind != "resource")] | length > 0 and all(.benchmark.policy == "required" or (.benchmark.policy == "waived" and ((.benchmark.reason // "") | length) > 0))' "$S/caps.json" >/dev/null \
  || { echo "    an executable capability has no benchmark policy, or a waiver without a reason"; jq -c '.capabilities[] | select(.kind != "resource") | {id, benchmark}' "$S/caps.json"; exit 1; }
