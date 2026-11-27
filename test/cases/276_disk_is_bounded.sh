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
#
# On 2026-10-10 the same machine and a second one reached 99% with 270GB and 48GB of build
# output the reaper would not touch: every branch was pushed and waiting to land, so none was
# an ancestor of origin/master, and nothing ran the reaper anyway. So two more things are held:
# a pushed, clean, unattended worktree's build output is reclaimed like a merged one's (an
# unpushed one is still kept), and a build that starts below the reclaim mark runs the sweep
# itself before the floor decides.
#
# On 2026-10-28 (I4346) the reaper removed a live worktree's build output anyway: liveness was a
# substring match against `ps` and working directories, and a session working through absolute
# paths showed in neither. Liveness is now one typed capability, `majordomus worktree liveness`,
# which both the reaper and the build host's disk-guard ask. So this case also holds that the
# guard keeps a hard-tier build directory idle for longer than its window when the worktree's
# HEAD moved within it, and removes no build directory at all when the executable its service
# names is not there. Every fixture worktree's git state is aged past the window first: a tree
# made a second ago is, to the predicate, somebody's work in progress. The rule is
# project.reclaim-only-what-you-own v2.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
# the reaper asks this executable, here and in the fixture's own copy of it below
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"
[ -x "$REAPER" ] || { echo "    scripts/reap-orphans is not executable"; exit 1; }

# A worktree's index, HEAD and HEAD reflog, aged past any idle window.
age_git() {
  local gd
  gd="$(git -C "$1" rev-parse --absolute-git-dir)" || return 1
  touch -t 202001010000 "$gd/HEAD"
  [ ! -e "$gd/index" ] || touch -t 202001010000 "$gd/index"
  [ ! -e "$gd/logs/HEAD" ] || touch -t 202001010000 "$gd/logs/HEAD"
}

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

for w in idle dirty live-by-command live-by-cwd ahead pushed symlinked unmarked; do
  addwt "$w" || { echo "    could not add the $w worktree"; exit 2; }
done

# ahead: a commit origin/master does not have
(
  cd "$WTS/ahead" && echo more > more.txt && git add more.txt && git commit -qm ahead
) >/dev/null

# pushed: a commit origin/master does not have, which IS on a remote — a branch waiting to land
(
  cd "$WTS/pushed" && echo queued > queued.txt && git add queued.txt && git commit -qm pushed \
    && git update-ref refs/remotes/origin/pushed HEAD
) >/dev/null

for w in idle dirty live-by-command live-by-cwd ahead pushed symlinked unmarked; do
  age_git "$WTS/$w" || { echo "    could not age the $w worktree's git state"; exit 2; }
done

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
expect_grep 'live-by-command.*keep — merged, but somebody is working in it: process_arguments'
expect_grep 'live-by-cwd.*keep — merged, but somebody is working in it: process_directory'
expect_grep 'ahead.*keep — .* is neither on origin/master nor on any remote'
expect_grep 'pushed.*would reclaim — published on origin/pushed, nobody in it'
expect_grep 'idle.*would reclaim — merged, nobody in it'
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
expect_grep 'pushed.*reclaimed'
expect_grep 'reclaimed 2'

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
[ -e "$WTS/pushed/apps/majordomus-cli/target" ] && { echo "    the pushed worktree's build output survived"; exit 1; }
expect_file "$WTS/pushed/queued.txt"
expect_file "$T/shared-target/MARKER"
[ -L "$WTS/symlinked/apps/majordomus-cli/target" ] || { echo "    it removed the symlink to a shared build directory"; exit 1; }

# ---------------------------------------------------------------- the two consents
# `--kill` ends servers; it does not delete a hundred gigabytes of build output on the way.
#
# `--min-age 999d`, and never `0s`, because the server sweep is the one part of the reaper
# MJ_ROOT does not confine. It asks `ps -eo pid=,ppid=,...` across the whole machine and
# takes every `majordomus serve|mcp` whose parent is init — the fixture cannot contain it,
# because a server's identity is its process, not its path. With `0s` every live server on
# the machine qualifies, and a draft of case 318 ended a session's server on port 8741
# that way: correct by the reaper's predicate, ppid 1 and no client attached, and still
# somebody's. A case must not act on the machine it measures; case 318 carries the same bound.
#
# The assertion is unchanged by the bound: what is proven here is that `--kill` leaves
# build output alone, and a sweep that ends no server proves it at least as well as one
# that ends somebody's.
mkbuild "$WTS/idle"
rc=0
MJ_ROOT="$FIX" "$REAPER" --servers --kill --min-age 999d > "$T/servers.txt" 2>&1 || rc=$?
# the sweep ran and exited cleanly: a reaper that died on its options would leave the
# marker too, so the marker alone proves nothing without the exit code and the output
[ "$rc" = 0 ] || { echo "    --servers --kill exited $rc"; cat "$T/servers.txt"; exit 1; }
LAST_OUT="$(cat "$T/servers.txt")"
expect_no_grep 'would reclaim'
expect_no_grep '^WORKTREE +SIZE'
expect_no_grep 'reap-orphans: build output'
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# ---------------------------------------------------------------- the bound
# shellcheck source=../../lib/rust_bin.sh
. "$ROOT/lib/rust_bin.sh"

