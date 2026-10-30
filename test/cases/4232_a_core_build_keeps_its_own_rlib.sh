# majordomus-covers: none
# A build of the portable core, from the repository's root, leaves the executable's rlib of it
# untouched in the shared build directory (I4232).
#
# Every worktree builds into one build directory (.cargo/config.toml), and cargo finds that
# config from the directory it runs in, not from --manifest-path. apps/majordomus-core has its
# own Cargo.lock. While its [lib] declared rlib+cdylib, cargo named the rlib without a hash,
# so `cargo test --manifest-path apps/majordomus-core/Cargo.toml` run from the root wrote the
# one libmajordomus_core.rlib every executable links. That happened on 2026-10-24 and again
# on 2026-10-25, and the next executable build anywhere failed at canonical_json. An rlib alone
# is named by its hash.
#
# What is proved, in a fixture repository with the same shape (a shared build-dir in its root
# .cargo/config.toml, a "cli" crate depending on a "core" crate that is its own workspace, and
# the core built differently by the two), whose core declares the crate types the real
# apps/majordomus-core/Cargo.toml declares:
#   - the cli's build of the core leaves one rlib of it in the shared build directory;
#   - `cargo test --manifest-path core/Cargo.toml`, run from the root, leaves that rlib byte
#     for byte as it was;
#   - with rlib+cdylib, the replay overwrites it, so the fixture can tell the two apart;
#   - scripts/rust-check builds the wasm32 module as a cdylib asked for on the command line.
. "$ROOT/test/lib.sh"

command -v cargo >/dev/null 2>&1 || skip "cargo is not installed: nothing here can build the fixture"

MANIFEST="$ROOT/apps/majordomus-core/Cargo.toml"
CT="$(awk '/^\[lib\]/ { lib = 1; next } /^\[/ { lib = 0 } lib && /^crate-type *=/ { sub(/^crate-type *= */, ""); print; exit }' "$MANIFEST")"
[ -n "$CT" ] || { echo "    $MANIFEST declares no crate-type under [lib]"; exit 1; }
case "$CT" in
  *cdylib*) echo "    $MANIFEST declares $CT: a cdylib makes cargo name the rlib without a hash"; exit 1 ;;
esac
grep -qE 'cargo rustc .*--crate-type cdylib' "$ROOT/scripts/rust-check" \
  || { echo "    scripts/rust-check does not ask for the wasm32 module as a cdylib"; exit 1; }

R="$T/repo"
SHARED="$T/shared"
mkdir -p "$R/.cargo" "$R/core/src" "$R/cli/src"
# the repository's pin: the machine's default toolchain may predate build-dir
cp "$ROOT/rust-toolchain.toml" "$R/rust-toolchain.toml"
printf '[build]\nbuild-dir = "%s"\n' "$SHARED" > "$R/.cargo/config.toml"
core_manifest() {  # core_manifest <crate-type value>
  printf '[package]\nname = "core"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n\n[lib]\nname = "core"\ncrate-type = %s\n\n[features]\nx = []\n' "$1" \
    > "$R/core/Cargo.toml"
}
printf 'pub fn n() -> u32 { if cfg!(feature = "x") { 2 } else { 1 } }\n#[test]\nfn t() { assert!(n() > 0); }\n' \
  > "$R/core/src/lib.rs"
# the cli builds the core with a feature its standalone build lacks, as the executable's lock
# resolves the core's dependencies differently from the core's own lock
printf '[package]\nname = "cli"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n\n[dependencies]\ncore = { path = "../core", features = ["x"] }\n' \
  > "$R/cli/Cargo.toml"
printf 'fn main() { println!("{}", core::n()); }\n' > "$R/cli/src/main.rs"

# replay <crate-type value> -- the cli build, then the core's from the root; sets BEFORE and AFTER
replay() {
  core_manifest "$1"
  rm -rf "$SHARED" "$R/cli/target" "$R/core/target" "$R/cli/Cargo.lock" "$R/core/Cargo.lock"
  (cd "$R" && env -u CARGO_BUILD_BUILD_DIR -u CARGO_TARGET_DIR cargo build --quiet --offline \
    --manifest-path cli/Cargo.toml) >"$T/build.log" 2>&1 \
    || { echo "    the fixture's cli did not build:"; cat "$T/build.log"; exit 2; }
  [ -d "$SHARED/debug/deps" ] || { echo "    the fixture's build did not use its shared build-dir"; cat "$T/build.log"; exit 2; }
  BEFORE="$(cd "$SHARED/debug/deps" && cksum libcore*.rlib)"
  [ "$(printf '%s\n' "$BEFORE" | grep -c .)" = 1 ] \
    || { echo "    the cli's build left not one rlib of the core: $BEFORE"; exit 2; }
  (cd "$R" && env -u CARGO_BUILD_BUILD_DIR -u CARGO_TARGET_DIR cargo test --quiet --offline \
    --manifest-path core/Cargo.toml) >"$T/test.log" 2>&1 \
    || { echo "    the fixture's core tests did not run:"; cat "$T/test.log"; exit 2; }
  AFTER="$(cd "$SHARED/debug/deps" && cksum libcore*.rlib)"
}

# ---------------------------------------------------------------- the core as it is declared
replay "$CT"
printf '%s\n' "$AFTER" | grep -qxF "$BEFORE" \
  || { echo "    the core's build from the root replaced the cli's rlib (crate-type $CT):"; echo "    before: $BEFORE"; echo "    after:  $AFTER"; exit 1; }

# ---------------------------------------------------------------- what the fixture tells apart
replay '["rlib", "cdylib"]'
if printf '%s\n' "$AFTER" | grep -qxF "$BEFORE"; then
  echo "    with rlib+cdylib the replay left the rlib as it was: the fixture cannot see the defect"
  exit 1
fi

echo "    ok: a core build from the root keeps the cli's rlib (crate-type $CT), where rlib+cdylib replaced it; the wasm32 module is a cdylib asked for by rust-check"
