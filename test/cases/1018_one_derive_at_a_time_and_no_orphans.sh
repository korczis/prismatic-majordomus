# claim: derived-data-current
# One derive at a time per machine, and a stopped derive leaves no orphans
# (project.one-derive-at-a-time).
#
# On 2026-10-09 a derive ran without the machine's lock beside a repair's derive, and stopping
# it left stage B running for an hour, reparented to pid 1. The lock had lived only in the
# sessions' own wrappers. This case drives scripts/derive's own lock through `--locked`, which
# runs a command under it without building or deriving anything. It proves each half: the
# reclaim of a dead owner's lock (announced, never silent), the wait for a live owner and for a
# priority marker that names another branch, re-entry under a wrapper that already holds the
# lock, and a TERM that takes the command's whole tree down and releases the lock.
. "$ROOT/test/lib.sh"

D="$ROOT/scripts/derive"
export MAJORDOMUS_DERIVE_LOCK="$T/machine/.derive.lock"
export MAJORDOMUS_DERIVE_PRIORITY="$T/machine/.derive.priority"
export MAJORDOMUS_DERIVE_POLL=1
mkdir -p "$T/machine"
BRANCH="$(git -C "$ROOT" symbolic-ref --quiet --short HEAD 2>/dev/null || printf 'detached')"
fail() { printf '    %s\n' "$*"; exit 1; }
lock_free() { [ ! -e "$MAJORDOMUS_DERIVE_LOCK" ] || fail "the lock was left behind: $(cat "$MAJORDOMUS_DERIVE_LOCK/owner" 2>/dev/null)"; }

# ---------------------------------------------------------------- taken and released
expect_exit 0 "$D" --locked -- sh -c 'cat "$MAJORDOMUS_DERIVE_LOCK/owner"'
expect_grep "^derive:$BRANCH [0-9]+"
lock_free
expect_exit 7 "$D" --locked -- sh -c 'exit 7'
lock_free
expect_exit 2 "$D" --locked --
expect_exit 2 "$D" --no-such-flag

# ---------------------------------------------------------------- a dead owner is reclaimed, aloud
sleep 0 & dead=$!; wait "$dead"
mkdir "$MAJORDOMUS_DERIVE_LOCK" && printf 'ghost-session %s\n' "$dead" > "$MAJORDOMUS_DERIVE_LOCK/owner"
expect_exit 0 "$D" --locked -- true
expect_grep "reclaimed .*\.derive\.lock from a dead owner: ghost-session $dead"
lock_free

# ---------------------------------------------------------------- an owner without a pid is live
# Lines the sessions write today: a hand-taken lock, and a repair loop that ends in a time.
# Neither names a pid, so neither can be checked, and neither may ever be reclaimed.
for line in 'someone taken by hand' 'd1 fix/a-lanes-own-output-is-not-a-dirty-tree repair 10:28'; do
  mkdir "$MAJORDOMUS_DERIVE_LOCK" && printf '%s\n' "$line" > "$MAJORDOMUS_DERIVE_LOCK/owner"
  MAJORDOMUS_DERIVE_WAIT_MAX=2 expect_exit 75 "$D" --locked -- true
  expect_grep "held by an owner that names no pid"
  expect_no_grep 'reclaimed'
  grep -qxF "$line" "$MAJORDOMUS_DERIVE_LOCK/owner" || fail "a lock held by \"$line\" was touched"
  rm -rf "$MAJORDOMUS_DERIVE_LOCK"
done
# a dead pid written as pid=<n> is still a dead pid
mkdir "$MAJORDOMUS_DERIVE_LOCK" && printf 'someone pid=%s branch=x\n' "$dead" > "$MAJORDOMUS_DERIVE_LOCK/owner"
expect_exit 0 "$D" --locked -- true
expect_grep "reclaimed .* from a dead owner: someone pid=$dead branch=x"
lock_free

# ---------------------------------------------------------------- only the owner releases
# The line is rewritten under the derive by somebody else: it is no longer this process's
# lock, and the exit leaves it.
expect_exit 0 "$D" --locked -- sh -c 'echo "another-holder 1" > "$MAJORDOMUS_DERIVE_LOCK/owner"'
expect_grep 'no longer names this process; left as it is'
grep -qx 'another-holder 1' "$MAJORDOMUS_DERIVE_LOCK/owner" || fail 'the exit released a lock it no longer held'
rm -rf "$MAJORDOMUS_DERIVE_LOCK"

# ---------------------------------------------------------------- a live owner is waited for
sleep 60 & live=$!
mkdir "$MAJORDOMUS_DERIVE_LOCK" && printf 'other-session %s\n' "$live" > "$MAJORDOMUS_DERIVE_LOCK/owner"
MAJORDOMUS_DERIVE_WAIT_MAX=2 expect_exit 75 "$D" --locked -- true
expect_grep "waiting: .*held by other-session $live"
expect_grep 'gave up after'
grep -qx "other-session $live" "$MAJORDOMUS_DERIVE_LOCK/owner" || fail 'a waiting derive touched a live owner'"'"'s lock'
kill "$live" 2>/dev/null || true; wait "$live" 2>/dev/null || true
rm -rf "$MAJORDOMUS_DERIVE_LOCK"