# the floor is a measured number, and the measurement is written beside it
expect_grep 'MJ_MIN_FREE_MB_DEFAULT=[0-9]+' "$ROOT/lib/rust_bin.sh"
expect_grep 'measured' "$ROOT/lib/rust_bin.sh"

# Every call here has MAJORDOMUS_INCREMENTAL_IDLE_MIN=0: the floor's check first sweeps the
# idle incremental sessions of the real build directory (I2713), which a case must not touch;
# that sweep is proved against a fixture in test/cases/3080_idle_incremental_sessions_are_pruned.sh.
# below the floor a build does not start, and the refusal says what to do instead
rc=0
out="$(MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_MIN_FREE_MB=99999999 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 1 ] || { echo "    a build below the floor was allowed to start (exit $rc): $out"; exit 1; }
expect_grep 'refusing to start this build'
expect_grep 'floor [0-9]+MB'
expect_grep 'reap-orphans --reclaim'
expect_grep 'MAJORDOMUS_MIN_FREE_MB=0'

# above it, nothing is said and nothing is stopped
rc=0
out="$(MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    a build with room refused to start: $out"; exit 1; }
[ -z "$out" ] || { echo "    the bound is noisy when it has nothing to say: $out"; exit 1; }

# and the override lifts it entirely
rc=0
out="$(MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_MIN_FREE_MB=0 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    MAJORDOMUS_MIN_FREE_MB=0 did not lift the bound"; exit 1; }

# a bound that cannot measure its own input says so and lets the build run. This is the
# deliberate opposite of the sweep above: refusing every build on a machine whose df is
# unreadable stops all work to prevent a hypothetical.
printf '#!/bin/sh\nexit 0\n' > "$T/stub/df"
chmod +x "$T/stub/df"
rc=0
out="$(PATH="$T/stub:$PATH" MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_MIN_FREE_MB=99999999 mj_rust_space_check "$ROOT" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 0 ] || { echo "    an unmeasurable df stopped a build (exit $rc): $out"; exit 1; }
expect_grep 'could not be measured'
expect_grep 'starting anyway'
rm "$T/stub/df"

