# majordomus-covers: none
# Disk, the fifth self-accumulating quantity: what the reaper refuses to reclaim, and what a
# build refuses to start without.
#
# On 2026-09-12 this machine reached zero bytes free on a 926GiB volume and every session on
# it died in the same minute — not gracefully, but with the harness unable to write its own
# task files, which reads to each session like broken tooling rather than like a full disk.
# ~145GB of it was `apps/majordomus-cli/target` in worktrees whose branches were already
# merged; one had landed hours earlier and still held 32GiB. Nothing measured it and nothing
# was allowed to reclaim it (ADR 0055, project.accumulation-is-measured v2).
#
# The reaper that answers it removes `target/` and never a worktree, which is what makes a
# false positive cost a rebuild instead of a byte of source. So what this case proves is not
# that it reclaims — it is what it refuses:
#
#   - a merged worktree somebody is working in, caught by the command line and, separately,
#     by the working directory alone. This is the trap that nearly caught a person doing it
#     by hand the same morning: a session that has not committed yet leaves its tip exactly
#     where the merge left it, so `merge-base` calls the branch fully merged while the work
#     is still in flight.
#   - a merged worktree with uncommitted or untracked files
#   - a branch that is not an ancestor of origin/master
#   - a build directory symlinked out of the worktree, which is how worktrees here have
#     shared one
#   - a directory carrying no mark of cargo's
#   - everything, when liveness cannot be measured at all
#
# and that the bound on the other side refuses to start a build below the floor, while a
# bound that cannot measure its own input lets the build run rather than stopping all work.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"
[ -x "$REAPER" ] || { echo "    scripts/reap-orphans is not executable"; exit 1; }

# ---------------------------------------------------------------- a fixture repository
# Its own repository, its own worktrees: nothing here touches the checkout the suite runs in.
FIX="$T/fix"
WTS="$T/fix-wt"
mkdir -p "$FIX" "$WTS"
(
  cd "$FIX"
  git init -q .
  git config user.email t@example.com
  git config user.name Test
  mkdir -p apps/majordomus-cli/src
  echo 'fn main(){}' > apps/majordomus-cli/src/main.rs
  # build output is ignored here as it is in the real repository, so that "idle" means what
  # it means there: nothing *authored* uncommitted, with target/ not counting as work.
  # Without the trailing slash on purpose: `target/` does not ignore a *symlink* named
  # target, and the symlinked worktree below would then be kept as dirty before the
  # ownership test it exists to exercise was ever reached.
  echo 'target' > .gitignore
  git add -A && git commit -qm base
  # what "merged" is measured against
  git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be built"; exit 2; }
# the tip everything is measured against, kept by value: the fixture's own default branch
# may be `main` or `master` depending on the git that runs the suite
BASE_SHA="$(git -C "$FIX" rev-parse HEAD)"

# A build directory, with cargo's own mark on it and a file that proves whether it was
# removed. Not a real build: what is under test is which directories the sweep selects.
mkbuild() {
  mkdir -p "$1/apps/majordomus-cli/target/debug"
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' \
    > "$1/apps/majordomus-cli/target/CACHEDIR.TAG"
  echo build-output > "$1/apps/majordomus-cli/target/MARKER"
}

addwt() { git -C "$FIX" worktree add -q -b "$1" "$WTS/$1" >/dev/null 2>&1 || return 1; mkbuild "$WTS/$1"; }

for w in idle dirty live-by-command live-by-cwd ahead symlinked unmarked; do
  addwt "$w" || { echo "    could not add the $w worktree"; exit 2; }
done

# ahead: a commit origin/master does not have
(
  cd "$WTS/ahead" && echo more > more.txt && git add more.txt && git commit -qm ahead
) >/dev/null

# dirty: authored work that was never committed. Merged-ness says nothing about it.
echo 'unfinished' > "$WTS/dirty/draft.rs"

# symlinked: the build directory is somebody else's bytes under this worktree's name
mkdir -p "$T/shared-target"
echo shared > "$T/shared-target/MARKER"
rm -rf "$WTS/symlinked/apps/majordomus-cli/target"
ln -s "$T/shared-target" "$WTS/symlinked/apps/majordomus-cli/target"

# unmarked: a directory at the right path carrying nothing that says cargo wrote it
rm -f "$WTS/unmarked/apps/majordomus-cli/target/CACHEDIR.TAG"
rm -rf "$WTS/unmarked/apps/majordomus-cli/target/debug"

# live: one process that names the path on its command line, one that only stands in it
mkfifo "$T/never" 2>/dev/null || true
tail -f "$WTS/live-by-command/apps/majordomus-cli/target/MARKER" >/dev/null 2>&1 &
LIVE_CMD=$!
( cd "$WTS/live-by-cwd" && exec tail -f apps/majordomus-cli/target/MARKER ) >/dev/null 2>&1 &
LIVE_CWD=$!
trap 'kill "$LIVE_CMD" "$LIVE_CWD" 2>/dev/null || true' EXIT
sleep 1

# ---------------------------------------------------------------- the report
rc=0
MJ_ROOT="$FIX" "$REAPER" --targets > "$T/report.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/report.txt")"
[ "$rc" = 10 ] || { echo "    the report exited $rc, not 10 (candidates found):"; sed 's/^/      /' "$T/report.txt"; exit 1; }

