# majordomus-covers: none
# A build never lets idle incremental sessions fill the disk (I2713, evidence
# idle_sessions_are_pruned_case; project.accumulation-is-measured clause 10).
#
# On 2026-10-14 one build directory held 807 incremental sessions — 24GB — of builds that
# failed, were interrupted or were cancelled, beside one compile that was running, and the
# machine fell to 4GB free. Cargo removes a crate's older sessions only at the end of a build
# that succeeds. So every build this repository's tooling starts asks lib/rust_bin.sh's
# mj_rust_space_check first, and that check now prunes the stale sessions of the build
# directory this build writes — its own and no other tree's — and says how much it freed.
# Below the floor its refusal names the largest build directories, so the next move is a
# path and not a search.
#
# What is proved, against a fixture and never the machine the suite runs on:
#   - stale sessions of the build's own directory go, with their lock files, and the line
#     that says so names the count and the directory;
#   - the crate's newest finalized session and a `-working` one stay, so the next build is
#     still incremental and a running compile keeps its session;
#   - nothing touched within the idle window (MAJORDOMUS_INCREMENTAL_IDLE_MIN) is stale;
#   - another tree's build directory is not touched, however stale its sessions are;
#   - a directory carrying no mark of cargo's is not touched;
#   - the test runner turns the pruning off (a case must not act on the machine it measures),
#     and MAJORDOMUS_PRUNE_INCREMENTAL=0 does the same anywhere;
#   - the refusal below the floor lists the largest build directories, largest first.
. "$ROOT/test/lib.sh"

# shellcheck source=../../lib/rust_bin.sh
. "$ROOT/lib/rust_bin.sh"

[ "${MAJORDOMUS_PRUNE_INCREMENTAL:-}" = 0 ] \
  || { echo "    test/run.sh does not turn the build's pruning off for the cases"; exit 1; }

FIX="$T/tree"
mkdir -p "$FIX/apps/majordomus-cli"
git -C "$FIX" init -q .
export CARGO_HOME="$T/cargo-home"

stamp() { printf 'Signature: 8a477f597d28d172789f06886806bc55\n' > "$1/CACHEDIR.TAG"; }
sessions() { # build-dir crate
  c="$1/debug/incremental/$2"
  mkdir -p "$c/s-old1" "$c/s-old2" "$c/s-newest" "$c/s-live-working"
  echo x > "$c/s-old1/work.bin"; : > "$c/s-old1.lock"; : > "$c/s-old2.lock"
  touch -t 202001010000 "$c/s-old1/work.bin" "$c/s-old1" "$c/s-old2"
  touch -t 202101010000 "$c/s-newest" "$c/s-live-working"
}
OWN="$T/own-build"
OTHER="$T/other-build"
UNMARKED="$T/unmarked-build"
mkdir -p "$OWN" "$OTHER" "$UNMARKED"
stamp "$OWN"; stamp "$OTHER"
sessions "$OWN" majordomus_cli-0abc
sessions "$OTHER" majordomus_cli-0abc
sessions "$UNMARKED" majordomus_cli-0abc
C="$OWN/debug/incremental/majordomus_cli-0abc"

# ---------------------------------------------------------------- off in the suite
out="$(CARGO_BUILD_BUILD_DIR="$OWN" MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_MIN_FREE_MB=1 \
  mj_rust_space_check "$FIX" "this build" 2>&1)" || true
[ -d "$C/s-old1" ] || { echo "    the build pruned with MAJORDOMUS_PRUNE_INCREMENTAL=0: $out"; exit 1; }

# a session touched within the idle window is not stale, however old the others think it is
out="$(MAJORDOMUS_PRUNE_INCREMENTAL=1 MAJORDOMUS_INCREMENTAL_IDLE_MIN=99999999 CARGO_BUILD_BUILD_DIR="$OWN" \
  MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || true
[ -d "$C/s-old1" ] || { echo "    a session within the idle window was pruned: $out"; exit 1; }

# ---------------------------------------------------------------- on: its own directory only
rc=0
out="$(MAJORDOMUS_PRUNE_INCREMENTAL=1 CARGO_BUILD_BUILD_DIR="$OWN" MAJORDOMUS_RECLAIM_BELOW_MB=0 \
  MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 0 ] || { echo "    the space check refused (exit $rc): $out"; exit 1; }
expect_grep "pruned 2 idle incremental session\(s\), [0-9]+MB, in $OWN before this build"
[ -e "$C/s-old1" ] && { echo "    a stale session survived"; exit 1; }
[ -e "$C/s-old2" ] && { echo "    a stale session survived"; exit 1; }
[ -e "$C/s-old1.lock" ] && { echo "    a stale session's lock survived"; exit 1; }
[ -d "$C/s-newest" ] || { echo "    the newest finalized session was pruned: the next build is not incremental"; exit 1; }
[ -d "$C/s-live-working" ] || { echo "    a working session was pruned under a running compile"; exit 1; }
[ -d "$OTHER/debug/incremental/majordomus_cli-0abc/s-old1" ] \
  || { echo "    another tree's build directory was pruned"; exit 1; }
[ -d "$UNMARKED/debug/incremental/majordomus_cli-0abc/s-old1" ] \
  || { echo "    a directory with no mark of cargo's was pruned"; exit 1; }

# a second build finds nothing left to prune and says nothing about it
out="$(MAJORDOMUS_PRUNE_INCREMENTAL=1 CARGO_BUILD_BUILD_DIR="$OWN" MAJORDOMUS_RECLAIM_BELOW_MB=0 \
  MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || true
LAST_OUT="$out"
expect_no_grep 'pruned'

# ---------------------------------------------------------------- the refusal names the largest
mkdir -p "$CARGO_HOME/build-private/big/debug" "$CARGO_HOME/build-private/small"
dd if=/dev/zero of="$CARGO_HOME/build-private/big/debug/blob" bs=1024 count=2048 2>/dev/null
echo x > "$CARGO_HOME/build-private/small/blob"
rc=0
out="$(CARGO_BUILD_BUILD_DIR="$OWN" MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_MIN_FREE_MB=99999999 \
  mj_rust_space_check "$FIX" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 1 ] || { echo "    the floor did not refuse (exit $rc)"; exit 1; }
expect_grep 'Largest build directories here:'
expect_grep "build-private/big"
big_line="$(printf '%s\n' "$out" | grep -n 'build-private/big' | head -n 1 | cut -d: -f1)"
small_line="$(printf '%s\n' "$out" | grep -n 'build-private/small' | head -n 1 | cut -d: -f1)"
[ -n "$big_line" ] && [ -n "$small_line" ] && [ "$big_line" -lt "$small_line" ] \
  || { echo "    the largest build directory is not named first: $out"; exit 1; }

echo "    ok: a build prunes its own build directory's stale incremental sessions and says so, keeps the newest, the working and the fresh ones, touches no other tree's, is off in the suite, and a refusal names the largest build directories"
