# majordomus-covers: none
# A build never lets this crate's stale artifacts fill the shared build directory, and no sweep
# of it removes anything a build is using (I2787, I2713).
#
# Every generation of majordomus-cli leaves its library, its test binaries and its object
# files in <profile>/deps for good; on 2026-10-16 the shared directory's deps reached 204 GB
# and the next morning the volume reached zero. lib/rust_bin.sh's mj_rust_prune_build_dir
# removes the stale generations before a build, in every profile of the build directory —
# a coverage run's llvm-cov-target/debug included — and only while it holds cargo's own lock
# on that profile and no process names it: `cargo test` releases the lock before the
# binaries and doctests it built run, and a sweep that judged by age alone once removed rlibs
# a running doctest was about to read.
#
# What is proved, against a fixture build directory named to cargo by CARGO_BUILD_BUILD_DIR
# (so cargo's own answer is what is read, and nothing outside $T is ever touched):
#   - a generation of this crate's units no file of which was written within the age is
#     removed whole, in debug and in the coverage profile, and what was removed is said;
#   - a generation with one recent file, one held open, another crate's artifact and a name
#     with no unit hash are never touched;
#   - while a build holds cargo's lock on the profile, nothing in it is removed and that is
#     said; while a process names the profile, nothing in it is removed and that is said;
#   - an age of 0 removes nothing, and the test runner sets it to 0 for every other case.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_rust_prune_build_dir()' "$LIB" \
  || { echo "    lib/rust_bin.sh defines no mj_rust_prune_build_dir"; exit 1; }
grep -qE '^MAJORDOMUS_INCREMENTAL_IDLE_MIN=0; MAJORDOMUS_DEPS_IDLE_MIN=0' "$ROOT/test/run.sh" \
  || { echo "    test/run.sh does not turn the build directory sweeps off: a case's build would sweep the machine"; exit 1; }
command -v cargo >/dev/null 2>&1 \
  || { echo "    cargo is not on PATH: the build directory is cargo's answer, so it cannot be asked"; exit 1; }

SIG='Signature: 8a477f597d28d172789f06886806bc55'
OLD=202001010000   # touch -t: long idle, under any age the sweep is given
B="$T/build"
D="$B/debug/deps"
C="$B/llvm-cov-target/debug/deps"

art() {  # art <path> [old] -- a file of a little data; `old` makes it idle
  mkdir -p "$(dirname "$1")"
  head -c 32768 /dev/zero > "$1"
  if [ "${2:-}" = old ]; then touch -t "$OLD" "$1"; fi
}
fixture() {
  rm -rf "$B"
  mkdir -p "$B/debug/.fingerprint" "$B/llvm-cov-target/debug/.fingerprint"
  printf '%s\n' "$SIG" > "$B/CACHEDIR.TAG"
  # a stale generation of the library and one of a test binary (tests/cli.rs)
  art "$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib" old
  art "$D/majordomus_cli-aaaaaaaaaaaaaaaa.d" old
  art "$D/majordomus_cli-aaaaaaaaaaaaaaaa.majordomus_cli.1a2b3c-cgu.0.rcgu.o" old
  art "$D/cli-bbbbbbbbbbbbbbbb" old
  art "$D/cli-bbbbbbbbbbbbbbbb.d" old
  # a generation one file of which was written just now: all of it is kept
  art "$D/majordomus_cli-cccccccccccccccc.d" old
  art "$D/libmajordomus_cli-cccccccccccccccc.rlib"
  # another crate's artifact, and a name with no unit hash
  art "$D/libserde_json-dddddddddddddddd.rlib" old
  art "$D/majordomus_cli-notahash.d" old
  # a generation held open by a running process
  art "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib" old
  # the coverage run's profile, nested in the build directory
  art "$C/majordomus_cli-ffffffffffffffff.d" old
  art "$C/libmajordomus_cli-ffffffffffffffff.rlib" old
}
STALE="$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib $D/majordomus_cli-aaaaaaaaaaaaaaaa.d
$D/majordomus_cli-aaaaaaaaaaaaaaaa.majordomus_cli.1a2b3c-cgu.0.rcgu.o $D/cli-bbbbbbbbbbbbbbbb
$D/cli-bbbbbbbbbbbbbbbb.d $C/majordomus_cli-ffffffffffffffff.d $C/libmajordomus_cli-ffffffffffffffff.rlib"
NEVER="$D/majordomus_cli-cccccccccccccccc.d $D/libmajordomus_cli-cccccccccccccccc.rlib
$D/libserde_json-dddddddddddddddd.rlib $D/majordomus_cli-notahash.d
$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib $B/CACHEDIR.TAG"

