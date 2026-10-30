# majordomus-covers: none
# The reaper bounds the build directory cargo builds into, not one it worked out itself.
#
# .cargo/config.toml declares `build-dir = "{cargo-cache-home}/build-shared/majordomus"`, and
# scripts/reap-orphans expanded the template from the shell's CARGO_HOME. asdf's rust shim
# sets CARGO_HOME for cargo alone, so the shell's was unset, the reaper looked under
# ~/.cargo/build-shared/majordomus — which did not exist — and on 2026-10-16 the directory
# cargo really built into, ~/.asdf/installs/rust/<version>/build-shared/majordomus, held
# 214-261GB while the data volume reached 100%. Its 6144MB bound had never applied.
#
# The fixture reproduces exactly that: a `cargo` on PATH that sets its own CARGO_HOME, as the
# shim does, and a shell whose CARGO_HOME names another directory, laid out as a decoy that
# the old reaper reclaimed. Then the refusals (project.destructive-sweeps-fail-closed): no
# answer from cargo and an answer without a build directory each reclaim nothing and say why,
# a directory without cargo's CACHEDIR.TAG is never removed whole (only its stale units, I4340); and a session's private
# CARGO_BUILD_BUILD_DIR does not redirect the sweep onto that session's directory.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"
command -v cargo >/dev/null 2>&1 || skip "cargo absent"
command -v lsof >/dev/null 2>&1 || skip "lsof absent: the reaper refuses without it"

P="$(cd "$T" && pwd -P)"
FIX="$P/fix"; CRATE="$FIX/apps/majordomus-cli"
CARGO_SIDE="$P/cargo-home"; SHELL_SIDE="$P/shell-home"
REAL_DIR="$CARGO_SIDE/build-shared/majordomus"
DECOY_DIR="$SHELL_SIDE/build-shared/majordomus"
PRIVATE_DIR="$P/private-build"
mkdir -p "$CRATE/src" "$FIX/.cargo" "$P/stub" "$CARGO_SIDE" "$SHELL_SIDE"
# the repository's pin, because the fixture stands outside the repository and rustup picks
# the toolchain by directory: the machine's default may predate build-dir
cp "$ROOT/rust-toolchain.toml" "$FIX/rust-toolchain.toml"
cp "$ROOT/.cargo/config.toml" "$FIX/.cargo/config.toml"
grep -q '{cargo-cache-home}' "$FIX/.cargo/config.toml" \
  || { echo "    .cargo/config.toml no longer declares build-dir under {cargo-cache-home}"; exit 1; }
printf '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n' > "$CRATE/Cargo.toml"
printf 'pub fn n() -> u32 { 7 }\n' > "$CRATE/src/lib.rs"
(
  cd "$FIX"
  git init -q . && git config user.email t@example.com && git config user.name Test
  git add -A && git commit -qm base
  git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be built"; exit 2; }

# The toolchain's own cargo, not a version manager's shim: the shim sets CARGO_HOME itself
# and would resolve the template to this machine's real shared directory.
REAL_CARGO="$(command -v cargo)"
if command -v rustup >/dev/null 2>&1; then
  REAL_CARGO="$(cd "$CRATE" && rustup which cargo 2>/dev/null)" || REAL_CARGO="$(command -v cargo)"
fi
# What the asdf shim does — CARGO_HOME of its own for cargo alone — and one thing it does
# not: an answer naming a build directory outside this fixture is refused, so that no run of
# this case can point --reclaim at a directory of the machine's.
cat > "$P/stub/cargo" <<EOF
#!/bin/sh
case "\${FAKE_CARGO:-real}" in
  fail) exit 101 ;;
  old)  printf '{"packages":[],"target_directory":"%s/target","version":1}\n' "$CRATE"; exit 0 ;;
esac
out="\$(CARGO_HOME="$CARGO_SIDE" "$REAL_CARGO" "\$@")" || exit \$?
case "\$out" in
  *'"build_directory":"$P/'*) printf '%s\n' "\$out" ;;
  *) echo "fixture cargo: refusing an answer outside $P" >&2; exit 3 ;;
esac
EOF
chmod +x "$P/stub/cargo"
probe="$(cd "$CRATE" && PATH="$P/stub:$PATH" RUSTUP_AUTO_INSTALL=0 \
  cargo metadata --format-version 1 --no-deps --manifest-path "$CRATE/Cargo.toml" 2>/dev/null)" \
  || skip "cargo could not answer for a fixture crate here"
