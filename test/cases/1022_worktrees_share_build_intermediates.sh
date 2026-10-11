# majordomus-covers: none
# Worktrees share what cargo compiles on the way, and nothing else.
#
# Every worktree of this repository compiles the same dependencies into its own target/,
# 2.45GiB apiece (ADR 0055). Sharing one CARGO_TARGET_DIR would have saved that and
# given up something worse: every worktree would overwrite one debug/majordomus, so a build
# in one checkout would replace the executable another was running or borrowing — the failure
# project.reclaim-only-what-you-own was written about. cargo's `build-dir` splits the two:
# intermediates go to one directory, finished artifacts stay in each worktree's target-dir.
#
# This case holds the declaration, the behaviour that makes it worth having, and the bound
# that keeps an accumulating directory from becoming the next full disk
# (project.accumulation-is-measured).
. "$ROOT/test/lib.sh"

CFG="$ROOT/.cargo/config.toml"
expect_file "$CFG"

# ---------------------------------------------------------------- the declaration
# the intermediates are shared, at a path that is the same in every worktree whatever its
# depth, and the artifacts are not: a `target-dir` here would be the shared executable again
expect_grep '^build-dir *= *"\{cargo-cache-home\}/build-shared/majordomus"' "$CFG"
if grep -qE '^ *target-dir *=' "$CFG"; then
  echo "    .cargo/config.toml names a target-dir: every worktree would share one executable"
  exit 1
fi

# ---------------------------------------------------------------- the behaviour
# Two roots at different paths that depend on one crate, built with cargo's own home pointed
# at a scratch directory, so that nothing here reads or writes the machine's real cache.
if ! command -v cargo >/dev/null 2>&1; then skip "cargo absent"; fi
W="$T/cargo-fixture"
mkdir -p "$W/dep/src" "$W/a/src" "$W/a/.cargo" "$W/b/src" "$W/b/.cargo" "$W/home"
printf '[package]\nname = "dep"\nversion = "0.1.0"\nedition = "2021"\n' > "$W/dep/Cargo.toml"
printf 'pub fn n() -> u32 { 7 }\n' > "$W/dep/src/lib.rs"
for r in a b; do
  printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\ndep = { path = "../dep" }\n' "$r" > "$W/$r/Cargo.toml"
  printf 'fn main() { println!("{}", dep::n()); }\n' > "$W/$r/src/main.rs"
  cp "$CFG" "$W/$r/.cargo/config.toml"
done
cb() { # root
  ( cd "$W/$1" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_BUILD_DIR CARGO_HOME="$W/home" \
      RUSTC_WRAPPER="" CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=never \
      cargo build --offline 2>&1 )
}
out="$(cb a)" || { echo "    the first build failed:"; printf '%s\n' "$out" | sed 's/^/      /'; skip "cargo could not build a fixture here"; }
printf '%s\n' "$out" | grep -q 'Compiling dep' || { echo "    the first build did not compile dep:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
LAST_OUT="$(cb b)" || { echo "    the second build failed:"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
# the second root reuses the shared dependency instead of compiling it again
expect_no_grep 'Compiling dep'
expect_grep 'Compiling b '
# and each root still has its own finished executable, which is what a target-dir is for
[ -x "$W/a/target/debug/a" ] || { echo "    root a has no executable of its own"; exit 1; }
[ -x "$W/b/target/debug/b" ] || { echo "    root b has no executable of its own"; exit 1; }
[ -d "$W/home/build-shared/majordomus" ] || { echo "    nothing landed in the shared build directory"; exit 1; }

# ---------------------------------------------------------------- the bound
# The reaper measures the shared directory, bounds it and reduces it — and acts on it only
# where the checkout declares one, so a fixture with no declaration has nothing to lose.
REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"
FIX="$T/fix"; SHARED="$T/shared-build"
mkdir -p "$FIX/.cargo" "$SHARED/debug"
(
  cd "$FIX"
  git init -q . && git config user.email t@example.com && git config user.name Test
  echo x > f && git add -A && git commit -qm base
  git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be built"; exit 2; }
printf '[build]\nbuild-dir = "%s"\n' "$SHARED" > "$FIX/.cargo/config.toml"
printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' > "$SHARED/CACHEDIR.TAG"
dd if=/dev/zero of="$SHARED/debug/blob" bs=1024 count=2048 2>/dev/null

# the process table is the reaper's liveness input, so it is the one thing stubbed
mkdir -p "$T/stub"
printf '#!/bin/sh\necho "/bin/sleep 1"\n' > "$T/stub/ps"; chmod +x "$T/stub/ps"
run() { MJ_ROOT="$FIX" PATH="$T/stub:$PATH" MAJORDOMUS_SHARED_BUILD_CAP_MB="$1" "$REAPER" "${@:2}" 2>&1; }

# within its bound: kept, and said so
rc=0; LAST_OUT="$(run 64 --targets)" || rc=$?
[ "$rc" = 0 ] || { echo "    a shared directory within its bound was reported as work (exit $rc)"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'keep — shared by every worktree, within its'

# over its bound with nobody building: a candidate, and the sweep says it would reclaim it
rc=0; LAST_OUT="$(run 1 --targets)" || rc=$?
[ "$rc" = 10 ] || { echo "    an oversized shared directory was not reported (exit $rc)"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'would reclaim — shared, over its'
[ -d "$SHARED" ] || { echo "    the report removed the directory it only measures"; exit 1; }

# over its bound while a build runs: kept, because removing it would fail that build
printf '#!/bin/sh\necho "/Users/x/.cargo/bin/cargo build"\n' > "$T/stub/ps"
rc=0; LAST_OUT="$(run 1 --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc with a build running"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'a cargo or rustc process is running'
[ -d "$SHARED" ] || { echo "    the shared directory was removed under a running build"; exit 1; }

# over its bound and idle: reclaimed, as a whole, and the report says what it freed
printf '#!/bin/sh\necho "/bin/sleep 1"\n' > "$T/stub/ps"
rc=0; LAST_OUT="$(run 1 --targets --reclaim)" || rc=$?
[ "$rc" = 0 ] || { echo "    --reclaim exited $rc"; printf '%s\n' "$LAST_OUT"; exit 1; }
expect_grep 'reclaimed'
[ ! -d "$SHARED" ] || { echo "    --reclaim left the oversized shared directory"; exit 1; }

# no declaration, no action: the same directory, the same bound, a checkout that names none
mkdir -p "$SHARED/debug"
printf 'Signature: 8a477f597d28d172789f06886806bc55\n' > "$SHARED/CACHEDIR.TAG"
dd if=/dev/zero of="$SHARED/debug/blob" bs=1024 count=2048 2>/dev/null
rm -f "$FIX/.cargo/config.toml"
rc=0; LAST_OUT="$(run 1 --targets --reclaim)" || rc=$?
expect_no_grep 'shared by every worktree|shared, over its|cargo or rustc process'
[ -d "$SHARED" ] || { echo "    the reaper removed a directory no checkout declared"; exit 1; }
