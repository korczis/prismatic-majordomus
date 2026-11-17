# majordomus-covers: none
# The CI build cache carries cargo's build directory pruned to the dependencies' units (I4221).
# setup-rust handed ~/.cargo/build-shared to rust-cache as a cache-directories entry, which is
# archived as it lies: the coverage job's carried every instrumented test executable of the
# crate, its save ran the runner out of disk (runs 41107049778, 41254167755), and no coverage
# cache was ever stored. A workspace target is pruned before the save (rust-cache's
# cleanTargetDir), so the build directory is handed over as one.
#
#   the build directory is never a cache-directories entry;
#   it is a second target of the apps/majordomus-cli workspace, from the step that asks cargo;
#   that step takes cargo metadata's build_directory and makes it relative to the workspace,
#   because rust-cache joins a target onto the workspace root;
#   the check can fail: the action as it was before this case is refused.
. "$ROOT/test/lib.sh"

ACTION="$ROOT/.github/actions/setup-rust/action.yml"

# pruned <action.yml>: the build directory reaches rust-cache as a target, never as it lies
pruned() {
  expect_no_grep 'cache-directories:.*build-shared' "$1" || return 1
  expect_grep '^ +apps/majordomus-cli -> \$\{\{ steps\.build-dir\.outputs\.relative \}\}$' "$1" || return 1
  expect_grep '^ +- id: build-dir$' "$1" || return 1
  expect_grep 'cargo metadata .*--no-deps .*build_directory' "$1" || return 1
  expect_grep 'os\.path\.relpath\(.*apps/majordomus-cli' "$1" || return 1
}

pruned "$ACTION" || exit 1

# the action before I4221's fix: build-shared as a raw cache directory, no workspace target
sed -e 's#^\( *\)apps/majordomus-cli -> .*#\1cache-directories: ~/.cargo/build-shared#' \
  "$ACTION" > before.yml
if pruned before.yml >/dev/null; then
  echo "    the check accepted an action that saves the build directory as it lies"
  exit 1
fi
echo "ok: the build cache saves only the dependencies' units"
