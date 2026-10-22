# majordomus-covers: none
# One build directory never serves two worktrees' jobs at once (I2714).
#
# An exported CARGO_BUILD_BUILD_DIR is taken as the tree's own, and an export is inherited by
# every worktree a shell or a chain enters. Cargo's lock does not keep two of them apart:
# `cargo test` releases it before the binaries and doctests it built run. On 2026-10-14 the
# rust job judging I1501 lost about 600 doctests to another worktree's build in the same
# exported directory. mj_cargo_bounded now records which tree holds such a directory while a
# build runs, and a build of another tree that finds it held compiles nothing there. This case
# drives it with a stand-in `cargo`, so it compiles nothing.
#
# What is proved: two worktrees building with the same exported directory at once — the second
# moves to its own directory with a line naming the first; a tree's repeated builds, and a
# second build of the same tree while the first runs, stay in their directory and say
# nothing; a holder whose process is gone holds nothing; a build whose own directory is the
# one held stops with 75 before it runs cargo; the disk floor is asked before a move; the
# record is gone when the build ends, whatever its outcome; and the shared directory, which
# serves every worktree by design (ADR 0139), gets no record.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
grep -q '^mj_rust_hold_build_dir()' "$LIB" || { echo "    lib/rust_bin.sh defines no mj_rust_hold_build_dir"; exit 1; }

FAKE="$T/fake"; mkdir -p "$FAKE"
CALLS="$T/calls.log"
# The stand-in records the build directory each call was given; FAKE_SLEEP keeps it running
# (a build in progress), FAKE_RC is its exit status.
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
[ "$1" = metadata ] && exit 1
printf 'dir=%s\n' "${CARGO_BUILD_BUILD_DIR:-shared}" >> "$FAKE_CALLS"
[ -z "${FAKE_SLEEP:-}" ] || sleep "$FAKE_SLEEP"
exit "${FAKE_RC:-0}"
SH
chmod +x "$FAKE/cargo"

A="$T/tree-a"; B="$T/tree-b"; mkdir -p "$A" "$B"
A="$(cd "$A" && pwd -P)"; B="$(cd "$B" && pwd -P)"
DIR="$T/exported"; mkdir -p "$DIR"
HOLDERS="$DIR/.majordomus-holders"
OWN_B="$T/cargo-home/build-private/$(printf '%s' "$B" | cksum | awk '{ print $1 }')"

# build <tree> [VAR=value ...] -- mj_cargo_bounded under the stand-in; sets RC, ERR
build() {
  tree="$1"; shift
  RC=0
  ERR="$(env PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" CARGO_HOME="$T/cargo-home" \
        MAJORDOMUS_BUILD_LOCK_WAIT=5 MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 "$@" \
        sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet --locked' sh "$LIB" "$tree" 2>&1 >/dev/null)" || RC=$?
}
calls() { wc -l < "$CALLS" | tr -d ' '; }
records() { find "$HOLDERS" -type f 2>/dev/null | wc -l | tr -d ' '; }
LIVE=''
cleanup() { [ -z "$LIVE" ] || kill $LIVE 2>/dev/null || true; }
trap cleanup EXIT

# ---------------------------------------------------------------- two worktrees at once
# A's build runs in the exported directory; B's starts while it does.
: > "$CALLS"
env PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" FAKE_SLEEP=8 CARGO_HOME="$T/cargo-home" CARGO_BUILD_BUILD_DIR="$DIR" \
  MAJORDOMUS_BUILD_LOCK_WAIT=5 MAJORDOMUS_MIN_FREE_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 \
  sh -c '. "$1"; mj_cargo_bounded "$2" build --quiet' sh "$LIB" "$A" >/dev/null 2>"$T/a.err" &
APID=$!
LIVE="$APID"
i=0
until [ "$(records)" = 1 ] && grep -q '^dir=' "$CALLS"; do
  i=$((i + 1)); [ "$i" -le 100 ] || { echo "    the first build never recorded itself as the holder: $(cat "$T/a.err")"; exit 1; }
  sleep 0.1
