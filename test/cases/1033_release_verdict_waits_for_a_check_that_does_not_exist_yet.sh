# majordomus-covers: none
# claims: release-tag-leaves-a-judged-commit
# The verdict waits for a check that does not exist yet.
#
# The `ci` check-run of a commit is created when the aggregate job of the validate workflow
# starts, and that job is the last one: for most of the time a commit is being judged it has
# no `ci` check-run at all. `scripts/ci/release-verdict --wait N` polled only a run it could
# already see, so on 2026-10-09 `--wait 3600` answered 12 within a second for a commit whose
# validate was running, and the release of v0.19.0 failed in its `plan` job, whose
# `--wait 300` met a tag pushed before the check existed.
#
# The forge is a stub `gh` on PATH, as in case 362, that answers the one endpoint the script
# reads and counts how often it was asked. Time is a stub as well: `sleep` records the
# seconds it was asked for and moves a clock by them, and `date +%s` reads that clock, so
# the case waits for nothing, a loaded runner cannot change what it sees, and the waiting
# itself can be read to the second. This case holds:
#
#   appears    no check-run for two answers, then a successful one: --wait answers 0, having
#              asked three times and slept twice, fifteen seconds each
#   running    the same through a run that appears in progress and then concludes
#   failed     a check that appears and concluded failure is 10, as it is without the wait
#   never      nothing ever appears: 12 when the bound has passed and not a second later,
#              saying how long it waited; the last sleep is cut to what is left of the bound
#   at once    without --wait an absent check-run is 12 on the first answer, with no sleep
#   unread     an answer that cannot be read ends the wait at once: it is not an absence
#
# Each assertion was shown to fail against the script before this change, where the loop
# left on an absent run: `appears` and `running` answered 12 after one question, and `never`
# said nothing of a wait. Restoring `[ -n "$run" ] &&` in front of the loop's first test is
# that mutation.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/release-verdict"
[ -x "$GATE" ] || { echo "    scripts/ci/release-verdict is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq is required"
# Whatever git a caller of the suite was inside of, this case's repository is its own.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY

R="$T/repo"; mkdir -p "$R"
( cd "$R" && git init -q . && git config user.email t@e && git config user.name t \
  && git config commit.gpgsign false && echo a > a && git add a \
  && git -c core.hooksPath=/dev/null commit -qm "feat: a thing" && git tag -a v9.9.9 -m v9.9.9 )
SHA="$(cd "$R" && git rev-parse HEAD)"

# The forge answers from a script: the n-th question gets $T/answers/<n>.json, and a
# question past the last file gets the last one again, so "nothing ever appears" is one file.
mkdir -p "$T/bin" "$T/answers"
cat > "$T/bin/gh" <<STUB
#!/usr/bin/env bash
case "\$*" in
  "api repos/example/repo/commits/$SHA/check-runs?check_name=ci&per_page=100")
    n=\$(( \$(cat "$T/asked" 2>/dev/null || echo 0) + 1 )); echo "\$n" > "$T/asked"
    [ -f "$T/gh-fails-at-\$n" ] && { echo "HTTP 502: Bad Gateway" >&2; exit 1; }
    while [ "\$n" -gt 1 ] && [ ! -f "$T/answers/\$n.json" ]; do n=\$(( n - 1 )); done
    cat "$T/answers/\$n.json"; exit 0 ;;
esac
echo "stub gh: unexpected call: \$*" >&2
exit 1
STUB
DATE="$(command -v date)"
cat > "$T/bin/sleep" <<STUB
#!/usr/bin/env bash
echo "\$1" >> "$T/slept"
echo \$(( \$(cat "$T/clock") + \$1 )) > "$T/clock"
STUB
cat > "$T/bin/date" <<STUB
#!/usr/bin/env bash
[ "\$*" = "+%s" ] && { cat "$T/clock"; exit 0; }
exec "$DATE" "\$@"
STUB
chmod +x "$T/bin/gh" "$T/bin/sleep" "$T/bin/date"

