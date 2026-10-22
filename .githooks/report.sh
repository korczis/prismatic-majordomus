# shellcheck shell=sh
# Sourced by the hooks. A local gate reports and CI decides (owner, 2026-10-21): a check
# that only CI can settle, or that a loaded machine can fail for the load, prints its
# finding and lets the commit or push through. The checks that keep refusing are the cheap
# integrity ones (derive-current, the SSH permissions, the task's scope, case numbers),
# called without this helper.
#
# MAJORDOMUS_HOOK tells bin/majordomus-cli it runs inside a hook, where a stale executable
# is replaced by the installed release instead of a cold cargo build.
MAJORDOMUS_HOOK="${MAJORDOMUS_HOOK:-1}"; export MAJORDOMUS_HOOK

# report <hook> <what> <command>...
report() {
  mj_hook="$1" mj_what="$2"; shift 2
  "$@" && return 0
  mj_rc=$?
  printf '%s: %s failed (exit %s); reported, not refused: CI holds the verdict\n' \
    "$mj_hook" "$mj_what" "$mj_rc" >&2
  return 0
}
