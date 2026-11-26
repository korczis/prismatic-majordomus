# majordomus-covers: none
# A build directory variable set but empty is read as unset, so the sweep still sweeps.
#
# Cargo refuses CARGO_TARGET_DIR and CARGO_BUILD_BUILD_DIR set to an empty string ("the target
# directory is set to an empty string"), and lib/rust_bin.sh asks cargo where a build keeps
# its intermediates before it removes anything. On 2026-11-05 a caller wrote
# `CARGO_TARGET_DIR= mj_rust_prune_build_dir ...`, meaning "unset": every metadata call
# failed, the sweep said only "cargo metadata failed" and returned 0, a caller that discarded
# its output saw nothing, and a build host sat under its 35 GiB floor for over an hour.
# Unset, the same sweep freed 4.7 GB.
#
# What is proved, against fixture build directories under $T (nothing outside it is touched):
#   - with CARGO_TARGET_DIR= the build directory CARGO_BUILD_BUILD_DIR names is swept;
#   - with both variables (and CARGO_BUILD_TARGET_DIR) empty, the build-dir of the tree's own
#     .cargo/config.toml is cargo's answer, and it is swept;
#   - when cargo cannot answer at all, the refusal names the empty variable as a likely cause.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/rust_bin.sh"
command -v cargo >/dev/null 2>&1 \
  || { echo "    cargo is not on PATH: the build directory is cargo's answer"; exit 1; }

SIG='Signature: 8a477f597d28d172789f06886806bc55'
OLD=202001010000   # touch -t: long idle, under any age the sweep is given

B="$T/build"
IDLE="$B/debug/incremental/crate_a-1aaa/s-h0old-aaa-svh1"
FRESH="$B/debug/incremental/crate_b-2bbb/s-h0fresh-bbb-svh2"
fixture() {
  rm -rf "$B"
  mkdir -p "$B/debug/.fingerprint" "$IDLE" "$FRESH"
  printf '%s\n' "$SIG" > "$B/CACHEDIR.TAG"
  head -c 65536 /dev/zero > "$IDLE/query-cache.bin"
  head -c 65536 /dev/zero > "$FRESH/query-cache.bin"
  touch -t "$OLD" "$IDLE/query-cache.bin" "$IDLE"
}

# prune <tree-root> [VAR=value ...] -- the incremental sweep in a fresh shell; sets RC and OUT
prune() {
  pr_root="$1"; shift
  RC=0
  OUT="$(env MAJORDOMUS_INCREMENTAL_IDLE_MIN=30 "$@" \
    sh -c '. "$1"; mj_rust_prune_incremental "$2"' sh "$LIB" "$pr_root" 2>&1)" || RC=$?
  LAST_OUT="$OUT"
}

swept() {
  [ "$RC" = 0 ] || { echo "    the sweep failed (exit $RC): $OUT"; exit 1; }
  expect_no_grep 'cargo metadata failed' || exit 1
  [ ! -e "$IDLE" ] || { echo "    $1: the idle session was left:"; printf '%s\n' "$OUT"; exit 1; }
  [ -e "$FRESH" ] || { echo "    $1: a fresh session was removed:"; printf '%s\n' "$OUT"; exit 1; }
  expect_grep 'removing 1' || exit 1
}

# ---------------------------------------------------------------- CARGO_TARGET_DIR= is unset
fixture
prune "$ROOT" CARGO_BUILD_BUILD_DIR="$B" CARGO_TARGET_DIR=
swept 'CARGO_TARGET_DIR set empty'

# ---------------------------------------------------------------- every directory variable empty
# A tree of its own whose .cargo/config.toml names the fixture as its build-dir, with the
# repository's toolchain pin, so the pinned cargo answers from the config once the empty
# variables are read as unset; cargo's default would be inside $T as well.
R="$T/root"
mkdir -p "$R/.cargo" "$R/apps/majordomus-cli/src"
cp "$ROOT/rust-toolchain.toml" "$R/rust-toolchain.toml"
printf '[build]\nbuild-dir = "%s"\n' "$B" > "$R/.cargo/config.toml"
printf '[package]\nname = "majordomus-cli"\nversion = "0.0.0"\nedition = "2021"\n' \
  > "$R/apps/majordomus-cli/Cargo.toml"
: > "$R/apps/majordomus-cli/src/lib.rs"
fixture
prune "$R" CARGO_BUILD_BUILD_DIR= CARGO_TARGET_DIR= CARGO_BUILD_TARGET_DIR=
swept 'every directory variable set empty'

# ---------------------------------------------------------------- the refusal names the cause
fixture
mkdir -p "$T/stub"
printf '#!/bin/sh\nexit 101\n' > "$T/stub/cargo"
chmod +x "$T/stub/cargo"
prune "$ROOT" PATH="$T/stub:$PATH" CARGO_BUILD_BUILD_DIR="$B" CARGO_TARGET_DIR=
[ "$RC" = 0 ] || { echo "    a sweep without cargo's answer failed (exit $RC): $OUT"; exit 1; }
[ -e "$IDLE" ] || { echo "    a sweep without cargo's answer removed something: $OUT"; exit 1; }
expect_grep 'none removed: cargo did not say where this build keeps them \(cargo metadata failed' \
  || exit 1
expect_grep 'set but empty: CARGO_TARGET_DIR, which cargo refuses' || exit 1
expect_no_grep 'CARGO_BUILD_BUILD_DIR' || exit 1

echo "    an empty CARGO_TARGET_DIR or CARGO_BUILD_BUILD_DIR is read as unset, and named"
