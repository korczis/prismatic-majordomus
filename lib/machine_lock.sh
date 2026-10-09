# shellcheck shell=bash
# One derive at a time on a machine that many checkouts share.
#
# A derive rewrites every derived artifact of a checkout and takes most of the machine while
# it runs: minutes of build, then the site generator and the use-case scenarios. Every
# worktree of a repository lives beside the others on one machine, and on 2026-10-09 eleven
# sessions shared it. They took turns through a directory `.derive.lock` beside the primary
# checkout, made with `mkdir`, with an `owner` file — by agreement. The agreement failed
# every way an agreement can: a derive run without the lock beside one holding it, a session
# that restarted and left the lock under its name with nothing running, a waiting loop killed
# with its caller, a stopped derive whose site generator ran on for an hour as an orphan, and
# a priority marker read by some sessions and not by others.
#
# So the lock is taken by the derive itself, and kept by nothing but the process that took it.
# `scripts/derive` sources this file and calls `mj_lock_acquire` before its first stage; no
# caller needs to know the protocol for it to hold.
#
#   <home>/.derive.lock/owner   one line: who, `pid=<pid>`, `branch=<branch>`, where, when
#   <home>/.derive.priority     present: only a derive on a branch the file names may take
#                               the lock; any other waits. A person or a session leaves it for
#                               a branch the integration queue waits on, naming that branch.
#
# <home> is the directory that holds the repository's primary checkout, found from git's
# common directory, so every worktree of the repository agrees on it without configuration.
#
#   MJ_DERIVE_LOCK_HOME   use another directory (the case that proves this lock does)
#   MJ_DERIVE_LOCK_WAIT   give up after this many seconds of waiting, exit 12; unset waits
#   MJ_DERIVE_LOCK_POLL   seconds between attempts (default 15)
#   MJ_DERIVE_LOCK_HELD   set by a caller that took the lock itself the old way, by hand,
#                         before calling the derive: the derive then takes nothing and
#                         releases nothing. A caller that does not hold it must not set it.
#   MJ_DERIVE_OWNER       the name written into the owner line (default `derive`)
#
# What it decides, and what it never does:
#
# - A lock whose owner line names a pid that no longer runs is stale: the derive that held it
#   is gone and nothing of it can still be writing. It is reclaimed, and the reclaim is
#   printed with the line it replaced.
# - A lock whose owner names no pid — taken by hand, the old way — is never reclaimed: whether
#   its holder lives cannot be told, so it is waited for, and the wait prints who holds it.
# - A lock held by an ancestor of this process is this process's own: a caller that took it
#   and then ran the derive does not wait for itself. It is not released here either.
# - Only the process whose pid the owner line names removes the lock.

# The directory that holds the lock and the priority marker.
mj_lock_home() {
  if [ -n "${MJ_DERIVE_LOCK_HOME:-}" ]; then printf '%s' "$MJ_DERIVE_LOCK_HOME"; return 0; fi
  local common
  common="$(git -C "$1" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
  dirname "$(dirname "$common")"
}

# The pid an owner line names, or nothing.
mj_lock_pid_of() { sed -n 's/.*[[:space:]]pid=\([0-9][0-9]*\).*/\1/p' "$1" 2>/dev/null | head -1; }

# Whether pid $1 is this process or one of its ancestors.
mj_lock_is_ancestor() {
  local p=$$
  while [ -n "$p" ] && [ "$p" -gt 1 ]; do
    [ "$p" = "$1" ] && return 0
    p="$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')"
  done
  return 1
}

# Take the lock for the checkout at $1, writing $2 as the owner's name. Waits as long as
# MJ_DERIVE_LOCK_WAIT allows; returns 12 when it cannot take it or cannot even find where it
# lives. Sets MJ_LOCK_OWNED=1 when this process took it and must release it.
mj_lock_acquire() {
  local root="$1" tag="${2:-derive}" home lock prio branch owner pid waited=0 said=0
  MJ_LOCK_OWNED=0
  if [ -n "${MJ_DERIVE_LOCK_HELD:-}" ]; then
    printf '==> derive: MJ_DERIVE_LOCK_HELD is set; the caller holds the machine lock\n' >&2
    return 0
  fi
  home="$(mj_lock_home "$root")" || { printf 'derive: cannot tell where the machine lock lives (git -C %s rev-parse --git-common-dir)\n' "$root" >&2; return 12; }
  lock="$home/.derive.lock"; prio="$home/.derive.priority"
  branch="$(git -C "$root" symbolic-ref --short -q HEAD 2>/dev/null || printf 'detached')"
  MJ_LOCK_DIR="$lock"
  while :; do
    if [ -f "$prio" ] && ! grep -qF -- "$branch" "$prio" 2>/dev/null; then
      owner="priority: $(head -1 "$prio" 2>/dev/null)"
    elif mkdir "$lock" 2>/dev/null; then
      printf '%s pid=%s branch=%s worktree=%s at=%s\n' "$tag" "$$" "$branch" "$root" \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$lock/owner"
      MJ_LOCK_OWNED=1
      [ "$said" = 0 ] || printf '==> derive: took the machine lock after %ss\n' "$waited" >&2
      return 0
    else
      owner="$(head -1 "$lock/owner" 2>/dev/null)"
      pid="$(printf '%s\n' "$owner" | sed -n 's/.*[[:space:]]pid=\([0-9][0-9]*\).*/\1/p')"
      if [ -n "$pid" ] && mj_lock_is_ancestor "$pid"; then
        printf '==> derive: the machine lock is held by this derive'"'"'s caller (%s)\n' "$owner" >&2
        return 0
      fi
      if [ -n "$pid" ] && ! kill -0 "$pid" 2>/dev/null; then
        printf '==> derive: reclaiming the machine lock; its holder (pid %s) is gone: %s\n' "$pid" "$owner" >&2
        rm -rf "$lock"
        continue
      fi
      [ -n "$owner" ] || owner="an owner line not yet written"
    fi
    if [ -n "${MJ_DERIVE_LOCK_WAIT:-}" ] && [ "$waited" -ge "$MJ_DERIVE_LOCK_WAIT" ]; then
      printf 'derive: gave up after %ss waiting for the machine lock (%s): %s\n' "$waited" "$lock" "$owner" >&2
      return 12
    fi
    # said aloud when the wait starts and then once a minute: a derive that waits in
    # silence reads as a hang, and a session has already killed a healthy one for it
    if [ "$said" = 0 ] || [ $(( waited % 60 )) -lt "${MJ_DERIVE_LOCK_POLL:-15}" ]; then
      printf '==> derive: waiting for the machine lock (%ss): %s\n' "$waited" "$owner" >&2
      said=1
    fi
    sleep "${MJ_DERIVE_LOCK_POLL:-15}"
    waited=$(( waited + ${MJ_DERIVE_LOCK_POLL:-15} ))
  done
}

# Release the lock, when this process took it and the owner line still names it.
mj_lock_release() {
  [ "${MJ_LOCK_OWNED:-0}" = 1 ] || return 0
  [ "$(mj_lock_pid_of "$MJ_LOCK_DIR/owner")" = "$$" ] || return 0
  rm -rf "$MJ_LOCK_DIR"
  MJ_LOCK_OWNED=0
}

# Stop every descendant of pid $1, deepest first. A stopped derive must not leave its site
# generator running as an orphan beside the next one.
mj_lock_kill_tree() {
  local child
  for child in $(pgrep -P "$1" 2>/dev/null); do
    mj_lock_kill_tree "$child"
    kill -TERM "$child" 2>/dev/null || :
  done
}