# ---------------------------------------------------------------- the priority marker
printf 'd1: #825 then #831, repair for another branch\n' > "$MAJORDOMUS_DERIVE_PRIORITY"
MAJORDOMUS_DERIVE_WAIT_MAX=2 expect_exit 75 "$D" --locked -- true
expect_grep "waiting: .*names another branch: d1: #825"
lock_free
MAJORDOMUS_DERIVE_IGNORE_PRIORITY=1 expect_exit 0 "$D" --locked -- true
# a marker naming a longer branch that merely starts with this one is another branch
printf '19: %s-two (owner priority)\n' "$BRANCH" > "$MAJORDOMUS_DERIVE_PRIORITY"
MAJORDOMUS_DERIVE_WAIT_MAX=2 expect_exit 75 "$D" --locked -- true
expect_grep "names another branch"
printf '19: %s (owner priority)\n' "$BRANCH" > "$MAJORDOMUS_DERIVE_PRIORITY"
expect_exit 0 "$D" --locked -- true
rm -f "$MAJORDOMUS_DERIVE_PRIORITY"

# ---------------------------------------------------------------- re-entrant under a wrapper
# The wrapper the sessions wrote before this existed: mkdir the lock, write "<tag> $$", run
# the derive. Its derive must see its own caller, not a stranger, and leave the lock to it.
expect_exit 0 bash -c '
  mkdir "$MAJORDOMUS_DERIVE_LOCK" && echo "wrapper $$" > "$MAJORDOMUS_DERIVE_LOCK/owner"
  "$1" --locked -- true || exit 1
  grep -qx "wrapper $$" "$MAJORDOMUS_DERIVE_LOCK/owner" || { echo "the wrapper lost its lock"; exit 1; }
  rm -f "$MAJORDOMUS_DERIVE_LOCK/owner"; rmdir "$MAJORDOMUS_DERIVE_LOCK"' _ "$D"
expect_grep "held by this process's own caller \(wrapper [0-9]+\)"
lock_free

# ---------------------------------------------------------------- a stop takes the tree down
# A grandchild that would outlive a shell-only kill, as stage B's generate-site-data did.
"$D" --locked -- bash -c 'sleep 300 & echo $! > "$0"; wait' "$T/grandchild" 2> "$T/stop.err" &
derive=$!
for _ in $(seq 1 50); do [ -s "$T/grandchild" ] && break; sleep 0.1; done
[ -s "$T/grandchild" ] || fail 'the command under the lock never started'
gc="$(cat "$T/grandchild")"
kill -TERM "$derive"
rc=0; wait "$derive" || rc=$?
[ "$rc" = 143 ] || fail "a derive stopped by TERM exited $rc, not 143"
for _ in $(seq 1 30); do ps -p "$gc" >/dev/null 2>&1 || break; sleep 0.1; done
if ps -p "$gc" >/dev/null 2>&1; then kill -KILL "$gc"; fail "pid $gc outlived the derive that started it"; fi
grep -q 'stopped by SIGTERM; taking the whole process tree down' "$T/stop.err" \
  || fail "the stop was not announced: $(cat "$T/stop.err")"
lock_free

# ---------------------------------------------------------------- only this checkout's orphans
# The reaper's predicate, over a process table this case writes: an orphan is parent pid 1 AND
# this checkout's own absolute script path. Another worktree's derive, a sibling path that
# merely shares a prefix, the same path nested under another root, and this checkout's derive
# with a live parent are each a process the reaper must never touch.
OTHER="$(dirname "$ROOT")/$(basename "$ROOT")-wt/fix/another/scripts/generate-site-data"
cat > "$T/ps" <<PS
4101 1 bash $ROOT/scripts/generate-site-data
4102 1 bash $ROOT/scripts/derive
4103 1 bash $OTHER
4104 1 bash ${ROOT}2/scripts/generate-site-data
4105 1 bash /elsewhere$ROOT/scripts/generate-site-data
4106 777 bash $ROOT/scripts/generate-site-data
4107 1 bash $ROOT/scripts/derive-check
4108 1 vim $ROOT/scripts/derive.orig
PS
MAJORDOMUS_DERIVE_PS="$T/ps" expect_exit 0 "$D" --orphans
got="$(printf '%s\n' "$LAST_OUT" | awk '{print $1}' | sort | tr '\n' ' ')"
[ "$got" = "4101 4102 4107 " ] \
  || fail "the reaper would take [$got], not exactly this checkout's orphans [4101 4102 4107 ]"
# and the read-only listing takes no lock
lock_free