# prune [VAR=value ...] -- the sweep in a fresh shell; sets RC and OUT
prune() {
  RC=0
  OUT="$(env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_DEPS_IDLE_MIN=120 "$@" \
    sh -c '. "$1"; mj_rust_prune_build_dir "$2"' sh "$LIB" "$ROOT" 2>&1)" || RC=$?
  LAST_OUT="$OUT"
}
kept() {
  for p in "$@"; do
    [ -e "$p" ] || { echo "    the sweep removed what it must never touch: $p"; printf '%s\n' "$OUT"; exit 1; }
  done
}
gone() {
  for p in "$@"; do
    [ ! -e "$p" ] || { echo "    a stale artifact was left behind: $p"; printf '%s\n' "$OUT"; exit 1; }
  done
}
# shellcheck disable=SC2086 # the lists are whitespace-separated paths without blanks
never_all() { kept $NEVER; }
# shellcheck disable=SC2086
stale_kept() { kept $STALE; }
# shellcheck disable=SC2086
stale_gone() { gone $STALE; }

PIDS=""
cleanup_pids() { for p in $PIDS; do pkill -P "$p" 2>/dev/null || true; kill "$p" 2>/dev/null || true; done; PIDS=""; }
trap cleanup_pids EXIT

# without flock(1) or lsof the sweep cannot take cargo's lock or see what is open, and then it
# must remove nothing and say why; the rest of this case needs both
if ! command -v flock >/dev/null 2>&1 || ! command -v lsof >/dev/null 2>&1; then
  fixture
  prune
  stale_kept; never_all
  expect_grep 'flock\(1\) is not installed|could not be read \(lsof\)'
  echo "    ok (degraded): without flock(1) or lsof nothing is removed, and that is said"
  exit 0
fi

# hold <path>: a process that holds that file open until the case ends
hold() {
  ( exec 7< "$1"; sleep 120 ) &
  PIDS="$PIDS $!"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    lsof -n -P -F n "$1" 2>/dev/null | grep -q "^n" && return 0
    sleep 0.2
  done
  echo "    the fixture could not hold $1 open"; exit 1
}

# ---------------------------------------------------------------- stale generations go, the rest stays
fixture
hold "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"
prune
[ "$RC" = 0 ] || { echo "    the sweep failed (exit $RC): $OUT"; exit 1; }
stale_gone; never_all
expect_grep "stale crate artifacts of $D, no file written for over 120 min: removing 2 generation\\(s\\), 5 file\\(s\\)"
expect_grep "stale crate artifacts of $C, no file written for over 120 min: removing 1 generation\\(s\\), 2 file\\(s\\)"
expect_grep 'majordomus_cli-aaaaaaaaaaaaaaaa'
expect_grep 'cli-bbbbbbbbbbbbbbbb'
expect_no_grep 'cccccccccccccccc|dddddddddddddddd|eeeeeeeeeeeeeeee|notahash'
expect_grep 'removed 5 file\(s\); free on its volume'
cleanup_pids

# once nothing holds it, the generation that was open is stale like the others
prune
expect_grep "removing 1 generation\\(s\\), 1 file\\(s\\)"
expect_grep 'eeeeeeeeeeeeeeee'
gone "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"

# and a build after that finds nothing to do and says nothing
prune
[ "$RC" = 0 ] && [ -z "$OUT" ] || { echo "    a sweep with nothing to reclaim said: $OUT"; exit 1; }

# ---------------------------------------------------------------- an age of 0 removes nothing
fixture
prune MAJORDOMUS_DEPS_IDLE_MIN=0
[ "$RC" = 0 ] && [ -z "$OUT" ] || { echo "    an age of 0 was not a no-op: $OUT"; exit 1; }
stale_kept; never_all

# ---------------------------------------------------------------- cargo's lock is held
# a build holding the profile's lock: nothing in that profile is removed, and that is said;
# the coverage profile, which nobody holds, is still swept
fixture
: > "$B/debug/.cargo-build-lock"
flock -x "$B/debug/.cargo-build-lock" sleep 120 &
PIDS="$PIDS $!"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  flock -n -x "$B/debug/.cargo-build-lock" true 2>/dev/null || break
  sleep 0.2
done
flock -n -x "$B/debug/.cargo-build-lock" true 2>/dev/null \
  && { echo "    the fixture could not hold cargo's lock"; exit 1; }
prune
[ "$RC" = 0 ] || { echo "    a sweep that met a held lock failed the build (exit $RC): $OUT"; exit 1; }
kept "$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib" "$D/cli-bbbbbbbbbbbbbbbb"
gone "$C/majordomus_cli-ffffffffffffffff.d"
never_all
expect_grep "$B/debug not swept: a build holds cargo's lock on it"
cleanup_pids

# ---------------------------------------------------------------- a process names the profile
# a test binary or a doctest running from the profile holds no lock; it still keeps the sweep out
fixture
sh -c 'sleep 120; :' "$D/cli-bbbbbbbbbbbbbbbb" &
PIDS="$PIDS $!"
sleep 0.3
prune
[ "$RC" = 0 ] || { echo "    a sweep that met a running binary failed the build (exit $RC): $OUT"; exit 1; }
kept "$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib" "$D/cli-bbbbbbbbbbbbbbbb"
gone "$C/majordomus_cli-ffffffffffffffff.d"
never_all
expect_grep "$B/debug not swept: in use by [0-9]+ "
cleanup_pids

echo "    ok: stale generations are removed whole in every profile; recent, open, foreign and unhashed ones stay; nothing is removed under cargo's lock or while a process names the profile"