# ---------------------------------------------------------------- the build reclaims first
# Below the reclaim mark a build runs the classified sweep itself, before the floor decides, so
# nobody has to remember the command. Against the fixture, never this machine: the fixture
# carries its own copy of the reaper, which the space check runs as the repository's.
mkdir -p "$FIX/scripts" "$FIX/lib"
cp "$ROOT/scripts/reap-orphans" "$FIX/scripts/"; cp "$ROOT"/lib/*.sh "$FIX/lib/"
mkbuild "$WTS/idle"
rc=0
out="$(XDG_CACHE_HOME="$T/cache" CARGO_TARGET_DIR="$T/no-target" MAJORDOMUS_RECLAIM_BELOW_MB=99999999 \
  MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || rc=$?
LAST_OUT="$out"
[ "$rc" = 0 ] || { echo "    a build above the floor was refused after reclaiming (exit $rc): $out"; exit 1; }
expect_grep 'reclaiming unattended build output first'
expect_grep 'reap-orphans: build output — reclaimed 1'
[ -e "$WTS/idle/apps/majordomus-cli/target" ] && { echo "    the build started below the mark did not reclaim"; exit 1; }
expect_file "$WTS/ahead/apps/majordomus-cli/target/MARKER"
expect_file "$WTS/live-by-cwd/apps/majordomus-cli/target/MARKER"

# once per interval, machine-wide: a second build a moment later does not sweep again
mkbuild "$WTS/idle"
out="$(XDG_CACHE_HOME="$T/cache" CARGO_TARGET_DIR="$T/no-target" MAJORDOMUS_RECLAIM_BELOW_MB=99999999 \
  MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || true
[ -z "$out" ] || { echo "    a second build inside the interval swept again: $out"; exit 1; }
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# and the mark at 0 turns it off: the suite runs with it off (test/run.sh)
rm -rf "$T/cache"
out="$(XDG_CACHE_HOME="$T/cache" CARGO_TARGET_DIR="$T/no-target" MAJORDOMUS_RECLAIM_BELOW_MB=0 \
  MAJORDOMUS_MIN_FREE_MB=1 mj_rust_space_check "$FIX" "this build" 2>&1)" || true
[ -z "$out" ] || { echo "    MAJORDOMUS_RECLAIM_BELOW_MB=0 still swept: $out"; exit 1; }
expect_file "$WTS/idle/apps/majordomus-cli/target/MARKER"

# ---------------------------------------------------------------- at a real call site
# Amended for ADR 0224 (I3957): the launcher no longer builds at all, so it can no longer start
# a build below the floor — it refuses a missing or stale executable with exit 12 naming the
# explicit build, `just build`, and that recipe asks the floor first (.just/build.just, held by
# scripts/ci/backlog-check). What is asserted here is the half that moved: below the floor, from
# a person or from a hook, nothing compiles and the refusal names the build. The release
# profile is used because a suite that has built the debug one would otherwise have nothing
# missing to resolve.
if [ ! -x "$ROOT/apps/majordomus-cli/target/release/majordomus" ] && [ -z "${CARGO_TARGET_DIR:-}" ]; then
  for hook in 0 1; do
    rc=0
    out="$(env -u MAJORDOMUS_BIN -u MAJORDOMUS_HOOK MAJORDOMUS_HOOK="$hook" MAJORDOMUS_RECLAIM_BELOW_MB=0 MAJORDOMUS_INCREMENTAL_IDLE_MIN=0 MAJORDOMUS_BUILD_PROFILE=release MAJORDOMUS_MIN_FREE_MB=99999999 \
      "$ROOT/bin/majordomus-cli" --version 2>&1)" || rc=$?
    LAST_OUT="$out"
    case "$rc" in
      # a release of this tree's generation may answer; nothing else does
      0) expect_grep 'answering through the installed release .* whose crate this tree carries unchanged' ;;
      12) expect_grep 'run: just build' ;;
      *) echo "    the launcher (hook=$hook) neither answered through a release of this generation nor refused (exit $rc): $out"; exit 1 ;;
    esac
    if printf '%s\n' "$out" | grep -q 'Compiling'; then
      echo "    the launcher (hook=$hook) compiled below the floor: $out"; exit 1
    fi
  done
  grep -q 'mj_rust_space_check' "$ROOT/.just/build.just" \
    || { echo "    just build no longer asks the floor before it builds"; exit 1; }
else
  echo "    (a release executable is already built: the launcher's refusal is not exercised)"
fi

# ---------------------------------------------------------------- the build host's guard
# disk-guard, sourced (it then only defines its functions), over a home of the fixture's own:
# a worktree under its dev/ whose build output has been idle for years while the worktree's
# HEAD moved a moment ago, and a build directory under .cache that nothing names. The shims
# stand in for what a Mac lacks: GNU's `rm --one-file-system`, `du -B` and `timeout`.
GUARD="$ROOT/apps/majordomus-cli/assets/mesh-cache-provider/disk-guard"
expect_file "$GUARD"
LIVE_PATH="$(sed -n 's/^Environment=LIVENESS_BIN=//p' "$GUARD.service")"
[ -n "$LIVE_PATH" ] || { echo "    disk-guard.service names no LIVENESS_BIN for the guard to ask"; exit 1; }
GH="$T/guard-home"
mkdir -p "$GH/dev" "$GH/.cache/spare/target/debug"
git -C "$FIX" worktree add -q -b guard-busy "$GH/dev/busy" >/dev/null 2>&1 \
  || { echo "    could not add the guard's worktree"; exit 2; }
mkdir -p "$GH/dev/busy/target/debug"
for d in "$GH/dev/busy/target" "$GH/.cache/spare/target"; do
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' > "$d/CACHEDIR.TAG"
done
find "$GH" -exec touch -h -t 202001010000 {} + 2>/dev/null
age_git "$GH/dev/busy" || { echo "    could not age the guard's worktree"; exit 2; }
# the case's own directory is a repository the runner made a moment ago, and .cache/ is inside it
age_git "$T" || { echo "    could not age the case directory's git state"; exit 2; }
touch "$(git -C "$GH/dev/busy" rev-parse --absolute-git-dir)/HEAD"

guard() {  # <LIVENESS_BIN> — the hard tier's reclaim of idle cargo output, and what it said
  (
    . "$GUARD"
    set +e  # the guard runs without errexit, and its idle check counts on lsof's exit 1
    HOMES=("$GH"); RUNNER_HOMES=(); LIVENESS_BIN="$1"
    command -v timeout >/dev/null 2>&1 || timeout() { shift; "$@"; }
    rm() { command rm -rf -- "${@: -1}"; }
    size_gib() { echo 1; }
    reclaim_cargo "$HARD_IDLE_MIN"
    printf '%s\n' "${actions[@]+"${actions[@]}"}"
  ) 2>&1
}

# no executable at the path the service names: no build directory is removed, and it says so
LAST_OUT="$(guard "$T/root$LIVE_PATH")"
expect_grep 'no liveness executable'
expect_file "$GH/.cache/spare/target/CACHEDIR.TAG"
expect_file "$GH/dev/busy/target/CACHEDIR.TAG"

# with it, the directory nothing names goes and the one whose worktree moved stays
LAST_OUT="$(guard "$RB")"
[ -e "$GH/.cache/spare/target" ] && { echo "    the guard kept a build directory nothing names:"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
expect_grep 'removed cargo output idle'
expect_file "$GH/dev/busy/target/CACHEDIR.TAG"
expect_grep 'kept .*busy/target: git_activity'

echo "    ok: the reaper refuses six ways, reclaims landed and pushed build output only, a build below the mark reclaims first, below the floor a build does not start, from a person or from a hook, and the build host's guard keeps a build directory whose worktree moved within its window and removes none without the liveness executable"