expect_grep 'idle.*would reclaim'
expect_grep 'dirty.*keep — 1 uncommitted or untracked'
expect_grep 'live-by-command.*keep — merged, but somebody is working in it'
expect_grep 'live-by-cwd.*keep — merged, but somebody is working in it'
expect_grep 'ahead.*keep — .* is not an ancestor of origin/master'
expect_grep 'symlinked.*keep — that build directory is not this worktree'
expect_grep 'unmarked.*skip — carries no mark of cargo'
# the measurement that decided each verdict is printed beside it, including for the ones
# nothing happens to (project.destructive-sweeps-fail-closed)
expect_grep 'idle +[0-9]+(\.[0-9])?(MB|GB)'
expect_grep 'skipped 1 for want of a readable input'
# and a report changes nothing
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# ---------------------------------------------------------------- it refuses as a whole
# A predicate that cannot be measured is false. Liveness is the one where the refusal is
# total: a reaper that cannot tell the living from the dead reclaims nothing, not everything.
mkdir -p "$T/stub"
printf '#!/bin/sh\nexit 0\n' > "$T/stub/ps"
chmod +x "$T/stub/ps"
rc=0
PATH="$T/stub:$PATH" MJ_ROOT="$FIX" "$REAPER" --targets --reclaim > "$T/blind.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/blind.txt")"
[ "$rc" = 12 ] || { echo "    a sweep that cannot read ps exited $rc, not 12:"; sed 's/^/      /' "$T/blind.txt"; exit 1; }
expect_grep 'liveness could not be measured'
expect_grep 'nothing is reclaimed'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"
rm "$T/stub/ps"

# the same when the working-directory reading is the one that is blind
printf '#!/bin/sh\nexit 0\n' > "$T/stub/lsof"
chmod +x "$T/stub/lsof"
rc=0
PATH="$T/stub:$PATH" MJ_ROOT="$FIX" "$REAPER" --targets --reclaim > "$T/blind2.txt" 2>&1 || rc=$?
[ "$rc" = 12 ] || { echo "    a sweep that cannot read working directories exited $rc, not 12"; sed 's/^/      /' "$T/blind2.txt"; exit 1; }
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"
rm "$T/stub/lsof"

# and when merged-ness itself is undecidable
git -C "$FIX" update-ref -d refs/remotes/origin/master
rc=0
MJ_ROOT="$FIX" "$REAPER" --targets --reclaim > "$T/nobase.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/nobase.txt")"
[ "$rc" = 12 ] || { echo "    a sweep with no origin/master exited $rc, not 12"; exit 1; }
expect_grep 'merged-ness is undecidable'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"
git -C "$FIX" update-ref refs/remotes/origin/master "$BASE_SHA"

# ---------------------------------------------------------------- and then it reclaims
rc=0
MJ_ROOT="$FIX" "$REAPER" --targets --reclaim > "$T/reclaim.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/reclaim.txt")"
[ "$rc" = 0 ] || { echo "    the sweep exited $rc, not 0:"; sed 's/^/      /' "$T/reclaim.txt"; exit 1; }
expect_grep 'idle.*reclaimed'
expect_grep 'reclaimed 1'

