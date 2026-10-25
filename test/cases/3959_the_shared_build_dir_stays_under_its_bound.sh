# majordomus-covers: none
# The shared build directory stays under its bound while builds run (I3959).
#
# On 2026-10-23 the build directory every worktree shares held 102 GB against its 6144MB
# bound: scripts/reap-orphans reduced it only as a whole and only when no cargo or rustc ran
# anywhere on the machine, which with nine sessions building was never, and the age sweeps of
# I2713 and I2787 ask nothing about size. lib/rust_bin.sh's mj_rust_bound_build_dir now evicts
# its stale units, oldest first, until it is under its bound — between builds, inside cargo's
# own lock on each profile — and the reaper runs it when a build is running.
#
# What is proved, against a fixture build directory and a fixture repository (no real cargo
# build, nothing outside $T is touched):
#   - over its bound with its locks free, the stale units are evicted in order — a coverage
#     run's llvm-cov-target and a dependency generation no worktree's Cargo.lock names first,
#     then an idle incremental session and this crate's idle generations by age — and the
#     eviction stops once the directory is under its bound; each unit is named with its size;
#   - a recent generation, a finished session's `-working` sibling, a dependency generation a
#     Cargo.lock names and the generation a running stand-in build holds open are kept;
#   - while a stand-in build holds cargo's lock on debug nothing in debug is evicted and that
#     is said, and the coverage profile, whose lock is free, is still evicted;
#   - while a process names the profile (a running test binary) nothing in it is evicted;
#   - --dry-run evicts nothing and says what it would; an idle age of 0 evicts nothing;
#   - the build path's pass (mj_rust_prune_build_dir) evicts and is rate-limited, and the pass
#     after a build (mj_rust_bound_after_build) waits for a held lock in the background,
#     bounded, and evicts once the stand-in build lets go, without the build waiting for it;
#   - scripts/reap-orphans, over the bound while a build runs, evicts stale units instead of
#     keeping the whole directory.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_rust_bound_build_dir()' "$LIB" \
  || { echo "    lib/rust_bin.sh defines no mj_rust_bound_build_dir"; exit 1; }
grep -qE '^MAJORDOMUS_INCREMENTAL_IDLE_MIN=0; .*MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=0' "$ROOT/test/run.sh" \
  || { echo "    test/run.sh does not turn the bound's eviction off: a case's build would evict the machine's"; exit 1; }
command -v flock >/dev/null 2>&1 || skip "flock(1) is not installed: cargo's lock cannot be taken, so nothing is evicted (said by mj_rust_locked)"
command -v lsof >/dev/null 2>&1 || skip "lsof is not installed: what is open cannot be read, so nothing is evicted"

SIG='Signature: 8a477f597d28d172789f06886806bc55'
B="$T/build"
D="$B/debug/deps"
COV="$B/llvm-cov-target"
INC="$B/debug/incremental/majordomus_cli-1x2y3z"
R="$T/repo"
CRATE="$R/apps/majordomus-cli"
REG="$T/registry/src/index.crates.io-0000000000000000"
export XDG_CACHE_HOME="$T/cache"

# the fixture repository: a crate whose Cargo.lock names serde 1.0.1, and nothing else
mkdir -p "$CRATE/src" "$CRATE/tests"
# the repository's pin: rustup picks the toolchain by directory, and the machine's default may
# predate build-dir, which the build path's pass asks cargo for
cp "$ROOT/rust-toolchain.toml" "$R/rust-toolchain.toml"
printf '[package]\nname = "majordomus-cli"\nversion = "0.1.0"\nedition = "2021"\n' > "$CRATE/Cargo.toml"
printf 'version = 4\n\n[[package]]\nname = "majordomus-cli"\nversion = "0.1.0"\n\n[[package]]\nname = "serde"\nversion = "1.0.1"\n' \
  > "$CRATE/Cargo.lock"
