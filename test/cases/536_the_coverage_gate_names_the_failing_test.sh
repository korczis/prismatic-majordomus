# majordomus-covers: none
# A test that fails under coverage is named in the coverage job's log, with its panic.
#
# scripts/rust-coverage sent `cargo llvm-cov`'s stdout to /dev/null, and stdout is where the test
# harness writes `test ... FAILED`, the `---- <name> stdout ----` section and the panic. The
# coverage job of #632 went red twice on different binaries and its log said only `error: test
# failed, to rerun pass --test bench`, then `--lib`: an intermittent failure nobody could name.
#
# A `cargo` on PATH stands in for cargo-llvm-cov, so no instrumented build is needed:
#
#   1. a red suite, two binaries failing: both failing tests' names and panics reach the output,
#      a passing test's `ok` line does not, and the exit is the gate's own 12, never cargo's 101
#      (cargo-llvm-cov writes no export for a suite that failed, and neither does the stand-in);
#   2. a green suite prints none of the harness's output: the `ok` lines stay out of the log;
#   3. a failure the harness never reported in a `failures:` section (a binary killed by a
#      signal) still names what it has: the `FAILED` line and the end of the output.
. "$ROOT/test/lib.sh"

FX_EXP="$ROOT/test/fixtures/coverage/export.json"
[ -f "$FX_EXP" ] || { echo "    the coverage fixture export is missing"; exit 1; }
command -v python3 >/dev/null 2>&1 || skip "no python3"

# A scratch root the script resolves itself against: the script, and the crate directory it
# changes into before asking cargo.
W="$T/repo"
mkdir -p "$W/scripts" "$W/apps/majordomus-cli" "$T/shim"
cp "$ROOT/scripts/rust-coverage" "$W/scripts/rust-coverage"
chmod +x "$W/scripts/rust-coverage"

# The stand-in answers `llvm-cov --version`; otherwise it prints the harness output named by
# MJ_SHIM_OUT, writes the export only when MJ_SHIM_STATUS is 0 (as cargo-llvm-cov does), and
# exits with MJ_SHIM_STATUS.
cat > "$T/shim/cargo" <<'SHIM'
#!/bin/sh
[ "$*" = "llvm-cov --version" ] && { echo "cargo-llvm-cov 0.0.0-shim"; exit 0; }
out=""; prev=""
for a in "$@"; do [ "$prev" = "--output-path" ] && out="$a"; prev="$a"; done
cat "$MJ_SHIM_OUT"
echo "error: test failed, to rerun pass --test bench" >&2
if [ "$MJ_SHIM_STATUS" = 0 ] && [ -n "$out" ]; then cp "$MJ_SHIM_EXPORT" "$out"; fi
exit "$MJ_SHIM_STATUS"
SHIM
chmod +x "$T/shim/cargo"

gate() { # <harness output file> <cargo status>
  ( cd "$W" && PATH="$T/shim:$PATH" MJ_SHIM_OUT="$1" MJ_SHIM_STATUS="$2" \
      MJ_SHIM_EXPORT="$FX_EXP" TMPDIR="$T" scripts/rust-coverage --report 2>&1 )
}

# ---------------------------------------------------------------- 1. a red suite is named
cat > "$T/red.txt" <<'HARNESS'

running 2 tests
test bench_a_fast_target_answers ... ok
test the_same_run_again_is_within_policy ... FAILED

failures:

---- the_same_run_again_is_within_policy stdout ----
thread 'the_same_run_again_is_within_policy' panicked at tests/bench.rs:824:5:
bench --check: FAIL objects.get p50 +31%

failures:
    the_same_run_again_is_within_policy

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out


running 2 tests
test lease::keeps_its_lease ... ok
test lease::a_slow_owner_answers ... FAILED

failures:

---- lease::a_slow_owner_answers stdout ----
thread 'lease::a_slow_owner_answers' panicked at src/lease.rs:164:5:
assertion `left == right` failed
  left: 10
 right: 0

failures:
    lease::a_slow_owner_answers

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

HARNESS
rc=0
out="$(gate "$T/red.txt" 101)" || rc=$?
[ "$rc" = 12 ] || { echo "    a red suite under coverage exited $rc, not the gate's 12"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
for want in "---- the_same_run_again_is_within_policy stdout ----" "FAIL objects.get p50 +31%" \
            "---- lease::a_slow_owner_answers stdout ----" "left: 10"; do
  case "$out" in
    *"$want"*) ;;
    *) echo "    the gate's output does not carry: $want"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1 ;;
  esac
done
case "$out" in
  *"... ok"*) echo "    the gate printed the passing tests too, burying the failure"; exit 1 ;;
esac

# ---------------------------------------------------------------- 2. a green suite is quiet
cat > "$T/green.txt" <<'HARNESS'

running 2 tests
test bench_a_fast_target_answers ... ok
test the_same_run_again_is_within_policy ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

HARNESS
rc=0
out="$(gate "$T/green.txt" 0)" || rc=$?
[ "$rc" = 0 ] || { echo "    a green suite exited $rc"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
case "$out" in
  *"... ok"*|*"test result"*|*"harness reported"*)
    echo "    a green run printed the harness's output"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1 ;;
esac

# ---------------------------------------------------------------- 3. no failures section
cat > "$T/killed.txt" <<'HARNESS'

running 3 tests
test mesh::a_peer_answers ... ok
test mesh::a_peer_that_crashes ... FAILED
HARNESS
rc=0
out="$(gate "$T/killed.txt" 101)" || rc=$?
[ "$rc" = 12 ] || { echo "    a killed binary exited $rc, not the gate's 12"; exit 1; }
case "$out" in
  *"test mesh::a_peer_that_crashes ... FAILED"*) ;;
  *) echo "    a failure with no failures section is not named"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1 ;;
esac

# The scratch TMPDIR holds nothing the gate left behind but the export of the green run.
left="$(find "$T" -maxdepth 1 -name 'mj-cov-out.*' | LC_ALL=C sort)"
[ -z "$left" ] || { echo "    the gate left its captured output behind: $left"; exit 1; }