none='{"total_count":0,"check_runs":[]}'
run() { printf '{"total_count":1,"check_runs":[{"id":%s,"name":"ci","status":"%s","conclusion":%s,"html_url":"https://example.invalid/run/%s"}]}' "$1" "$2" "$3" "$1"; }
# forge <answer>...: the answers the next questions get, in order; the counters and the
# clock start over
forge() {
  rm -f "$T"/answers/*.json "$T/asked" "$T/slept" "$T"/gh-fails-at-*
  echo 1000 > "$T/clock"
  local n=0 a
  for a in "$@"; do n=$((n + 1)); printf '%s\n' "$a" > "$T/answers/$n.json"; done
}
asked() { cat "$T/asked" 2>/dev/null || echo 0; }
slept() { [ -f "$T/slept" ] && tr '\n' ' ' < "$T/slept" | sed 's/ $//' || true; }
gate() { ( cd "$R" && PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo "$GATE" "$@" ); }

echo "  a check-run that appears during the wait is the verdict"
forge "$none" "$none" "$(run 7 completed '"success"')"
expect_exit 0 gate --commit "$SHA" --wait 600 || exit 1
expect_grep "'ci' check of ${SHA:0:12} concluded success" || exit 1
[ "$(asked)" = 3 ] || { echo "    the forge was asked $(asked) time(s), not until the check appeared (3)"; exit 1; }
[ "$(slept)" = "15 15" ] || { echo "    between the answers it slept '$(slept)', not 15 and 15"; exit 1; }

echo "  the tag form waits the same way, which is what the release pipeline runs"
forge "$none" "$(run 8 completed '"success"')"
expect_exit 0 gate --tag v9.9.9 --wait 300 || exit 1
[ "$(asked)" = 2 ] || { echo "    the forge was asked $(asked) time(s), not 2"; exit 1; }

echo "  absent, then running, then concluded: one wait covers all three"
forge "$none" "$(run 9 queued null)" "$(run 9 in_progress null)" "$(run 9 completed '"success"')"
expect_exit 0 gate --commit "$SHA" --wait 600 || exit 1
[ "$(asked)" = 4 ] || { echo "    the forge was asked $(asked) time(s), not 4"; exit 1; }

echo "  a check that appears and failed is refused as a failure, not as an absence"
forge "$none" "$(run 10 completed '"failure"')"
expect_exit 10 gate --commit "$SHA" --wait 600 || exit 1
expect_grep 'concluded failure; a release follows the verdict' || exit 1

echo "  nothing ever appears: 12 once the bound has passed, and it says so"
forge "$none"
expect_exit 12 gate --commit "$SHA" --wait 40 || exit 1
expect_grep "has no 'ci' check-run after 40s; nothing says it passed" || exit 1
expect_grep 'a longer --wait would see it, or validate never ran on this commit' || exit 1
# asked at 0, 15, 30 and 40 seconds: the last sleep is what was left of the bound, so the
# answer comes at the bound and not an interval past it
[ "$(asked)" = 4 ] || { echo "    it asked the forge $(asked) time(s) in a 40s wait, not 4"; exit 1; }
[ "$(slept)" = "15 15 10" ] || { echo "    a 40s wait slept '$(slept)', not 15, 15 and the 10 left"; exit 1; }

echo "  a run still in progress at the bound is 12 too, and says it is running"
forge "$none" "$(run 11 in_progress null)"
expect_exit 12 gate --commit "$SHA" --wait 20 || exit 1
expect_grep 'is in_progress after 20s; no verdict yet' || exit 1
[ "$(slept)" = "15 5" ] || { echo "    a 20s wait slept '$(slept)', not 15 and the 5 left"; exit 1; }

echo "  without --wait an absent check-run is 12 at once"
forge "$none" "$(run 12 completed '"success"')"
expect_exit 12 gate --commit "$SHA" || exit 1
expect_grep "has no 'ci' check-run; nothing says it passed" || exit 1
expect_grep 'wait <seconds> waits for it' || exit 1
[ "$(asked)" = 1 ] || { echo "    without --wait the forge was asked $(asked) time(s)"; exit 1; }
[ ! -e "$T/slept" ] || { echo "    without --wait it slept: $(slept)"; exit 1; }
forge "$none" "$(run 12 completed '"success"')"
expect_exit 12 gate --commit "$SHA" --wait 0 || exit 1
[ "$(asked)" = 1 ] || { echo "    --wait 0 asked the forge $(asked) time(s)"; exit 1; }

echo "  an answer that cannot be read ends the wait: unread is not absent"
forge "$none" "$(run 13 completed '"success"')"
touch "$T/gh-fails-at-2"
expect_exit 12 gate --commit "$SHA" --wait 600 || exit 1
expect_grep 'could not be read' || exit 1
[ "$(asked)" = 2 ] || { echo "    it went on asking after an unreadable answer ($(asked) questions)"; exit 1; }

echo "    --wait waits for a check-run that does not exist yet, for one that is running, and for no longer than it was told; without it the answer is this moment's"
exit 0
