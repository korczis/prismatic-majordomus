# majordomus-covers: none
# A build never lets idle incremental sessions fill the disk (I2713).
#
# rustc keeps every crate's last incremental session for good, and the worktrees of this
# repository share one build directory (ADR 0139), so it keeps every worktree's. On
# 2026-10-14 one build directory held 24 GiB of it in 574 sessions, one of them live, and the
# machine fell to 4 GiB free; on 2026-10-16 the shared one held 79 GB in 454 sessions, five
# live, at 6.9 GiB free. Both times a person removed the idle ones by hand. lib/rust_bin.sh's
# mj_rust_prune_incremental is what every build of this repository now does first.
#
# What is proved, against a fixture build directory named to cargo by CARGO_BUILD_BUILD_DIR
# (so cargo's own answer is what is read, and nothing outside $T is ever touched):
#   - an idle session is removed, and what was removed is said with its age and size;
#   - a `-working` session, a fresh one, a lock file, a session reached through a symlink to
#     another build directory, and that other build directory itself are never touched;
#   - the sweep reclaims nothing when its inputs cannot be read: a listing that cannot be
#     read whole, a build directory cargo will not name, one that carries no mark of cargo's;
#   - an age of 0 removes nothing;
#   - the floor's own check and mj_cargo_bounded both sweep before a build, and the floor's
#     refusal names the largest build directories.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_rust_prune_incremental()' "$LIB" \
  || { echo "    lib/rust_bin.sh defines no mj_rust_prune_incremental"; exit 1; }
command -v cargo >/dev/null 2>&1 \
  || { echo "    cargo is not on PATH: the build directory is cargo's answer, so it cannot be asked"; exit 1; }

SIG='Signature: 8a477f597d28d172789f06886806bc55'
OLD=202001010000   # touch -t: long idle, under any age the sweep is given

# session <dir> [old] -- a session directory holding a little data; `old` makes it idle
session() {
  mkdir -p "$1"
  head -c 65536 /dev/zero > "$1/query-cache.bin"
  if [ "${2:-}" = old ]; then touch -t "$OLD" "$1/query-cache.bin" "$1"; fi
}

B="$T/build"        # the build directory under test
O="$T/other-build"  # another build directory: never this build's to sweep
fixture() {
  rm -rf "$B" "$O"
  mkdir -p "$B/debug/.fingerprint" "$O/debug/.fingerprint"
  printf '%s\n' "$SIG" > "$B/CACHEDIR.TAG"
  printf '%s\n' "$SIG" > "$O/CACHEDIR.TAG"
  session "$B/debug/incremental/crate_a-1aaa/s-h0old-aaa-svh1" old
  session "$B/debug/incremental/crate_a-1aaa/s-h0live-aaa-working" old
  session "$B/debug/incremental/crate_b-2bbb/s-h0fresh-bbb-svh2"
  session "$B/debug/incremental/crate_c-3ccc/s-h0gone-ccc-svh3" old
  : > "$B/debug/incremental/crate_a-1aaa/s-h0old-aaa.lock"
  touch -t "$OLD" "$B/debug/incremental/crate_a-1aaa/s-h0old-aaa.lock"
  session "$O/debug/incremental/crate_o-9ooo/s-h0other-ooo-svh9" old
  # a profile directory of this build that is a symlink into the other one
  ln -s "$O/debug" "$B/linked"
}

# prune [VAR=value ...] -- the sweep in a fresh shell; sets RC and OUT
prune() {
  RC=0
  OUT="$(env CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=30 "$@" \
    sh -c '. "$1"; mj_rust_prune_incremental "$2"' sh "$LIB" "$ROOT" 2>&1)" || RC=$?
  LAST_OUT="$OUT"
}

