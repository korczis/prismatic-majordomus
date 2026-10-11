# shellcheck shell=bash
# One derive at a time on a machine that many checkouts share (project.one-derive-at-a-time).
#
# A derive rewrites every committed artifact and costs minutes of a machine that, on
# 2026-10-09, carried nine sessions at load 57. They took turns through a directory
# `.derive.lock` beside the primary checkout and a priority marker beside it, by agreement,
# in wrappers of their own. The agreement failed as agreements do. One `just derive` without
# the wrapper ran beside a repair's derive. Stopping it killed the shell and not stage B,
# whose generate-site-data ran on for an hour reparented to pid 1. So the lock is taken by the
# script that does the work, and by nothing else. d1 built the same lock independently
# (fix/a-derive-takes-the-machine-lock); its reading of owner lines without a pid is the one
# this file keeps.
#
#   <home>/.derive.lock/owner   one line naming the holder; this file writes
#                               "<who>:<branch> <pid>". scripts/reap-orphans (I2109) reads the
#                               branch after the last ':' to spare the worktree in use, and
#                               case 1018 pins the shape, so the two change together
#                               or not at all
#   <home>/.derive.priority     present: only a derive on a branch it names, as a whole word,
#                               may take the lock; any other waits
#
# <home> is the directory holding the primary checkout, found from git's common directory, so
# every worktree agrees on it without configuration and no machine path is written down.
#
#   MAJORDOMUS_DERIVE_LOCK              the lock directory instead
#   MAJORDOMUS_DERIVE_PRIORITY          the priority marker instead
#   MAJORDOMUS_DERIVE_IGNORE_PRIORITY=1 the marker is this worker's own, under another name
#   MAJORDOMUS_DERIVE_WAIT_MAX          seconds before giving up with exit 75 (default 14400)
#   MAJORDOMUS_DERIVE_POLL              seconds between looks (default 15)
#   MAJORDOMUS_DERIVE_PS                a file of "pid ppid command" lines read in place of
#                                       the process table (a case's fixture)
#
# What it decides:
# - The holder's pid is the owner line's `pid=<n>` or its last word, and only when that is a
#   number. A line with no numeric pid, written by hand or by a wrapper that named a time
#   instead, is a live holder nobody can check. It is never reclaimed and never taken for an
#   ancestor. It is waited for, and its line is printed.
# - A numeric pid that is an ancestor of this process means the caller holds the lock: a
#   wrapper took it the old way and runs this derive. Nothing is taken and nothing released.
# - A numeric pid that is not running is a dead owner. The lock is reclaimed and the reclaim
#   is printed with the owner line. It is never silent.
# - Only the process that wrote the owner line removes the lock, and only while the line is
#   still exactly what it wrote.
# - Every child runs in the background under `wait`, because bash defers a trap until a
#   foreground child returns, and a deferred trap is how stage B was orphaned. On TERM, INT
#   or HUP the whole tree is signalled deepest first, then killed, and the lock released.

MJ_LOCK_HOLD=""

# mj_lock_init <checkout root> <who>: where the lock and the marker are, and who is asking.
mj_lock_init() {
  local common
  MJ_LOCK_ROOT="$1"; MJ_LOCK_WHO="${2:-derive}"
  common="$(git -C "$1" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || printf '%s/.git' "$1")"
  MJ_LOCK_DIR="${MAJORDOMUS_DERIVE_LOCK:-$(dirname "$(dirname "$common")")/.derive.lock}"
  MJ_LOCK_PRIORITY="${MAJORDOMUS_DERIVE_PRIORITY:-$(dirname "$MJ_LOCK_DIR")/.derive.priority}"
  MJ_LOCK_BRANCH="$(git -C "$1" symbolic-ref --quiet --short HEAD 2>/dev/null || printf 'detached')"
  MJ_LOCK_LINE="$MJ_LOCK_WHO:$MJ_LOCK_BRANCH $$"
}

mj_lock_say() { printf '==> %s: %s\n' "$MJ_LOCK_WHO" "$*" >&2; }

# The holder's pid from an owner line, or nothing when the line names no number.
mj_lock_owner_pid() {
  local pid
  pid="$(printf '%s\n' "$1" | sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p')"
  [ -n "$pid" ] || pid="${1##* }"
  case "$pid" in ''|*[!0-9]*) pid="" ;; esac
  printf '%s' "$pid"
}

mj_lock_is_running() { [ -n "$1" ] && ps -p "$1" >/dev/null 2>&1; }
mj_lock_is_ancestor() {
  local p="$$"
  [ -n "$1" ] || return 1
  while [ -n "$p" ] && [ "$p" != 1 ] && [ "$p" != 0 ]; do
    [ "$p" = "$1" ] && return 0
    p="$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')"
  done
  return 1
}

# Whether the priority marker names this branch as a whole word: "feature/x" is not named by
# a marker that names "feature/x-two".
mj_lock_marker_names_us() {
  awk -v b="$MJ_LOCK_BRANCH" '
    { n = split($0, w, /[[:space:]:,()]+/); for (i = 1; i <= n; i++) if (w[i] == b) f = 1 }
    END { exit !f }' "$MJ_LOCK_PRIORITY" 2>/dev/null
}

mj_lock_descendants() {
  local c
  for c in $(pgrep -P "$1" 2>/dev/null); do mj_lock_descendants "$c"; printf '%s\n' "$c"; done
}
mj_lock_kill_tree() {
  local pids
  pids="$(mj_lock_descendants "$1" | tr '\n' ' ')"
  [ -n "$pids" ] || return 0
  # shellcheck disable=SC2086
  kill -TERM $pids 2>/dev/null || true; sleep 1
  # shellcheck disable=SC2086
  kill -KILL $pids 2>/dev/null || true
}