case "$probe" in
  *"\"build_directory\":\"$REAL_DIR\""*) ;;
  *) skip "this cargo does not resolve build-dir from its CARGO_HOME (no build_directory $REAL_DIR)" ;;
esac

# a build directory as cargo leaves one: its tag, and enough in it to exceed a 1MB bound
fill() { # dir [untagged]
  mkdir -p "$1/debug"
  if [ "${2:-}" = untagged ]; then echo '{}' > "$1/.rustc_info.json"
  else printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' > "$1/CACHEDIR.TAG"
  fi
  dd if=/dev/zero of="$1/debug/blob" bs=1024 count=2048 2>/dev/null
}
fill "$REAL_DIR"; fill "$DECOY_DIR"; fill "$PRIVATE_DIR"

# the process table is the reaper's liveness input: nobody is building
printf '#!/bin/sh\necho "/bin/sleep 1"\n' > "$P/stub/ps"; chmod +x "$P/stub/ps"
run() { # FAKE_CARGO-mode reaper-args...
  local mode="$1"; shift
  MJ_ROOT="$FIX" PATH="$P/stub:$PATH" FAKE_CARGO="$mode" CARGO_HOME="$SHELL_SIDE" \
    CARGO_BUILD_BUILD_DIR="$PRIVATE_DIR" MAJORDOMUS_SHARED_BUILD_CAP_MB=1 "$REAPER" "$@" 2>&1
}

# ---------------------------------------------------------------- cargo's directory is the subject
rc=0; LAST_OUT="$(run real --targets)" || rc=$?
[ "$rc" = 10 ] || { echo "    the directory cargo builds into was not reported (exit $rc)"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep "cargo-home/build-shared/majordomus +[0-9.]+(MB|GB) +would reclaim — shared, over its"
expect_no_grep 'shell-home|private-build'
[ -d "$REAL_DIR" ] || { echo "    the report removed the directory it only measures"; exit 1; }

rc=0; LAST_OUT="$(run real --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'cargo-home/build-shared/majordomus +[0-9.]+(MB|GB) +reclaimed'
[ ! -d "$REAL_DIR" ] || { echo "    --reclaim left the directory cargo builds into"; exit 1; }
[ -f "$DECOY_DIR/debug/blob" ] || { echo "    --reclaim removed the directory the shell's CARGO_HOME names"; exit 1; }
[ -f "$PRIVATE_DIR/debug/blob" ] || { echo "    --reclaim removed a session's private build directory"; exit 1; }

# ---------------------------------------------------------------- what cannot be proven is kept
# no CACHEDIR.TAG: another of cargo's marks is not enough for a removal of the whole directory,
# only for an eviction of its stale units under cargo's lock (I4340; case 3959 proves those go)
rm -rf "$REAL_DIR"; fill "$REAL_DIR" untagged
rc=0; LAST_OUT="$(run real --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc on an untagged directory"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'carries no CACHEDIR.TAG of cargo.s, so it is not removed whole: evicting stale units'
expect_no_grep 'cargo-home/build-shared/majordomus +[0-9.]+(MB|GB) +reclaimed'
[ -f "$REAL_DIR/debug/blob" ] || { echo "    an untagged directory was removed"; exit 1; }

rm -rf "$REAL_DIR"; fill "$REAL_DIR"
# cargo does not answer
rc=0; LAST_OUT="$(run fail --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc with cargo failing"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'cargo metadata failed in .*nothing reclaimed from it'
expect_grep 'skipped 1 for want of a readable input'
[ -f "$REAL_DIR/debug/blob" ] || { echo "    a directory was removed although cargo did not answer"; exit 1; }
[ -f "$DECOY_DIR/debug/blob" ] || { echo "    without cargo's answer the reaper fell back to the shell's CARGO_HOME"; exit 1; }

# cargo answers without a build directory (older than build-dir)
rc=0; LAST_OUT="$(run old --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc with an old cargo"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'named no absolute build_directory'
[ -f "$REAL_DIR/debug/blob" ] || { echo "    a directory was removed on an answer that named none"; exit 1; }
[ -f "$DECOY_DIR/debug/blob" ] || { echo "    on an old cargo the reaper fell back to the shell's CARGO_HOME"; exit 1; }

# no declaration: cargo's build directory is the crate's own target/, so nothing is shared
rm -f "$FIX/.cargo/config.toml"
rc=0; LAST_OUT="$(run real --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc with no declaration"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_no_grep 'shared by every worktree|shared, over its|cargo or rustc process|build-shared'
[ -f "$REAL_DIR/debug/blob" ] || { echo "    the reaper removed a directory no checkout declared"; exit 1; }
