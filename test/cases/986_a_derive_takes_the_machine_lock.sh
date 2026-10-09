# majordomus-covers: none
# A derive takes the machine lock itself, and no session's agreement is needed for it to hold.
#
# Eleven sessions shared one machine on 2026-10-09 and took turns deriving through a lock kept
# by agreement. In one day it was skipped, leaked by a session that restarted, held by a loop
# killed with its caller, and outlived by an orphaned site generator. lib/machine_lock.sh is
# what scripts/derive now calls before its first stage. This case drives it over a lock home
# of its own and proves each decision:
#
#   free                              taken; the owner line names this process's pid
#   held by a live process            waited for, said aloud, and given up only when told to
#   held by a pid that is gone        reclaimed, and the reclaim names the line it replaced
#   held by hand (no pid)             never reclaimed: waited for
#   a priority marker for another     waited for; one naming this branch lets it through
#   held by this process's caller     its own: taken without waiting, and not released
#   MJ_DERIVE_LOCK_HELD               the caller says it holds it: nothing taken or released
#   released                          only by the process the owner line names
#   interrupted                       every descendant stopped
#
# and that scripts/derive itself takes it before anything else and lets it go on exit.
. "$ROOT/test/lib.sh"

LIB="$ROOT/lib/machine_lock.sh"
HOME_DIR="$T/home"; mkdir -p "$HOME_DIR"
LOCK="$HOME_DIR/.derive.lock"; PRIO="$HOME_DIR/.derive.priority"
git checkout -q -b feature/the-probe 2>/dev/null || git switch -q -c feature/the-probe
same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
# one attempt to take the lock in a child shell over this repository; prints the child's
# pid on stdout and everything it said on stderr into $T/said
attempt() {
  MJ_DERIVE_LOCK_HOME="$HOME_DIR" MJ_DERIVE_LOCK_WAIT="${WAIT:-0}" MJ_DERIVE_LOCK_POLL=1 \
    bash -c '. "$1"; mj_lock_acquire "$2" probe; rc=$?; echo "pid=$$ rc=$rc owned=$MJ_LOCK_OWNED"; exit 0' \
    _ "$LIB" "$PWD" 2> "$T/said"
}

# ---------------------------------------------------------------- free: taken, and named
out="$(attempt)"
case "$out" in *"rc=0 owned=1") ;; *) echo "    a free lock was not taken: $out"; cat "$T/said"; exit 1 ;; esac
child="$(printf '%s' "$out" | sed -n 's/^pid=\([0-9]*\).*/\1/p')"
grep -q "^probe pid=$child branch=feature/the-probe worktree=$PWD at=" "$LOCK/owner" \
  || { echo "    the owner line does not name the taker:"; cat "$LOCK/owner"; exit 1; }

# ---------------------------------------------------------------- a holder that is gone
# the child above has exited without releasing: its pid is gone, so the lock is stale
out="$(attempt)"
case "$out" in *"rc=0 owned=1") ;; *) echo "    a stale lock was not reclaimed: $out"; cat "$T/said"; exit 1 ;; esac
grep -q "reclaiming the machine lock; its holder (pid $child) is gone: probe pid=$child" "$T/said" \
  || { echo "    the reclaim did not name the line it replaced:"; cat "$T/said"; exit 1; }
rm -rf "$LOCK"

# ---------------------------------------------------------------- a live holder is waited for
sleep 300 & holder=$!
mkdir "$LOCK"; printf 'someone pid=%s branch=elsewhere worktree=/x at=now\n' "$holder" > "$LOCK/owner"
out="$(WAIT=2 attempt)"
same "a live holder makes it give up when told to" "rc=12 owned=0" "${out#pid=* }"
grep -q 'waiting for the machine lock (0s): someone pid=' "$T/said" \
  || { echo "    the wait was silent:"; cat "$T/said"; exit 1; }
grep -q "^someone pid=$holder" "$LOCK/owner" || { echo "    a live holder's lock was taken"; exit 1; }
kill "$holder" 2>/dev/null || :; wait "$holder" 2>/dev/null || :

# ---------------------------------------------------------------- a lock taken by hand
printf 'd1 taken by hand\n' > "$LOCK/owner"
out="$(WAIT=2 attempt)"
same "a lock with no pid is never reclaimed" "rc=12 owned=0" "${out#pid=* }"
grep -q 'd1 taken by hand' "$LOCK/owner" || { echo "    a hand-taken lock was removed"; exit 1; }
# a caller that took it that way says so, and the derive takes nothing and releases nothing
out="$(MJ_DERIVE_LOCK_HELD=1 attempt)"
same "a caller that holds it by hand" "rc=0 owned=0" "${out#pid=* }"
grep -q 'd1 taken by hand' "$LOCK/owner" || { echo "    a held lock was touched"; exit 1; }
rm -rf "$LOCK"

