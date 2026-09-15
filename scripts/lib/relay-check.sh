# shellcheck shell=bash
# relay_check — run a checker and turn its output into this script's own verdicts.
#
# scripts/site-check delegates two checks to scripts under scripts/ci/ and relays what they
# print: `OK …` becomes an ok, `FAIL …` becomes a failure, the closing summary counts only when
# the run could not decide at all, and any other line counts when the run failed. That relay was
# written twice, once per checker, and a verdict written twice is a verdict that can disagree
# with itself.
#
# It also had no answer for silence. Every branch of the relay asks "is this line bad?", and
# every one of them needs a line to exist: a checker that dies before printing anything — a
# crash, a missing interpreter, a new code path that exits early — leaves a non-zero status and
# an empty file, so the loop body never runs, nothing is recorded, and the section reports as
# passed. That is the same verdict lost that #351 fixed from the other side, where `bad` ran in
# a subshell of a pipeline and sixteen findings printed while the script exited 0. Printed and
# discarded, or never printed at all; either way the caller is told everything is well.
#
# So the relay answers for no output too, and it is one function. It expects the caller to
# provide `ok`, `bad` and the `fails` counter, which is what site-check has.
#
#   relay_check <subject> <script> <ok-prefix> <fail-prefix>
#
# <subject> is the column the verdicts are filed under; the prefixes are what the checker writes
# ahead of its own message, stripped so the relayed line reads as this script's own.
relay_check() {
  local subject="$1" script="$2" ok_prefix="$3" fail_prefix="$4"
  local out rc=0 before line name
  name="$(basename "$script")"
  out="$(mktemp "${TMPDIR:-/tmp}/mj.relay.XXXXXX")"
  # `fails`, `ok` and `bad` belong to the caller: this is a relay into someone else's
  # verdicts, not a checker of its own.
  # shellcheck disable=SC2154
  before="$fails"
  MJ_ROOT="$ROOT" "$script" > "$out" 2>&1 || rc=$?
  while IFS= read -r line; do
    case "$line" in
      "OK   "*) ok "$subject" "${line#"$ok_prefix"}" ;;
      "FAIL "*) bad "$subject" "${line#"$fail_prefix"}" ;;
      "") ;;
      # the closing summary of a run that found something restates its findings; only a run that
      # could not decide at all (exit 12) has a line of its own to count
      "$name: "*) [ "$rc" = 12 ] && bad "$subject" "$line" || printf '     %-12s %s\n' "$subject" "$line" ;;
      *) [ "$rc" = 0 ] || bad "$subject" "$line" ;;
    esac
  done < "$out"
  # the verdict for no output at all
  [ "$rc" = 0 ] || [ "$fails" != "$before" ] \
    || bad "$subject" "$name exited $rc and printed nothing; a check that could not speak has not passed"
  rm -f "$out"
  return 0
}
