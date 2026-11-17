# claims: none
# A local build refuses to start beside five cargo builds already running on the machine
# (I3825, clause 7 of rule project.the-cooperation-model-is-the-owners-word). On 2026-11-16
# the 641b Mac's free space fell from 26 GiB to 6 GiB in about two hours while three cargo
# builds of three sessions and a container ran, and nothing in the tool counted them.
#
# The process table is a stub on PATH, so every scenario is the same on any machine:
#
#   1  five builds run: a sixth refuses, naming the count, the limit, each build, the
#      offload route and the override
#   2  four builds (options before the subcommand, a path with a space), plus what is not
#      another build (`cargo metadata`, a cargo whose parent is a counted cargo, the cargo
#      builds this shell runs under): it starts, and says nothing
#   3  MAJORDOMUS_MAX_BUILDERS=0 lifts the count
#   4  on CI the count is not asked
#   5  a process table that cannot be read says so, and the build starts
#   6  through mj_rust_space_check: with room on the disk the count still refuses, and the
#      disk floor's own override (MAJORDOMUS_MIN_FREE_MB=0) does not lift the count
. "$ROOT/test/lib.sh"
unset CI GITHUB_ACTIONS MAJORDOMUS_MAX_BUILDERS
# the floor's sweeps run before the count; a case never touches the machine's build directories
export MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN=0

mkdir -p "$T/stub"
cat > "$T/stub/ps" <<EOF
#!/bin/sh
[ -f "$T/ps.fail" ] && exit 1
cat "$T/ps.table"
EOF
chmod +x "$T/stub/ps"
PATH="$T/stub:$PATH"; export PATH

# shellcheck source=../../lib/rust_bin.sh
. "$ROOT/lib/rust_bin.sh"

table() {
  # pid ppid args: four builds of four sessions, two of them with options before the
  # subcommand and one whose executable's path has a space in it
  cat > "$T/ps.table" <<EOF
  101     1 /Users/x/.rustup/toolchains/1.98.1/bin/cargo test --lib instances
  102     1 cargo --manifest-path apps/majordomus-cli/Cargo.toml build
  103     1 /opt/rust/bin/cargo +1.98.1 -Z unstable-options clippy --all-targets
  104     1 /Users/x/My Tools/cargo check --message-format=json
  200     1 cargo metadata --format-version 1
  301   101 /Users/x/.rustup/toolchains/1.98.1/bin/cargo test --lib instances
  $$    900 bash test/cases/3825_a_build_is_bounded_by_the_machine.sh
  900   901 cargo test --lib
  901     1 cargo --locked build
  400     1 /usr/bin/rustc --crate-name rhai
EOF
  [ "${1:-}" = five ] && printf '  105     1 cargo --config k=v doc --no-deps\n' >> "$T/ps.table"
  return 0
}

# ---------------------------------------------------------------- 1. five builds run
table five
expect_exit 1 mj_rust_builders_check "this build" || exit 1
expect_grep 'refusing to start this build: 5 cargo builds already run on this machine, limit 5' || exit 1
for pid in 101 102 103 104 105; do expect_grep "^    $pid cargo|^    $pid /" || exit 1; done
expect_grep 'majordomus offload derive' || exit 1
expect_grep 'MAJORDOMUS_MAX_BUILDERS=0' || exit 1

# ---------------------------------------------------------------- 2. four builds, and what is not one
table
expect_exit 0 mj_rust_builders_check "this build" || exit 1
[ -z "$LAST_OUT" ] || { printf '    the count is noisy when it has nothing to say: %s\n' "$LAST_OUT"; exit 1; }
LAST_OUT="$(mj_rust_builders)"
[ "$(printf '%s\n' "$LAST_OUT" | grep -c .)" = 4 ] || { printf '    four builds were not counted as four:\n%s\n' "$LAST_OUT"; exit 1; }
# not `cargo metadata`, not a cargo under a counted cargo, not rustc, and not this shell's
# own ancestors (900 and 901), which started whatever asks the guard
expect_no_grep '^(200|301|400|900|901|'"$$"') ' || exit 1

# ---------------------------------------------------------------- 3. the override
table five
MAJORDOMUS_MAX_BUILDERS=0 expect_exit 0 mj_rust_builders_check "this build" || exit 1

# ---------------------------------------------------------------- 4. CI is not counted
CI=true expect_exit 0 mj_rust_builders_check "this build" || exit 1
GITHUB_ACTIONS=true expect_exit 0 mj_rust_builders_check "this build" || exit 1

# ---------------------------------------------------------------- 5. an unreadable process table
: > "$T/ps.fail"
expect_exit 0 mj_rust_builders_check "this build" || exit 1
expect_grep 'could not be counted; this build is starting anyway' || exit 1
rm -f "$T/ps.fail"

# ---------------------------------------------------------------- 6. through the space check
MAJORDOMUS_MIN_FREE_MB=1 expect_exit 1 mj_rust_space_check "$ROOT" "this build" || exit 1
expect_grep '5 cargo builds already run' || exit 1
MAJORDOMUS_MIN_FREE_MB=0 expect_exit 1 mj_rust_space_check "$ROOT" "this build" || exit 1
expect_grep '5 cargo builds already run' || exit 1
table
MAJORDOMUS_MIN_FREE_MB=1 expect_exit 0 mj_rust_space_check "$ROOT" "this build" || exit 1
exit 0