[ -e "$WTS/idle/apps/majordomus-cli/target" ] && { echo "    the idle worktree's build output survived"; exit 1; }
# it removed build output and nothing else: the worktree, its sources and its git state stay
expect_file "$WTS/idle/apps/majordomus-cli/src/main.rs"
[ -d "$WTS/idle" ] || { echo "    it removed a worktree, which it must never do"; exit 1; }
git -C "$WTS/idle" rev-parse HEAD >/dev/null 2>&1 || { echo "    the idle worktree is no longer a worktree"; exit 1; }

# every refusal above is still a refusal after a run that acted
expect_file "$WTS/dirty/apps/majordomus-cli/target/MARKER"
expect_file "$WTS/dirty/draft.rs"
expect_file "$WTS/live-by-command/apps/majordomus-cli/target/MARKER"
expect_file "$WTS/live-by-cwd/apps/majordomus-cli/target/MARKER"
expect_file "$WTS/ahead/apps/majordomus-cli/target/MARKER"
expect_file "$T/shared-target/MARKER"
[ -L "$WTS/symlinked/apps/majordomus-cli/target" ] || { echo "    it removed the symlink to a shared build directory"; exit 1; }

# ---------------------------------------------------------------- the two consents
# `--kill` ends servers; it does not delete a hundred gigabytes of build output on the way.
mkbuild "$WTS/idle"
rc=0
MJ_ROOT="$FIX" "$REAPER" --servers --kill --min-age 0s > "$T/servers.txt" 2>&1 || rc=$?
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# ---------------------------------------------------------------- the bound
# shellcheck source=../../lib/rust_bin.sh
. "$ROOT/lib/rust_bin.sh"

# the floor is a measured number, and the measurement is written beside it
expect_grep 'MJ_MIN_FREE_MB_DEFAULT=[0-9]+' "$ROOT/lib/rust_bin.sh"
expect_grep 'measured' "$ROOT/lib/rust_bin.sh"

# below the floor a build does not start, and the refusal says what to do instead
rc=0
out="$(MAJORDOMUS_MIN_FREE_MB=99999999 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 1 ] || { echo "    a build below the floor was allowed to start (exit $rc): $out"; exit 1; }
expect_grep 'refusing to start this build'
expect_grep 'floor [0-9]+MB'
expect_grep 'reap-orphans --reclaim'
expect_grep 'MAJORDOMUS_MIN_FREE_MB=0'

# above it, nothing is said and nothing is stopped
rc=0
out="$(MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    a build with room refused to start: $out"; exit 1; }
[ -z "$out" ] || { echo "    the bound is noisy when it has nothing to say: $out"; exit 1; }

# and the override lifts it entirely
rc=0
out="$(MAJORDOMUS_MIN_FREE_MB=0 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    MAJORDOMUS_MIN_FREE_MB=0 did not lift the bound"; exit 1; }

# a bound that cannot measure its own input says so and lets the build run. This is the
# deliberate opposite of the sweep above: refusing every build on a machine whose df is
# unreadable stops all work to prevent a hypothetical.
printf '#!/bin/sh\nexit 0\n' > "$T/stub/df"
chmod +x "$T/stub/df"
rc=0
out="$(PATH="$T/stub:$PATH" MAJORDOMUS_MIN_FREE_MB=99999999 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 0 ] || { echo "    an unmeasurable df stopped a build (exit $rc): $out"; exit 1; }
expect_grep 'could not be measured'
expect_grep 'starting anyway'
rm "$T/stub/df"

# ---------------------------------------------------------------- at a real call site
# The launcher builds when the executable it resolves is missing or stale. Below the floor it
# must refuse before cargo starts rather than after the volume is full. The release profile is
# used because a suite that has built the debug one would otherwise have nothing to build.
if [ ! -x "$ROOT/apps/majordomus-cli/target/release/majordomus" ] && [ -z "${CARGO_TARGET_DIR:-}" ]; then
  rc=0
  out="$(env -u MAJORDOMUS_BIN MAJORDOMUS_BUILD_PROFILE=release MAJORDOMUS_MIN_FREE_MB=99999999 \
    "$ROOT/bin/majordomus-cli" --version 2>&1)" || rc=$?
  LAST_OUT="$out"
  [ "$rc" = 12 ] || { echo "    the launcher started a build below the floor (exit $rc): $out"; exit 1; }
  expect_grep 'refusing to start'
else
  echo "    (a release executable is already built: the launcher's refusal is not exercised)"
fi

echo "    ok: the reaper refuses six ways and removes only build output; a build below the floor does not start"