printf 'pub fn n() -> u32 { 7 }\n' > "$CRATE/src/lib.rs"
printf '#[test]\nfn t() {}\n' > "$CRATE/tests/cli.rs"
(
  cd "$R" && git init -q . && git config user.email t@example.com && git config user.name Test \
    && git add -A && git commit -qm base && git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be made"; exit 2; }

blob() {  # blob <path> <stamp> -- 2 MiB of data, written at that touch -t stamp
  mkdir -p "$(dirname "$1")"
  dd if=/dev/zero of="$1" bs=1024 count=2048 2>/dev/null
  touch -t "$2" "$1"
}
small() {  # small <path> <stamp> [text]
  mkdir -p "$(dirname "$1")"
  printf '%s\n' "${3:-x}" > "$1"
  touch -t "$2" "$1"
}
NOW="$(date +%Y%m%d%H%M)"
fixture() {
  rm -rf "$B"
  mkdir -p "$B/debug/.fingerprint" "$COV/debug/.fingerprint"
  printf '%s\n' "$SIG" > "$B/CACHEDIR.TAG"
  # rank 0: the coverage run's target directory, and a dependency no Cargo.lock names
  blob "$COV/debug/deps/libmajordomus_cli-9999999999999999.rlib" 202001010000
  touch -t 202001010000 "$COV/debug/deps" "$COV/debug/.fingerprint" "$COV/debug" "$COV"
  blob "$D/liboldcrate-ffffffffffffffff.rlib" 202001010100
  small "$D/oldcrate-ffffffffffffffff.d" 202001010100 \
    "$D/oldcrate-ffffffffffffffff.d: $REG/oldcrate-0.1.0/src/lib.rs"
  # rank 1, oldest first: an idle session, then this crate's idle generations
  blob "$INC/s-old-1abc/dep-graph.bin" 202001020000
  touch -t 202001020000 "$INC/s-old-1abc"
  blob "$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib" 202001030000
  small "$D/majordomus_cli-aaaaaaaaaaaaaaaa.d" 202001030000
  mkdir -p "$B/debug/.fingerprint/majordomus-cli-aaaaaaaaaaaaaaaa"
  blob "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib" 202001040000
  blob "$D/cli-bbbbbbbbbbbbbbbb" 202001050000
  small "$D/cli-bbbbbbbbbbbbbbbb.d" 202001050000
  # past the bound: idle too, but the eviction stops before it
  blob "$D/libmajordomus_cli-1111111111111111.rlib" 202001060000
  # never: recent, live, named
  blob "$D/libmajordomus_cli-cccccccccccccccc.rlib" "$NOW"
  blob "$INC/s-new-2def-working/dep-graph.bin" 202001020000
  touch -t 202001020000 "$INC/s-new-2def-working"
  blob "$D/libserde-dddddddddddddddd.rlib" 202001010000
  small "$D/serde-dddddddddddddddd.d" 202001010000 "$D/serde-dddddddddddddddd.d: $REG/serde-1.0.1/src/lib.rs"
  # the bound: whatever the volume's block sizes, the eviction needs five and a half units
  TOTAL="$(du -sk "$B" | awk '{ print $1 }')"
  CAP=$(( (TOTAL - 11264) / 1024 ))
}
EVICTED="$COV $D/liboldcrate-ffffffffffffffff.rlib $D/oldcrate-ffffffffffffffff.d $INC/s-old-1abc
$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib $D/majordomus_cli-aaaaaaaaaaaaaaaa.d
$B/debug/.fingerprint/majordomus-cli-aaaaaaaaaaaaaaaa $D/cli-bbbbbbbbbbbbbbbb $D/cli-bbbbbbbbbbbbbbbb.d"
DEBUG_EVICTED="$D/liboldcrate-ffffffffffffffff.rlib $INC/s-old-1abc $D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib
$D/cli-bbbbbbbbbbbbbbbb"
NEVER="$D/libmajordomus_cli-cccccccccccccccc.rlib $INC/s-new-2def-working $D/libserde-dddddddddddddddd.rlib
$D/libmajordomus_cli-1111111111111111.rlib $D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib $B/CACHEDIR.TAG"

# bound [mode] [VAR=value ...] -- the eviction in a fresh shell; sets RC and LAST_OUT
bound() {
  mode="$1"; shift
  RC=0
  LAST_OUT="$(env MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 "$@" \
    sh -c '. "$1"; mj_rust_bound_build_dir "$2" "$3" "$4"' sh "$LIB" "$B" "$CRATE" "$mode" 2>&1)" || RC=$?
}
kept() {
  for p in "$@"; do
    [ -e "$p" ] || { echo "    the eviction removed what it must keep: $p"; printf '%s\n' "$LAST_OUT"; exit 1; }
  done
}
gone() {
  for p in "$@"; do
    [ ! -e "$p" ] || { echo "    a stale unit was left behind: $p"; printf '%s\n' "$LAST_OUT"; exit 1; }
  done
}
# shellcheck disable=SC2086 # the lists are whitespace-separated paths without blanks
never_all() { kept $NEVER; }
# shellcheck disable=SC2086
evicted_kept() { kept $EVICTED; }

PIDS=""
cleanup_pids() {
  for p in $PIDS; do
    pkill -P "$p" 2>/dev/null || true; kill "$p" 2>/dev/null || true; wait "$p" 2>/dev/null || true
  done
  PIDS=""
}
trap cleanup_pids EXIT
hold() {  # hold <path>: a stand-in build that holds that file open
  ( exec 7< "$1"; sleep 120 ) &
  PIDS="$PIDS $!"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    lsof -n -P -F n "$1" 2>/dev/null | grep -q "^n" && return 0
    sleep 0.2
  done
  echo "    the fixture could not hold $1 open"; exit 1
}

# ---------------------------------------------------------------- a dry run changes nothing
fixture
bound dry-run
[ "$RC" = 10 ] || { echo "    a dry run over the bound did not say it would evict (exit $RC)"; printf '%s\n' "$LAST_OUT"; exit 1; }
evicted_kept; never_all
expect_grep 'over its [0-9.]+(MB|GB) bound: would evict 6 stale unit\(s\)'
expect_grep 'would evict +[0-9.]+(MB|GB) +coverage +idle +[0-9]+ min +llvm-cov-target'
expect_grep 'would evict .* unnamed .*oldcrate-0\.1\.0, which no worktree.s Cargo\.lock names'
expect_grep 'would evict .* incremental .*s-old-1abc'
expect_grep 'would evict .* crate .*aaaaaaaaaaaaaaaa'
expect_no_grep 'cccccccccccccccc|dddddddddddddddd|1111111111111111|s-new-2def-working'

# an idle age of 0 is the switch that turns eviction off, and the runner sets it for every case
bound evict MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=0
evicted_kept; never_all
expect_grep 'eviction is off'

# ---------------------------------------------------------------- a build holds debug's lock
: > "$B/debug/.cargo-build-lock"
flock -x "$B/debug/.cargo-build-lock" sleep 120 &
PIDS="$PIDS $!"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  flock -n -x "$B/debug/.cargo-build-lock" true 2>/dev/null || break
  sleep 0.2
done
flock -n -x "$B/debug/.cargo-build-lock" true 2>/dev/null \
  && { echo "    the fixture could not hold cargo's lock"; exit 1; }
bound evict
[ "$RC" = 0 ] || { echo "    an eviction that met a held lock failed (exit $RC)"; printf '%s\n' "$LAST_OUT"; exit 1; }
# shellcheck disable=SC2086
kept $DEBUG_EVICTED
never_all
gone "$COV"
expect_grep "$B/debug not swept: a build holds cargo's lock on it"
expect_grep 'evicted +[0-9.]+(MB|GB) +coverage'
expect_grep 'evicted 1 unit\(s\) holding .*; 5 left unreached'
cleanup_pids

# ---------------------------------------------------------------- a running test binary names debug
fixture
sh -c 'sleep 120; :' "$D/cli-bbbbbbbbbbbbbbbb" 2>/dev/null &
PIDS="$PIDS $!"
sleep 0.3
bound evict
# shellcheck disable=SC2086
kept $DEBUG_EVICTED
never_all
expect_grep "$B/debug not evicted from: in use by [0-9]+ "
cleanup_pids

# ---------------------------------------------------------------- the locks are free
# a stand-in build holds one generation open: it is kept and said; every other unit of the cut
# is evicted, oldest first, and the one past the bound is not
fixture
hold "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"
bound evict
[ "$RC" = 0 ] || { echo "    the eviction failed (exit $RC)"; printf '%s\n' "$LAST_OUT"; exit 1; }
# shellcheck disable=SC2086
gone $EVICTED
never_all
expect_grep 'evicting 6 stale unit\(s\)'
expect_grep 'evicted +[0-9.]+(MB|GB) +unnamed .*oldcrate-0\.1\.0'
expect_grep 'evicted +[0-9.]+(MB|GB) +crate .*bbbbbbbbbbbbbbbb'
expect_grep 'kept +[0-9.]+(MB|GB) +crate .*eeeeeeeeeeeeeeee +a file of it is open'
expect_grep 'evicted 5 unit\(s\) holding .*; 0 left unreached'
cleanup_pids
# the order is the plan's: rank 0 before rank 1, and age within it
order="$(printf '%s\n' "$LAST_OUT" | grep '^  evicted ' \
  | grep -oE 'llvm-cov-target|oldcrate|s-old-1abc|aaaaaaaaaaaaaaaa|bbbbbbbbbbbbbbbb' | tr '\n' ' ')"
[ "$order" = "llvm-cov-target oldcrate s-old-1abc aaaaaaaaaaaaaaaa bbbbbbbbbbbbbbbb " ] \
  || { echo "    evicted out of order: $order"; printf '%s\n' "$LAST_OUT"; exit 1; }

# under its bound now: a second pass has nothing to evict, and says so
bound evict MAJORDOMUS_SHARED_BUILD_CAP_MB=4096
expect_grep 'within its 4.0GB bound: nothing to evict'

# ---------------------------------------------------------------- the build path's pass
# mj_rust_prune_build_dir, which every bounded build runs first, applies the bound too (the age
# sweeps are given ages no fixture file meets, so the bound alone acts), at most every 120 s
fixture
hold "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"
RC=0
LAST_OUT="$(cd "$R" && env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=99999999 \
  MAJORDOMUS_DEPS_IDLE_MIN=99999999 MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 \
  sh -c '. "$1"; mj_rust_prune_build_dir "$2"' sh "$LIB" "$R" 2>&1)" || RC=$?
[ "$RC" = 0 ] || { echo "    the build path's pass failed (exit $RC)"; printf '%s\n' "$LAST_OUT"; exit 1; }
# shellcheck disable=SC2086
gone $EVICTED
never_all
expect_grep 'evicted 5 unit\(s\)'
cleanup_pids
fixture
LAST_OUT="$(env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 \
  sh -c '. "$1"; mj_rust_bound_after_build "$2"' sh "$LIB" "$R" 2>&1)" || true
[ -z "$LAST_OUT" ] || { echo "    a second pass within 120 s was not rate-limited: $LAST_OUT"; exit 1; }
evicted_kept

# with every profile locked by a build, a build's own pass makes no plan, evicts nothing, says so
fixture
export XDG_CACHE_HOME="$T/cache-locked"
: > "$B/debug/.cargo-build-lock"; : > "$COV/debug/.cargo-build-lock"
flock -x "$B/debug/.cargo-build-lock" flock -x "$COV/debug/.cargo-build-lock" sleep 60 &
PIDS="$PIDS $!"
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
  flock -n -x "$COV/debug/.cargo-build-lock" true 2>/dev/null || break
  sleep 0.2
done
LAST_OUT="$(cd "$R" && env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=99999999 \
  MAJORDOMUS_DEPS_IDLE_MIN=99999999 MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 \
  sh -c '. "$1"; mj_rust_prune_build_dir "$2"' sh "$LIB" "$R" 2>&1)" || true
evicted_kept; never_all
expect_grep 'every profile is locked by a build now, so nothing is evicted before this build'
cleanup_pids
export XDG_CACHE_HOME="$T/cache"

# ---------------------------------------------------------------- after a build, in the background
# cargo's lock is rarely free on a busy machine, so the pass after a build waits for it, bounded,
# in the background: the build returns at once, and the units go once the stand-in lets go
fixture
export XDG_CACHE_HOME="$T/cache-after"
hold "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"
flock -x "$B/debug/.cargo-build-lock" sleep 4 &
PIDS="$PIDS $!"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  flock -n -x "$B/debug/.cargo-build-lock" true 2>/dev/null || break
  sleep 0.2
done
RC=0
LAST_OUT="$(env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 \
  MAJORDOMUS_SHARED_BUILD_EVICT_LOCK_WAIT=30 sh -c '. "$1"; mj_rust_bound_after_build "$2"' sh "$LIB" "$R" 2>&1)" || RC=$?
[ "$RC" = 0 ] || { echo "    the pass after a build failed (exit $RC)"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'its stale units are evicted in the background once cargo.s lock is free \(pid [0-9]+, at most 30s'
waiter="$(printf '%s\n' "$LAST_OUT" | sed -n 's/.*(pid \([0-9]*\),.*/\1/p')"
[ -n "$waiter" ] || { echo "    the pass after a build named no waiter"; exit 1; }
# while the stand-in holds debug's lock its units are still there
kept "$D/libmajordomus_cli-aaaaaaaaaaaaaaaa.rlib" "$D/liboldcrate-ffffffffffffffff.rlib"
for _ in $(seq 1 150); do kill -0 "$waiter" 2>/dev/null || break; sleep 0.2; done
kill -0 "$waiter" 2>/dev/null && { echo "    the background waiter outlived its bound"; exit 1; }
LOG="$(ls "$XDG_CACHE_HOME"/majordomus/bound-*.log)"
LAST_OUT="$(cat "$LOG")"
# shellcheck disable=SC2086
gone $EVICTED
never_all
expect_grep 'pass after a build of .*, waiting up to 30s'
expect_grep 'evicted 5 unit\(s\)'
cleanup_pids
export XDG_CACHE_HOME="$T/cache"

# ---------------------------------------------------------------- the reaper, while a build runs
# its liveness input is the process table, so that is the one thing stubbed: a cargo is running
mkdir -p "$T/stub"
printf '#!/bin/sh\necho "/Users/x/.cargo/bin/cargo build"\n' > "$T/stub/ps"; chmod +x "$T/stub/ps"
fixture
hold "$D/libmajordomus_cli-eeeeeeeeeeeeeeee.rlib"
RC=0
LAST_OUT="$(MJ_ROOT="$R" PATH="$T/stub:$PATH" MAJORDOMUS_SHARED_BUILD_DIR="$B" MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" \
  MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 "$ROOT/scripts/reap-orphans" --targets 2>&1)" || RC=$?
[ "$RC" = 10 ] || { echo "    the reaper's report over the bound with a build running exited $RC, not 10"; printf '%s\n' "$LAST_OUT"; exit 1; }
evicted_kept; never_all
expect_grep 'a cargo or rustc process is running: stale units would be evicted under cargo.s lock'
expect_grep 'would evict 6 stale unit'
RC=0
LAST_OUT="$(MJ_ROOT="$R" PATH="$T/stub:$PATH" MAJORDOMUS_SHARED_BUILD_DIR="$B" MAJORDOMUS_SHARED_BUILD_CAP_MB="$CAP" \
  MAJORDOMUS_SHARED_BUILD_EVICT_IDLE_MIN=60 "$ROOT/scripts/reap-orphans" --targets --reclaim 2>&1)" || RC=$?
[ "$RC" = 0 ] || { echo "    the reaper's --reclaim exited $RC"; printf '%s\n' "$LAST_OUT"; exit 1; }
# shellcheck disable=SC2086
gone $EVICTED
never_all
[ -d "$B" ] || { echo "    the reaper removed the whole directory under a running build"; exit 1; }
expect_grep 'evicting stale units under cargo.s lock'
expect_grep 'evicted 5 unit\(s\)'
cleanup_pids

echo "    ok: over its bound the build directory loses its stale units oldest first until it is under it, under cargo's lock, from the build path and the reaper; held, named, open, recent and named-by-a-Cargo.lock units stay"
