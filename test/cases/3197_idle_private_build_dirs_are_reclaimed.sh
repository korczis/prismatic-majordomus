# majordomus-covers: none
# A build directory a build fell back to is removed once it is idle, and never while it is
# held, used or recent.
#
# mj_cargo_bounded builds in ${CARGO_HOME}/build-private/<tree> when the shared build
# directory stays locked by another build for a minute, and nothing ever removed those: on
# 2026-10-17 they held 178 GiB in 35 directories, 25 of them idle for over two hours, and the
# volume reached 3.9 GiB. lib/rust_bin.sh's mj_rust_reclaim_private, which the free-space
# check every build asks runs (at most once per ten minutes per machine), removes the idle
# ones whole.
#
# What is proved, against a fixture CARGO_HOME (nothing outside $T is ever touched):
#   - a cargo-marked private directory nothing in which was written within the age, held by
#     no build and named by no process, is removed and said;
#   - one with a recent file, one whose profile lock a build holds, one a running process
#     names, one with no mark of cargo's, a symlink and the one this shell builds in survive;
#   - an age of 0 removes nothing, the test runner sets it to 0, and the look is rate-limited.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_rust_reclaim_private()' "$LIB" \
  || { echo "    lib/rust_bin.sh defines no mj_rust_reclaim_private"; exit 1; }
sed -n '/^mj_rust_space_check()/,/^}/p' "$LIB" | grep -q 'mj_rust_reclaim_private' \
  || { echo "    the free-space check no longer reclaims private build directories"; exit 1; }
grep -qE 'MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN=0' "$ROOT/test/run.sh" \
  || { echo "    test/run.sh does not turn the private sweep off: a case's build would sweep the machine"; exit 1; }

SIG='Signature: 8a477f597d28d172789f06886806bc55'
OLD=202001010000
H="$T/cargo-home"
P="$H/build-private"

private() {  # private <name> [old|unmarked] -- a private build directory with one profile
  mkdir -p "$P/$1/debug/.fingerprint" "$P/$1/debug/deps"
  head -c 32768 /dev/zero > "$P/$1/debug/deps/libdep-0123456789abcdef.rlib"
  : > "$P/$1/debug/.cargo-build-lock"
  [ "${2:-}" = unmarked ] && rmdir "$P/$1/debug/.fingerprint" || printf '%s\n' "$SIG" > "$P/$1/CACHEDIR.TAG"
  if [ "${2:-}" = old ] || [ "${2:-}" = unmarked ]; then
    find "$P/$1" -exec touch -t "$OLD" {} +
  fi
}
fixture() {
  rm -rf "$H" "$T/cache" "$T/elsewhere"
  private idle old
  private recent
  private held old
  private used old
  private unmarked unmarked
  private mine old
  mkdir -p "$T/elsewhere/debug/.fingerprint"
  printf '%s\n' "$SIG" > "$T/elsewhere/CACHEDIR.TAG"
  find "$T/elsewhere" -exec touch -t "$OLD" {} +
  ln -s "$T/elsewhere" "$P/linked"
}

PIDS=""
cleanup_pids() { for p in $PIDS; do pkill -P "$p" 2>/dev/null || true; kill "$p" 2>/dev/null || true; done; PIDS=""; }
trap cleanup_pids EXIT

# sweep [VAR=value ...] -- the sweep in a fresh shell; sets RC and OUT
sweep() {
  RC=0
  OUT="$(env CARGO_HOME="$H" XDG_CACHE_HOME="$T/cache" MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN=120 \
    CARGO_BUILD_BUILD_DIR="$P/mine" "$@" \
    sh -c '. "$1"; mj_rust_reclaim_private' sh "$LIB" 2>&1)" || RC=$?
  LAST_OUT="$OUT"
}

if ! command -v flock >/dev/null 2>&1; then
  fixture
  sweep
  for d in idle recent held used unmarked mine linked; do
    [ -e "$P/$d" ] || { echo "    without flock(1) the sweep removed $d"; exit 1; }
  done
  echo "    ok (degraded): without flock(1) no build's lock can be asked, and nothing is removed"
  exit 0
fi

# ---------------------------------------------------------------- the idle one goes, nothing else
fixture
flock -x "$P/held/debug/.cargo-build-lock" sleep 120 &
PIDS="$PIDS $!"
sh -c 'sleep 120; :' "$P/used/debug/deps/a-test-binary" &
PIDS="$PIDS $!"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  flock -n -x "$P/held/debug/.cargo-build-lock" true 2>/dev/null || break
  sleep 0.2
done
sleep 0.3
sweep
[ "$RC" = 0 ] || { echo "    the sweep failed (exit $RC): $OUT"; exit 1; }
[ ! -e "$P/idle" ] || { echo "    an idle private build directory was left: $OUT"; exit 1; }
for d in recent held used unmarked mine linked; do
  [ -e "$P/$d" ] || { echo "    the sweep removed $d, which it must never touch: $OUT"; exit 1; }
done
[ -e "$T/elsewhere/CACHEDIR.TAG" ] || { echo "    the sweep followed a symlink out of build-private"; exit 1; }
for r in "$P"/.reclaiming-*; do
  [ ! -e "$r" ] || { echo "    a renamed directory was left behind: $r"; exit 1; }
done
expect_grep "idle private build directories under $P, nothing written for over 120 min, held and named by nothing: removed idle; kept 5"

# the look is rate-limited: a second build within the interval does not look again
private idle old
sweep
[ -e "$P/idle" ] && [ -z "$OUT" ] || { echo "    the sweep looked again within its interval: $OUT"; exit 1; }
cleanup_pids

# once the build and the process are gone and the interval has passed, those go too
rm -rf "$T/cache"
sweep
for d in idle held used; do
  [ ! -e "$P/$d" ] || { echo "    $d was kept after nothing held or named it: $OUT"; exit 1; }
done
for d in recent unmarked mine linked; do
  [ -e "$P/$d" ] || { echo "    the sweep removed $d: $OUT"; exit 1; }
done

# ---------------------------------------------------------------- an age of 0 removes nothing
fixture
sweep MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN=0
[ "$RC" = 0 ] && [ -z "$OUT" ] && [ -e "$P/idle" ] || { echo "    an age of 0 was not a no-op: $OUT"; exit 1; }

echo "    ok: an idle private build directory is removed whole; a recent, held, used, unmarked, linked or current one stays; the look is rate-limited"
