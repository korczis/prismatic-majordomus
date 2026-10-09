# majordomus-covers: none
# A case leaves no git housekeeping behind.
#
# git may start `maintenance run --auto` detached after a commit, a fetch or a merge, and a
# detached process goes on writing into .git after the case's last assertion. The case's
# EXIT trap then removes a directory that is still being written to, `rm -rf` fails with
# "Directory not empty", and under `set -e` that is the case's status: case 489 failed so on
# 2026-10-08 with nothing wrong in what it asserts.
#
# test/lib.sh therefore tells every git a case starts to run no housekeeping of its own.
# This case holds the three things that makes true:
#
#   1  inside a case, git answers gc.auto=0 and maintenance.auto=false, and so does a
#      repository the case makes, because the setting travels in the environment
#   2  what the environment already carried is kept: an identity a runner names through
#      GIT_CONFIG_* is still the identity afterwards
#   3  sourcing the library again adds nothing
. "$ROOT/test/lib.sh"

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }

# ---------------------------------------------------------------- 1. the setting, here and in a new repository
same "gc.auto in the fixture" "0" "$(git config --get gc.auto)"
same "maintenance.auto in the fixture" "false" "$(git config --get maintenance.auto)"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj995.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q "$S/made"
same "gc.auto in a repository the case made" "0" "$(git -C "$S/made" config --get gc.auto)"
same "maintenance.auto in a repository the case made" "false" "$(git -C "$S/made" config --get maintenance.auto)"

# a child process is a git the tool under test would start: it inherits the setting
same "gc.auto in a child process" "0" "$(bash -c 'git config --get gc.auto')"

# ---------------------------------------------------------------- 2. what the environment carried is kept
# A harness that names an identity through the environment, then loads the library: both
# the identity and the two settings are there afterwards, at the indexes that follow it.
kept="$(
  unset MJ_CASE_GIT_IS_QUIET
  GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=user.name GIT_CONFIG_VALUE_0="a runner's identity"
  export GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0
  unset GIT_CONFIG_KEY_1 GIT_CONFIG_VALUE_1 GIT_CONFIG_KEY_2 GIT_CONFIG_VALUE_2
  . "$ROOT/test/lib.sh"
  printf '%s|%s|%s|%s' "$GIT_CONFIG_COUNT" "$(git config --get user.name)" \
    "$(git config --get gc.auto)" "$(git config --get maintenance.auto)"
)"
same "an identity named through the environment survives the library" \
  "3|a runner's identity|0|false" "$kept"

# a count that is not a number is not trusted: the two settings start at zero
garbled="$(
  unset MJ_CASE_GIT_IS_QUIET
  GIT_CONFIG_COUNT=many; export GIT_CONFIG_COUNT
  . "$ROOT/test/lib.sh"
  printf '%s|%s|%s' "$GIT_CONFIG_COUNT" "$(git config --get gc.auto)" "$(git config --get maintenance.auto)"
)"
same "a count that is not a number starts the settings at zero" "2|0|false" "$garbled"

# ---------------------------------------------------------------- 3. once
before="$GIT_CONFIG_COUNT"
. "$ROOT/test/lib.sh"
same "sourcing the library again adds nothing" "$before" "$GIT_CONFIG_COUNT"

echo "    every git a case starts runs no housekeeping of its own, an identity the environment named is kept, and the library adds its two settings once"