done
grep -qxF "$A" "$HOLDERS"/* || { echo "    the holder record does not name the tree: $(cat "$HOLDERS"/*)"; exit 1; }
build "$B" CARGO_BUILD_BUILD_DIR="$DIR"
[ "$RC" = 0 ] || { echo "    the second worktree's build failed instead of moving: rc $RC: $ERR"; exit 1; }
[ "$(grep -c "^dir=$DIR\$" "$CALLS")" = 1 ] \
  || { echo "    the second worktree compiled in the held directory: $(cat "$CALLS")"; exit 1; }
grep -qxF "dir=$OWN_B" "$CALLS" || { echo "    the second worktree did not build in its own directory: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -qF "is held by another worktree, $A (pid " \
  || { echo "    the move did not name the holder: $ERR"; exit 1; }
printf '%s\n' "$ERR" | grep -q "building in this tree's own build directory instead: $OWN_B" \
  || { echo "    the move did not say where it went: $ERR"; exit 1; }
[ "$(find "$OWN_B/.majordomus-holders" -type f 2>/dev/null | wc -l | tr -d ' ')" = 0 ] \
  || { echo "    the moved build left its record behind"; exit 1; }
wait "$APID" || { echo "    the first build failed: $(cat "$T/a.err")"; exit 1; }
LIVE=''
[ "$(records)" = 0 ] || { echo "    the first build left its record behind: $(ls "$HOLDERS")"; exit 1; }
[ ! -s "$T/a.err" ] || { echo "    the first build, which nobody was in the way of, printed: $(cat "$T/a.err")"; exit 1; }

# ---------------------------------------------------------------- one tree: unaffected
: > "$CALLS"
build "$A" CARGO_BUILD_BUILD_DIR="$DIR"
build "$A" CARGO_BUILD_BUILD_DIR="$DIR"
[ "$RC" = 0 ] && [ "$(grep -c "^dir=$DIR\$" "$CALLS")" = 2 ] && [ -z "$ERR" ] \
  || { echo "    a tree's repeated builds were moved or spoke: rc $RC, $(cat "$CALLS"): $ERR"; exit 1; }
# a second build of the same tree while the first one runs shares the directory
: > "$CALLS"
sleep 300 & HOLDER=$!; LIVE="$HOLDER"
mkdir -p "$HOLDERS"; printf '%s\n' "$A" > "$HOLDERS/$HOLDER"
build "$A" CARGO_BUILD_BUILD_DIR="$DIR"
[ "$RC" = 0 ] && grep -qxF "dir=$DIR" "$CALLS" && [ -z "$ERR" ] \
  || { echo "    a build of the tree that holds the directory was moved: rc $RC, $(cat "$CALLS"): $ERR"; exit 1; }
[ -f "$HOLDERS/$HOLDER" ] || { echo "    a build removed a live holder's record"; exit 1; }

# ---------------------------------------------------------------- a failing build releases it too
: > "$CALLS"
rm -f "$HOLDERS/$HOLDER"
build "$A" CARGO_BUILD_BUILD_DIR="$DIR" FAKE_RC=101
[ "$RC" = 101 ] || { echo "    cargo's own exit status was not returned: $RC"; exit 1; }
[ "$(records)" = 0 ] || { echo "    a failing build left its record behind: $(ls "$HOLDERS")"; exit 1; }

# ---------------------------------------------------------------- a holder whose process is gone
sh -c 'exit 0' & DEAD=$!; wait "$DEAD" || true
printf '%s\n' "$B" > "$HOLDERS/$DEAD"
: > "$CALLS"
build "$A" CARGO_BUILD_BUILD_DIR="$DIR"
[ "$RC" = 0 ] && grep -qxF "dir=$DIR" "$CALLS" && [ -z "$ERR" ] \
  || { echo "    a dead holder kept the directory: rc $RC, $(cat "$CALLS"): $ERR"; exit 1; }
[ ! -f "$HOLDERS/$DEAD" ] || { echo "    a dead holder's record was left"; exit 1; }

# ---------------------------------------------------------------- the disk floor is asked before a move
printf '%s\n' "$A" > "$HOLDERS/$HOLDER"
: > "$CALLS"
build "$B" CARGO_BUILD_BUILD_DIR="$DIR" MAJORDOMUS_MIN_FREE_MB=999999999
[ "$RC" = 12 ] || { echo "    a move below the disk floor returned $RC, expected 12: $ERR"; exit 1; }
[ "$(calls)" = 0 ] || { echo "    a build ran below the disk floor: $(cat "$CALLS")"; exit 1; }

# ---------------------------------------------------------------- nowhere else to go
mkdir -p "$OWN_B/.majordomus-holders"; printf '%s\n' "$A" > "$OWN_B/.majordomus-holders/$HOLDER"
: > "$CALLS"
build "$B" CARGO_BUILD_BUILD_DIR="$OWN_B"
[ "$RC" = 75 ] || { echo "    a build whose own directory is held returned $RC, expected 75: $ERR"; exit 1; }
[ "$(calls)" = 0 ] || { echo "    it ran cargo in a held directory: $(cat "$CALLS")"; exit 1; }
printf '%s\n' "$ERR" | grep -qF "held by another worktree, $A (pid $HOLDER)" \
  || { echo "    the stop did not name the holder: $ERR"; exit 1; }
printf '%s\n' "$ERR" | grep -q 'stopping before compiling anything' || { echo "    the stop was not explained: $ERR"; exit 1; }
kill "$HOLDER" 2>/dev/null || true; LIVE=''

# ---------------------------------------------------------------- the shared directory gets no record
: > "$CALLS"
build "$B" env -u CARGO_BUILD_BUILD_DIR
[ "$RC" = 0 ] && grep -qxF 'dir=shared' "$CALLS" || { echo "    a build in the shared directory failed: rc $RC: $ERR"; exit 1; }
if find "$T" -path "$HOLDERS" -prune -o -path "$OWN_B" -prune -o -name .majordomus-holders -print | grep -q .; then
  echo "    a build in the shared directory left a holder record"; exit 1
fi