kept() {
  for p in "$@"; do
    [ -e "$p" ] || { echo "    the sweep removed what it must never touch: $p"; printf '%s\n' "$OUT"; exit 1; }
  done
}
gone() {
  for p in "$@"; do
    [ ! -e "$p" ] || { echo "    an idle session was left behind: $p"; printf '%s\n' "$OUT"; exit 1; }
  done
}
NEVER="$B/debug/incremental/crate_a-1aaa/s-h0live-aaa-working
$B/debug/incremental/crate_b-2bbb/s-h0fresh-bbb-svh2
$B/debug/incremental/crate_a-1aaa/s-h0old-aaa.lock
$O/debug/incremental/crate_o-9ooo/s-h0other-ooo-svh9
$B/CACHEDIR.TAG"
IDLE="$B/debug/incremental/crate_a-1aaa/s-h0old-aaa-svh1
$B/debug/incremental/crate_c-3ccc/s-h0gone-ccc-svh3"
# shellcheck disable=SC2086 # the lists are newline-separated paths without blanks
kept_all() { IFS='
'; kept $NEVER; unset IFS; }
# shellcheck disable=SC2086
idle_all_kept() { IFS='
'; kept $IDLE; unset IFS; }
# shellcheck disable=SC2086
idle_all_gone() { IFS='
'; gone $IDLE; unset IFS; }

# ---------------------------------------------------------------- idle sessions are reclaimed
fixture
prune
[ "$RC" = 0 ] || { echo "    the sweep failed (exit $RC): $OUT"; exit 1; }
idle_all_gone
kept_all
expect_grep 'unwritten for over 30 min: removing 2'
expect_grep 'idle +[0-9]+ min  debug/incremental/crate_a-1aaa/s-h0old-aaa-svh1'
expect_grep 'idle +[0-9]+ min  debug/incremental/crate_c-3ccc/s-h0gone-ccc-svh3'
expect_grep 'removed 2 holding [0-9]+MB; free on its volume [0-9]+MB before, [0-9]+MB after'
expect_no_grep 'working|fresh|other|\.lock'

# a second build finds nothing to do and says nothing
prune
[ "$RC" = 0 ] && [ -z "$OUT" ] || { echo "    a sweep with nothing to reclaim said: $OUT"; exit 1; }
kept_all

# ---------------------------------------------------------------- an age of 0 removes nothing
fixture
prune MAJORDOMUS_INCREMENTAL_IDLE_MIN=0
[ "$RC" = 0 ] && [ -z "$OUT" ] || { echo "    an age of 0 was not a no-op: $OUT"; exit 1; }
idle_all_kept; kept_all

# ---------------------------------------------------------------- inputs that cannot be read
# a listing that cannot be read whole: nothing at all is removed, not even what was readable
fixture
mkdir -p "$B/debug/incremental/crate_d-4ddd/s-h0shut-ddd-svh4"
chmod 000 "$B/debug/incremental/crate_d-4ddd"
if [ "$(id -u)" != 0 ]; then
  prune
  chmod 755 "$B/debug/incremental/crate_d-4ddd"
  [ "$RC" = 0 ] || { echo "    a sweep that could not read its listing failed the build (exit $RC)"; exit 1; }
  idle_all_kept; kept_all
  expect_grep 'none removed: .*/debug/incremental could not be read whole'
  expect_no_grep 'removing'
else
  chmod 755 "$B/debug/incremental/crate_d-4ddd"
  echo "    (running as root: a mode cannot make the listing unreadable, so that refusal is not exercised here)"
fi

# a build directory cargo does not name: nothing is guessed, nothing is removed
fixture
mkdir -p "$T/stub"
printf '#!/bin/sh\nexit 101\n' > "$T/stub/cargo"
chmod +x "$T/stub/cargo"
prune PATH="$T/stub:$PATH"
[ "$RC" = 0 ] || { echo "    a sweep without cargo's answer failed the build (exit $RC)"; exit 1; }
idle_all_kept; kept_all
expect_grep 'none removed: cargo did not say where this build keeps them'

# a directory that carries no mark of cargo's is not provably a build directory
fixture
rm -f "$B/CACHEDIR.TAG"
rmdir "$B/debug/.fingerprint"
prune
idle_all_kept
expect_grep 'none removed: .* carries no mark of cargo'

# ---------------------------------------------------------------- at the entry points
# the floor's check sweeps first, and under the floor names the largest build directories
fixture
RC=0
OUT="$(env CARGO_BUILD_BUILD_DIR="$B" CARGO_HOME="$T/cargo-home" MAJORDOMUS_INCREMENTAL_IDLE_MIN=30 \
  MAJORDOMUS_MIN_FREE_MB=999999999 \
  sh -c '. "$1"; mj_rust_space_check "$2" "this build"' sh "$LIB" "$ROOT" 2>&1)" || RC=$?
LAST_OUT="$OUT"
[ "$RC" = 1 ] || { echo "    a build below the floor was allowed to start (exit $RC): $OUT"; exit 1; }
idle_all_gone; kept_all
expect_grep 'removing 2'
expect_grep 'refusing to start this build'
expect_grep 'Largest build directories'
# both build directories are named, by their physical paths, with a measured size
BP="$(cd -P "$B" && pwd -P)"; OP="$(cd -P "$O" && pwd -P)"
expect_grep "[0-9]+(MB|\\.[0-9]GB)  $BP\$"
expect_grep "[0-9]+(MB|\\.[0-9]GB)  $OP\$"

# mj_cargo_bounded sweeps before the build it runs, once, and asks cargo nothing else for it
fixture
REAL="$(command -v cargo)"
# shellcheck disable=SC2086 # newline-separated paths without blanks, made one line here
IDLE_WORDS="$(printf '%s ' $IDLE)"
mkdir -p "$T/fake"
cat > "$T/fake/cargo" <<SH
#!/bin/sh
if [ "\$1" = metadata ]; then exec "$REAL" "\$@"; fi
printf 'build %s\n' "\$*" >> "$T/calls.log"
for s in $IDLE_WORDS; do [ -e "\$s" ] && echo "idle session present at build: \$s" >> "$T/calls.log"; done
exit 0
SH
chmod +x "$T/fake/cargo"
: > "$T/calls.log"
RC=0
OUT="$(env PATH="$T/fake:$PATH" CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=30 \
  MAJORDOMUS_MIN_FREE_MB=0 \
  sh -c '. "$1"; mj_rust_space_check "$2" "this build" && mj_cargo_bounded "$2" build --quiet' \
  sh "$LIB" "$ROOT" 2>&1)" || RC=$?
LAST_OUT="$OUT"
[ "$RC" = 0 ] || { echo "    the bounded build failed (exit $RC): $OUT"; exit 1; }
grep -q '^build build' "$T/calls.log" || { echo "    the build did not run: $(cat "$T/calls.log")"; exit 1; }
if grep -q 'idle session present' "$T/calls.log"; then
  echo "    cargo build started with idle sessions still there: $(cat "$T/calls.log")"; exit 1
fi
idle_all_gone; kept_all
[ "$(grep -c 'removing' <<<"$OUT")" = 1 ] || { echo "    the floor and the build swept twice: $OUT"; exit 1; }

fixture
: > "$T/calls.log"
RC=0
OUT="$(env PATH="$T/fake:$PATH" CARGO_BUILD_BUILD_DIR="$B" MAJORDOMUS_INCREMENTAL_IDLE_MIN=30 \
  sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet' sh "$LIB" "$ROOT" 2>&1)" || RC=$?
LAST_OUT="$OUT"
[ "$RC" = 0 ] || { echo "    the bounded build failed (exit $RC): $OUT"; exit 1; }
if grep -q 'idle session present' "$T/calls.log"; then
  echo "    mj_cargo_bounded built before it swept: $(cat "$T/calls.log")"; exit 1
fi
idle_all_gone; kept_all
expect_grep 'removed 2 holding'

echo "    ok: idle sessions are reclaimed and said; live, fresh and foreign ones are never touched; unreadable inputs reclaim nothing"