# ---------------------------------------------------------------- the priority marker
printf '19: feature/somebody-else (owner priority)\n' > "$PRIO"
out="$(WAIT=2 attempt)"
same "a marker for another branch makes it wait" "rc=12 owned=0" "${out#pid=* }"
grep -q 'waiting for the machine lock (0s): priority: 19: feature/somebody-else' "$T/said" \
  || { echo "    the wait does not name the marker:"; cat "$T/said"; exit 1; }
[ ! -e "$LOCK" ] || { echo "    the lock was taken past a marker for another branch"; exit 1; }
printf 'd1: feature/the-probe, then feature/somebody-else\n' > "$PRIO"
out="$(attempt)"
case "$out" in *"rc=0 owned=1") ;; *) echo "    a marker naming this branch did not let it through: $out"; exit 1 ;; esac
rm -rf "$LOCK" "$PRIO"

# ---------------------------------------------------------------- the caller's own lock
# a caller takes it, then runs a derive: the derive does not wait for its own caller, and
# does not release what it did not take
out="$(MJ_DERIVE_LOCK_HOME="$HOME_DIR" MJ_DERIVE_LOCK_WAIT=2 MJ_DERIVE_LOCK_POLL=1 bash -c '
  . "$1"; mj_lock_acquire "$2" caller >/dev/null 2>&1 || exit 1
  inner="$(bash -c ". \"$1\"; mj_lock_acquire \"$2\" inner; echo rc=\$? owned=\$MJ_LOCK_OWNED; mj_lock_release" _ "$1" "$2" 2>/dev/null)"
  [ -d "$MJ_LOCK_DIR" ] && echo "$inner still-held" || echo "$inner released-by-inner"
  mj_lock_release; [ -d "$MJ_LOCK_DIR" ] && echo leaked || echo released' _ "$LIB" "$PWD")"
same "the caller's lock is its derive's" "rc=0 owned=0 still-held
released" "$out"

# ---------------------------------------------------------------- only the owner releases
out="$(attempt)"
mv "$LOCK/owner" "$T/o"; sed 's/pid=[0-9]*/pid=1/' "$T/o" > "$LOCK/owner"
MJ_DERIVE_LOCK_HOME="$HOME_DIR" bash -c '. "$1"; MJ_LOCK_OWNED=1; MJ_LOCK_DIR="$2"; mj_lock_release' _ "$LIB" "$LOCK"
[ -d "$LOCK" ] || { echo "    a process released a lock another pid owns"; exit 1; }
rm -rf "$LOCK"

# ---------------------------------------------------------------- an interrupt stops the tree
bash -c '. "$1"; sleep 300 & sleep 300 & wait' _ "$LIB" & parent=$!
sleep 1
kids="$(pgrep -P "$parent" | tr '\n' ' ')"
[ -n "$kids" ] || { echo "    the probe started no children"; exit 1; }
bash -c '. "$1"; mj_lock_kill_tree "$2"' _ "$LIB" "$parent"
sleep 1
for k in $kids; do kill -0 "$k" 2>/dev/null && { echo "    descendant $k survived"; kill "$k"; exit 1; }; done
kill "$parent" 2>/dev/null || :; wait "$parent" 2>/dev/null || :

# ---------------------------------------------------------------- scripts/derive takes it first
# before its first stage and before it builds anything, and lets it go on every exit
d="$ROOT/scripts/derive"
lock_line="$(grep -n '^mj_lock_acquire ' "$d" | head -1 | cut -d: -f1)"
first_step="$(grep -n '^  step \|^step ' "$d" | head -1 | cut -d: -f1)"
[ -n "$lock_line" ] && [ -n "$first_step" ] && [ "$lock_line" -lt "$first_step" ] \
  || { echo "    scripts/derive does not take the lock before its first stage ($lock_line, $first_step)"; exit 1; }
grep -q "^trap 'mj_lock_release' EXIT" "$d" || { echo "    scripts/derive does not release on exit"; exit 1; }
grep -q "^trap 'mj_lock_kill_tree \$\$; exit 130' INT TERM HUP" "$d" \
  || { echo "    scripts/derive does not stop its tree on an interrupt"; exit 1; }
