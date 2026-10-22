# majordomus-covers: none
# `worktree cleanup --remove` removes a merged worktree only when nothing about it says it is
# in use (I3832): beside the guards case 611 proves, a worktree changed within the idle window,
# one at the remote trunk's tip, one the derive lock names, one a running process names (a
# pinned build directory), and one whose build output a cargo holds the lock of are each
# refused with the reason; a worktree none of that is true of is removed. The disk-cleanup
# sweeps of 2026-10-20/21 removed trees under a verification session, a queued derive and a
# starting cargo, each of which one of these readings would have seen.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v lsof >/dev/null 2>&1 || skip "no lsof"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_DERIVE_LOCK="$T/derive.lock"; export MAJORDOMUS_DERIVE_LOCK

S="$T/sweep"; mkdir -p "$S"; cd "$S" || exit 1
git init -q --bare origin.git; git init -q repo; cd repo || exit 1
git config user.email t@t; git config user.name t
git remote add origin ../origin.git
printf 'one\n' > file.txt; printf 'target/\n' > .gitignore; git add -A && git commit -qm trunk
git push -q -u origin HEAD:master; git branch -M master
mk() {  # mk <branch> [ff]: a pushed branch merged into master, with its worktree
  git switch -q -c "$1" master; printf '%s\n' "$1" > "$1.txt"
  git add -A && git commit -qm "$1"; git push -q -u origin "$1"; git switch -q master
  if [ "${2:-}" = ff ]; then git merge -q --ff-only "$1"; else git merge -q --no-ff -m "merge $1" "$1"; fi
  git push -q origin master
  "$RB" worktree create "$1" >/dev/null 2>&1 || { echo "    could not create the worktree of $1"; exit 1; }
}
mk recent; mk named-by-lock; mk pinned; mk build-held; mk spare; mk at-tip ff

# every tree aged two days, past the one-day idle window, except the one that is to read as recent
age() { find "$1" -path "$1/.git" -prune -o -exec touch -t "$(date -v-2d +%Y%m%d%H%M 2>/dev/null || date -d '-2 days' +%Y%m%d%H%M)" {} + ; }
for b in named-by-lock pinned build-held spare at-tip; do age "$("$RB" worktree path "$b")"; done
# the derive lock names named-by-lock
mkdir -p "$MAJORDOMUS_DERIVE_LOCK"; printf 'derive:named-by-lock 1\n' > "$MAJORDOMUS_DERIVE_LOCK/owner"
# a process names a path inside pinned. macOS hides the environment of its own protected
# binaries from ps, so the fixture names the path on a command line (a tail on a file in its
# build directory) rather than in a CARGO_TARGET_DIR; a cargo's environment is visible, and
# reading a pin from an environment is held by worktree::sweep's unit tests.
pin="$("$RB" worktree path pinned)/target"; mkdir -p "$pin"; : > "$pin/build.log"
age "$("$RB" worktree path pinned)"
tail -f "$pin/build.log" > /dev/null & PIN=$!
# a cargo holds the build lock of build-held (perl's flock, so no shell dependency on flock(1))
held="$("$RB" worktree path build-held)/target/debug"; mkdir -p "$held"; : > "$held/.cargo-lock"
age "$("$RB" worktree path build-held)"
# the path goes in on stdin, so no command line or environment names the worktree and the
# refusal read is the lock's, not the process listing's
printf '%s' "$held/.cargo-lock" | perl -MFcntl=:flock -e 'my $p = <STDIN>; open(my $f, "+<", $p) or die; flock($f, LOCK_EX) or die; sleep 300' & HOLD=$!
trap 'kill "$PIN" "$HOLD" 2>/dev/null; :' EXIT
sleep 1

"$RB" worktree cleanup --remove --format json > "$T/out.json" 2>"$T/out.err" \
  || { echo "    cleanup --remove exited non-zero:"; tail -3 "$T/out.err"; exit 1; }
removed() { jq -r --arg b "$1" '[.removed[] | select(. == $b)] | length' "$T/out.json"; }
reason() { jq -r --arg b "$1" '.refused[] | select(.branch == $b) | .reason' "$T/out.json"; }
expect_reason() {  # expect_reason <branch> <pattern>
  r="$(reason "$1")"
  printf '%s\n' "$r" | grep -q "$2" || { echo "    $1 was not refused for '$2': ${r:-removed}"; cat "$T/out.json"; exit 1; }
  [ -d "$("$RB" worktree path "$1")" ] || { echo "    $1 was refused and removed anyway"; exit 1; }
}
expect_reason recent 'changed within'
expect_reason at-tip "tip"
expect_reason named-by-lock 'derive lock names'
expect_reason pinned 'running process names it'
expect_reason build-held 'a build holds'
[ "$(removed spare)" = 1 ] || { echo "    the spare worktree was not removed"; cat "$T/out.json"; exit 1; }
[ -d "$("$RB" worktree path spare)" ] && { echo "    spare was reported removed and is still there"; exit 1; }
echo "    recent, tip, derive-locked, pinned and build-held worktrees are kept with the reason; the spare one goes"

# --- a mesh declared and unreadable: the claims cannot say nobody works there, so it stays
mk spare-two; age "$("$RB" worktree path spare-two)"
mkdir -p .ai/repo/mesh; printf 'enabled: true\n' > .ai/repo/mesh/majordomus.yaml
"$RB" worktree cleanup --remove --format json > "$T/out.json" 2>"$T/out.err" \
  || { echo "    cleanup --remove exited non-zero:"; tail -3 "$T/out.err"; exit 1; }
expect_reason spare-two 'cannot read the mesh claims'
echo "    with the mesh declared and no server to read its claims, a spare worktree is kept and says why"