mj_lock_release() {
  [ "$MJ_LOCK_HOLD" = own ] || return 0
  MJ_LOCK_HOLD=""
  [ "$(cat "$MJ_LOCK_DIR/owner" 2>/dev/null)" = "$MJ_LOCK_LINE" ] || {
    mj_lock_say "$MJ_LOCK_DIR no longer names this process; left as it is"; return 0; }
  rm -f "$MJ_LOCK_DIR/owner"; rmdir "$MJ_LOCK_DIR" 2>/dev/null || true
}

mj_lock_on_signal() {
  trap - TERM INT HUP
  mj_lock_say "stopped by SIG$1; taking the whole process tree down"
  mj_lock_kill_tree "$$"
  mj_lock_release
  exit "$2"
}

# The traps a lock holder needs: the tree goes down on a signal, the lock on any exit.
mj_lock_traps() {
  trap 'mj_lock_on_signal TERM 143' TERM
  trap 'mj_lock_on_signal INT 130' INT
  trap 'mj_lock_on_signal HUP 129' HUP
  trap 'mj_lock_release' EXIT
}

# A child in the background and `wait` for it: `wait` is interruptible, a foreground child is
# not. The child's status is the function's, so `set -e` and `|| rc=$?` read it as before.
mj_lock_run() {
  local pid rc=0
  "$@" & pid=$!
  wait "$pid" || rc=$?
  return "$rc"
}

mj_lock_acquire() {
  local owner pid waited=0 said="" why poll="${MAJORDOMUS_DERIVE_POLL:-15}"
  local max="${MAJORDOMUS_DERIVE_WAIT_MAX:-14400}"
  mkdir -p "$(dirname "$MJ_LOCK_DIR")"
  while :; do
    owner="$(cat "$MJ_LOCK_DIR/owner" 2>/dev/null || true)"
    pid="$(mj_lock_owner_pid "$owner")"
    if [ -d "$MJ_LOCK_DIR" ] && mj_lock_is_ancestor "$pid"; then
      MJ_LOCK_HOLD=ancestor
      mj_lock_say "$MJ_LOCK_DIR is held by this process's own caller ($owner)"
      return 0
    fi
    if [ -d "$MJ_LOCK_DIR" ] && [ -n "$pid" ] && ! mj_lock_is_running "$pid"; then
      mj_lock_say "reclaimed $MJ_LOCK_DIR from a dead owner: $owner (pid $pid is not running)"
      rm -f "$MJ_LOCK_DIR/owner"; rmdir "$MJ_LOCK_DIR" 2>/dev/null || true
      continue
    fi
    if [ -e "$MJ_LOCK_PRIORITY" ] && [ "${MAJORDOMUS_DERIVE_IGNORE_PRIORITY:-}" != 1 ] \
      && ! mj_lock_marker_names_us; then
      why="$MJ_LOCK_PRIORITY names another branch: $(head -n 1 "$MJ_LOCK_PRIORITY" 2>/dev/null)"
    elif mkdir "$MJ_LOCK_DIR" 2>/dev/null; then
      printf '%s\n' "$MJ_LOCK_LINE" > "$MJ_LOCK_DIR/owner"
      MJ_LOCK_HOLD=own
      [ "$waited" = 0 ] || mj_lock_say "lock taken after ${waited}s"
      return 0
    elif [ -z "$pid" ] && [ -n "$owner" ]; then
      why="$MJ_LOCK_DIR is held by an owner that names no pid, so it cannot be checked: $owner"
    else
      why="$MJ_LOCK_DIR is held by ${owner:-an owner still writing its name}"
    fi
    if [ "$why" != "$said" ]; then mj_lock_say "waiting: $why"; said="$why"; fi
    if [ "$waited" -ge "$max" ]; then mj_lock_say "gave up after ${waited}s: $why"; exit 75; fi
    sleep "$poll"; waited=$(( waited + poll ))
  done
}

# An orphan of an earlier derive of this checkout: a SIGKILL leaves no trap to run. Its
# identity is this checkout's absolute script path, starting a word in its command line,
# together with parent pid 1. Another worktree's derive has another path, and a live derive
# has a live parent, so neither is ever matched. Nothing is matched by a process name
# (project.reclaim-only-what-you-own).
mj_lock_process_table() {
  if [ -n "${MAJORDOMUS_DERIVE_PS:-}" ]; then cat "$MAJORDOMUS_DERIVE_PS"
  else ps -axo pid=,ppid=,command= 2>/dev/null; fi
}
mj_lock_orphans() {
  mj_lock_process_table | awk -v me="$$" \
      -v a="$MJ_LOCK_ROOT/scripts/derive" -v b="$MJ_LOCK_ROOT/scripts/generate-site-data" '
    function owns(s, p,   i, c) {
      i = index(s, p); if (!i) return 0
      if (i > 1 && substr(s, i - 1, 1) != " ") return 0   # the path must start a word
      c = substr(s, i + length(p), 1)          # and end there, or at "-check"
      return c == "" || c == " " || substr(s, i + length(p), 6) == "-check"
    }
    $2 == 1 && $1 != me && (owns($0, a) || owns($0, b)) { print }'
}
mj_lock_reap_orphans() {
  local pid rest
  while read -r pid _ rest; do
    [ -n "$pid" ] || continue
    mj_lock_say "an orphan of an earlier derive here is still running (pid $pid: $rest); stopping it and its tree"
    mj_lock_kill_tree "$pid"; kill -KILL "$pid" 2>/dev/null || true
  done <<EOF_ORPHANS
$(mj_lock_orphans)
EOF_ORPHANS
}
