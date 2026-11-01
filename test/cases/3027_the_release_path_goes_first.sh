# majordomus-covers: none
# The release path goes first on the shared runner pool (I2710, evidence
# release_path_has_priority_case).
#
# On 2026-10-14 v0.26.0 was published while its record's verdict (validate run 38620987044)
# and its Pages deploy (pages run 38620990702) sat queued: the pull-request validates of #897
# and #946 held all twenty job slots, and a person cancelled one by hand. scripts/ci/release-path
# decides who yields, from the runs GitHub lists; this case feeds it those lists through a
# stub `gh` on PATH, offline, and reads what it asked the forge to do:
#
#   1  clear: nothing of the release path runs, a pull request's run proceeds, nothing written
#   2  each member of the release path makes it busy — the release run, the record's verdict,
#      the deploy the release's token dispatched — and nothing else does: a pages run of a
#      push, a pages run a person dispatched, master's own validate
#   3  busy: a pull request's run labels its pull request, says which run it yielded to, and
#      cancels itself; a push, a dispatch and a record branch never yield
#   4  preempt: every pull-request validate queued or running is labelled, told and cancelled;
#      the record's verdict, master's validate, the caller and a run with no pull request are
#      not; a cancelled run still running after the wait is force-cancelled, a completed one
#      and one that cannot be read are not; clear, and nothing is touched
#   5  resume: busy, nothing; clear, the cancelled newest run of a labelled head runs again,
#      a head whose newest run is not the cancelled one is only unlabelled
#   6  a forge that cannot be read decides nothing: status says 12, yield lets the run proceed
#   7  the wiring: validate.yml asks first thing in its plan job on a pull request alone and
#      resumes at the end of every other run; release-path.yml preempts on the tag push and
#      resumes on the schedule the model declares
#   8  a train never yields (the owner, 2026-11-01): a pull request labelled with the model's
#      exempt_label neither cancels itself nor is preempted, its members and every other pull
#      request still are, and one whose labels cannot be read is not known to be a train and
#      yields as before (the conservative reading: an unreadable label exempts nothing)
. "$ROOT/test/lib.sh"
RP="$ROOT/scripts/ci/release-path"
[ -x "$RP" ] || { echo "    scripts/ci/release-path is missing or not executable"; exit 1; }
MODEL="$ROOT/.ai/repo/ci/release-path.yaml"
V="$ROOT/.github/workflows/validate.yml"
WF="$ROOT/.github/workflows/release-path.yml"

FX="$T/fx"; LOG="$T/gh.log"; mkdir -p "$T/bin" "$FX"
# The stub answers from files under $FX and writes every call to $LOG, one per line.
cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
[ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503: the forge is down" >&2; exit 1; }
[ "$1" = api ] || { echo "stub gh: only api" >&2; exit 1; }
shift
case "$*" in
  "-X POST "*) echo '{}' ;;
  "-X DELETE "*) echo '[]' ;;
  *"/actions/runs?status="*)
    a="$*"; s="${a#*status=}"; s="${s%%&*}"
    if [ -f "$GH_STUB_FX/runs-$s.json" ]; then cat "$GH_STUB_FX/runs-$s.json"; else echo '{"workflow_runs":[]}'; fi ;;
  *"/issues?labels="*) if [ -f "$GH_STUB_FX/issues.json" ]; then cat "$GH_STUB_FX/issues.json"; else echo '[]'; fi ;;
  *"/issues/"[0-9]*"/labels")
    n="${*%/labels}"; n="${n##*/}"
    [ ! -f "$GH_STUB_FX/labels-$n.down" ] || { echo "HTTP 502: the labels of #$n did not answer" >&2; exit 1; }
    if [ -f "$GH_STUB_FX/labels-$n.json" ]; then cat "$GH_STUB_FX/labels-$n.json"; else echo '[]'; fi ;;
  *"/pulls/"*) cat "$GH_STUB_FX/pull-${*##*/}.json" ;;
  *"/actions/runs/"[0-9]*)
    id="${*##*/}"
    [ ! -f "$GH_STUB_FX/run-$id.down" ] || { echo "HTTP 502: run $id did not answer" >&2; exit 1; }
    if [ -f "$GH_STUB_FX/run-$id.json" ]; then cat "$GH_STUB_FX/run-$id.json"; else printf '{"id":%s,"status":"completed","conclusion":"cancelled"}' "$id"; fi ;;
  *"/actions/workflows/validate.yml/runs?"*)
    sha="${*#*head_sha=}"; sha="${sha%%&*}"; cat "$GH_STUB_FX/vruns-$sha.json" ;;
  *) echo "stub gh: no answer for: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"

rp() {
  : > "$LOG"
  env -u GITHUB_ACTIONS -u GITHUB_STEP_SUMMARY -u GITHUB_REPOSITORY \
    PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" MJ_RELEASE_PATH_CANCEL_WAIT=0 \
    "$RP" "$@" --repo o/r
}
# one run, as GitHub lists it, cut to what the enforcer reads
run() { # <id> <workflow file> <event> <branch> <status> [pr] [actor]
  local pr=""
  [ -z "${6:-}" ] || pr="{\"number\":$6}"
  printf '{"id":%s,"path":".github/workflows/%s","event":"%s","head_branch":"%s","status":"%s","html_url":"https://example.invalid/runs/%s","triggering_actor":{"login":"%s"},"pull_requests":[%s]}' \
    "$1" "$2" "$3" "$4" "$5" "$1" "${7:-someone}" "$pr"
}
runs() { # <status> <run json>... : the page GitHub answers for that status
  local s="$1"; shift
  local IFS=,
  printf '{"workflow_runs":[%s]}' "$*" > "$FX/runs-$s.json"
}
reset() { rm -f "$FX"/*; }
wrote() { grep -c -- "$1" "$LOG" || true; }

# ---------------------------------------------------------------- 1. clear
reset
runs in_progress "$(run 201 validate.yml pull_request feature/a in_progress 7)" \
                 "$(run 203 validate.yml push master in_progress)"
expect_exit 0 rp status || { echo "    a pool holding only branch and master validates was not clear"; exit 1; }
expect_grep '^clear ' || { echo "    status did not say clear"; exit 1; }
expect_exit 0 rp yield --event pull_request --pr 5 --branch feature/x --self 900 \
  || { echo "    yield failed while the path was clear"; exit 1; }
expect_grep 'this run proceeds' || { echo "    a clear path did not let the run proceed"; exit 1; }
[ "$(wrote '^api -X')" = 0 ] || { echo "    a clear path wrote to the forge:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 2. what the release path is
busy_with() { # <run json> <phrase it must be named by>
  reset
  runs queued "$1"
  expect_exit 10 rp status || { echo "    status was not busy with: $1"; exit 1; }
  expect_grep "^busy   $2" || { echo "    status did not name the run as /$2/"; exit 1; }
}
busy_with "$(run 101 release.yml push v9.9.9 queued)" 'the release run of v9.9.9, run 101 \(queued\)'
busy_with "$(run 102 validate.yml workflow_dispatch release/record-v9.9.9 queued)" 'the record verdict on release/record-v9.9.9, run 102'
busy_with "$(run 103 pages.yml workflow_dispatch master queued '' 'github-actions[bot]')" 'the release deploy of master, run 103'
# a run waiting in a concurrency group is asked about too
reset; runs pending "$(run 104 release.yml push v9.9.9 pending)"
expect_exit 10 rp status || { echo "    a pending release run was not seen"; exit 1; }
not_busy() { # <run json> <what it is>
  reset
  runs in_progress "$1"
  expect_exit 0 rp status || { echo "    $2 made the release path busy"; exit 1; }
}
not_busy "$(run 110 pages.yml push master in_progress)" "a pages run of a push"
not_busy "$(run 111 pages.yml workflow_dispatch master in_progress '' korczis)" "a pages run a person dispatched"
not_busy "$(run 112 validate.yml push master in_progress)" "master's validate"
not_busy "$(run 113 docker-image.yml workflow_run master in_progress)" "another workflow"
# the caller is never its own reason
reset; runs in_progress "$(run 900 release.yml push v9.9.9 in_progress)"
expect_exit 0 rp status --self 900 || { echo    "--self did not exclude the caller"; exit 1; }

# ---------------------------------------------------------------- 3. a pull request's run yields
reset
runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)" "$(run 900 validate.yml pull_request feature/x in_progress 5)"
expect_exit 0 rp yield --event pull_request --pr 5 --branch feature/x --self 900 || { echo "    yield did not exit 0"; exit 1; }
expect_grep 'yields to the release run of v9.9.9, run 101' || { echo "    the yield does not name the run it yielded to"; exit 1; }
[ "$(wrote '^api -X POST repos/o/r/issues/5/labels -f labels\[\]=ci:yielded$')" = 1 ] || { echo "    the pull request was not labelled ci:yielded:"; cat "$LOG"; exit 1; }
grep -q '^api -X POST repos/o/r/issues/5/comments -f body=.*run 101' "$LOG" || { echo "    the pull request was not told which run it yielded to:"; cat "$LOG"; exit 1; }
[ "$(wrote '^api -X POST repos/o/r/actions/runs/900/cancel$')" = 1 ] || { echo "    the run did not cancel itself:"; cat "$LOG"; exit 1; }
[ "$(wrote '/cancel$')" = 1 ] || { echo "    yield cancelled something other than its own run:"; cat "$LOG"; exit 1; }
for ev in push schedule workflow_dispatch; do
  expect_exit 0 rp yield --event "$ev" --pr 5 --branch master --self 900 || exit 1
  expect_grep 'never yields' || { echo    "a $ev run did not refuse to yield"; exit 1; }
  [ "$(wrote '^api')" = 0 ] || { echo "    a $ev run asked or wrote the forge:"; cat "$LOG"; exit 1; }
done
expect_exit 0 rp yield --event pull_request --pr 9 --branch release/record-v9.9.9 --self 901 || exit 1
expect_grep 'record branch' || { echo "    a record branch's run yielded to its own release"; exit 1; }
[ "$(wrote '^api -X')" = 0 ] || { echo "    a record branch's run wrote to the forge"; exit 1; }

# ---------------------------------------------------------------- 4. the tag push preempts
reset
runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)" \
                 "$(run 201 validate.yml pull_request feature/a in_progress 7)" \
                 "$(run 202 validate.yml pull_request release/record-v9.9.9 in_progress 8)" \
                 "$(run 203 validate.yml push master in_progress)" \
                 "$(run 204 validate.yml pull_request fork-branch in_progress)"
runs queued "$(run 205 validate.yml pull_request feature/b queued 9)" "$(run 900 release-path.yml push v9.9.9 queued)"
expect_exit 0 rp preempt --self 900 || { echo "    preempt did not exit 0"; exit 1; }
cancelled="$(sed -n 's#^api -X POST repos/o/r/actions/runs/\([0-9]*\)/cancel$#\1#p' "$LOG" | LC_ALL=C sort | tr '\n' ' ')"
[ "$cancelled" = "201 205 " ] || { echo "    preempt cancelled '$cancelled', not the two branch runs 201 and 205:"; cat "$LOG"; exit 1; }
for pr in 7 9; do
  grep -q "^api -X POST repos/o/r/issues/$pr/labels" "$LOG" || { echo "    #$pr was not labelled"; exit 1; }
  grep -q "^api -X POST repos/o/r/issues/$pr/comments -f body=.*the release run of v9.9.9, run 101" "$LOG" \
    || { echo "    #$pr was not told which run it yielded to"; exit 1; }
done
grep -q 'issues/8/' "$LOG" && { echo "    the record's own pull request was touched"; exit 1; }
expect_grep '^left   validate run 204' || { echo "    a run with no pull request was not named as left alone"; exit 1; }
expect_grep '2 pull-request validate run\(s\) yielded' || { echo "    preempt did not count what yielded"; exit 1; }
[ "$(wrote '/force-cancel$')" = 0 ] || { echo "    preempt force-cancelled a run whose cancel had completed:"; cat "$LOG"; exit 1; }
# A cancel GitHub accepted can leave a job running (2026-10-19: a suite job held its slot until
# force-cancel). After the wait each cancelled run is read again: one still in progress is
# force-cancelled after its cancel, one that completed is not, one that cannot be read is left.
reset
runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)" \
                 "$(run 201 validate.yml pull_request feature/a in_progress 7)" \
                 "$(run 205 validate.yml pull_request feature/b in_progress 9)" \
                 "$(run 206 validate.yml pull_request feature/c in_progress 10)"
printf '{"id":201,"status":"in_progress","conclusion":null}' > "$FX/run-201.json"
: > "$FX/run-206.down"
expect_exit 0 rp preempt --self 900 || { echo "    preempt did not exit 0 when a cancel did not land"; exit 1; }
forced="$(sed -n 's#^api -X POST repos/o/r/actions/runs/\([0-9]*\)/force-cancel$#\1#p' "$LOG" | tr '\n' ' ')"
[ "$forced" = "201 " ] || { echo "    preempt force-cancelled '$forced', not the one run still in progress, 201:"; cat "$LOG"; exit 1; }
awk '$0 == "-X POST repos/o/r/actions/runs/201/cancel" {c=NR} $0 == "repos/o/r/actions/runs/201" {r=NR}
     $0 == "-X POST repos/o/r/actions/runs/201/force-cancel" {f=NR} END {exit !(c && r && f && c < r && r < f)}' \
  <(sed 's/^api //' "$LOG") || { echo "    run 201 was not cancelled, read again, then force-cancelled, in that order:"; cat "$LOG"; exit 1; }
expect_grep '^force  validate run 201 was still in_progress' || { echo "    the force-cancel was not said"; exit 1; }
expect_grep 'validate run 206 could not be read after its cancel' || { echo "    a run that could not be read was not named as left"; exit 1; }
reset; runs in_progress "$(run 201 validate.yml pull_request feature/a in_progress 7)"
expect_exit 0 rp preempt --self 900 || exit 1
[ "$(wrote '^api -X')" = 0 ] || { echo "    preempt touched branch runs with no release path running"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 5. resume
reset
runs in_progress "$(run 102 validate.yml workflow_dispatch release/record-v9.9.9 in_progress)"
printf '[{"number":7,"pull_request":{}}]' > "$FX/issues.json"
expect_exit 0 rp resume --self 900 || exit 1
expect_grep 'still holds: the record verdict' || { echo    "resume did not say what still holds the path"; exit 1; }
[ "$(wrote '^api -X')" = 0 ] || { echo "    resume re-requested while the path was busy"; cat "$LOG"; exit 1; }
reset
printf '[{"number":7,"pull_request":{}},{"number":8,"pull_request":{}},{"number":3}]' > "$FX/issues.json"
printf '{"head":{"sha":"aaa111"}}' > "$FX/pull-7.json"
printf '{"head":{"sha":"bbb222"}}' > "$FX/pull-8.json"
printf '{"workflow_runs":[{"id":200,"status":"completed","conclusion":"failure","created_at":"2026-10-14T10:00:00Z"},{"id":201,"status":"completed","conclusion":"cancelled","created_at":"2026-10-14T11:00:00Z"}]}' > "$FX/vruns-aaa111.json"
printf '{"workflow_runs":[{"id":205,"status":"completed","conclusion":"cancelled","created_at":"2026-10-14T11:00:00Z"},{"id":206,"status":"in_progress","conclusion":null,"created_at":"2026-10-14T12:00:00Z"}]}' > "$FX/vruns-bbb222.json"
expect_exit 0 rp resume --self 900 || exit 1
[ "$(wrote '/rerun$')" = 1 ] && [ "$(wrote '^api -X POST repos/o/r/actions/runs/201/rerun$')" = 1 ] \
  || { echo "    resume did not re-request exactly the cancelled newest run 201:"; cat "$LOG"; exit 1; }
for pr in 7 8; do
  [ "$(wrote "^api -X DELETE repos/o/r/issues/$pr/labels/ci%3Ayielded$")" = 1 ] || { echo "    #$pr kept its label"; cat "$LOG"; exit 1; }
done
grep -q 'issues/3/' "$LOG" && { echo "    an issue that is not a pull request was touched"; exit 1; }
expect_grep '^drop   #8: its newest validate run 206 is in_progress' || { echo "    #8 was not said to need nothing"; exit 1; }

# ---------------------------------------------------------------- 6. an unreadable forge decides nothing
reset; runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)"; : > "$FX/down"
expect_exit 12 rp status || { echo "    an unreadable forge was not 12"; exit 1; }
expect_exit 0 rp yield --event pull_request --pr 5 --branch feature/x --self 900 || exit 1
expect_grep 'this run proceeds without asking' || { echo "    an unreadable forge did not let the run proceed, saying so"; exit 1; }
expect_exit 0 rp preempt || exit 1
expect_exit 0 rp resume || exit 1
expect_exit 2 "$RP" bogus || { echo "    an unknown command was not a usage error"; exit 1; }

# ---------------------------------------------------------------- 7. the wiring
# validate.yml: the plan job's step, asked before the plan, on a pull request alone, by its own id
plan_job="$(awk '$0 == "  plan:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$V")"
printf '%s\n' "$plan_job" | awk '/name: the release path goes first/ {y=NR} /- id: plan$/ {p=NR} END {exit !(y && p && y < p)}' \
  || { echo "    validate.yml's plan job does not ask the release path before it plans"; exit 1; }
printf '%s\n' "$plan_job" | grep -A1 'name: the release path goes first' | grep -qx "        if: github.event_name == 'pull_request'" \
  || { echo "    the yield step is not limited to a pull request's run"; exit 1; }
printf '%s\n' "$plan_job" | grep -qE 'run: scripts/ci/release-path yield --event "\$GITHUB_EVENT_NAME" .*--self "\$GITHUB_RUN_ID"$' \
  || { echo "    the yield step does not pass the event and its own run id"; exit 1; }
ci_job="$(awk '$0 == "  ci:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$V")"
printf '%s\n' "$ci_job" | grep -A1 'name: the runs that yielded to the release path run again' \
  | grep -qx "        if: always() && github.event_name != 'pull_request'" \
  || { echo "    the ci job does not resume on every run that is not a pull request's"; exit 1; }
awk '/^permissions:/{f=1; next} /^[a-z]/{f=0} f' "$V" | grep -qx '  pull-requests: write' \
  || { echo "    validate.yml cannot label a pull request"; exit 1; }
# release-path.yml: preempt on the tag push, resume on the schedule the model declares
[ -f "$WF" ] || { echo "    .github/workflows/release-path.yml is missing"; exit 1; }
tags="$(sed -n "s/^  tags: *'\(.*\)'.*/\1/p" "$MODEL")"
cron="$(sed -n "s/^  schedule: *'\(.*\)'.*/\1/p" "$MODEL")"
grep -qFx "      - '$tags'" "$WF" || { echo "    release-path.yml does not start on the tags the model declares ($tags)"; exit 1; }
grep -qFx "    - cron: '$cron'" "$WF" || { echo "    release-path.yml does not resume on the schedule the model declares ($cron)"; exit 1; }
grep -B4 'run: scripts/ci/release-path preempt' "$WF" | grep -q "if: github.event_name == 'push'" \
  || { echo "    release-path.yml does not preempt on the tag push"; exit 1; }
grep -q 'run: scripts/ci/release-path resume' "$WF" || { echo "    release-path.yml does not resume"; exit 1; }
# the enforcer cancels by one call only, and the workflow names no run of its own to cancel
[ "$(grep -c '/cancel"' "$RP")" = 2 ] || { echo "    release-path cancels from more places than yield and preempt"; exit 1; }
[ "$(grep -c '/force-cancel"' "$RP")" = 1 ] || { echo "    release-path force-cancels from more places than preempt"; exit 1; }

# ---------------------------------------------------------------- 8. a train never yields
exempt="$(sed -n 's/^  exempt_label: *//p' "$MODEL")"
[ "$exempt" = train ] || { echo "    the model's exempt_label is '$exempt', not train"; exit 1; }
reset
runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)" "$(run 900 validate.yml pull_request integration/train-14-20261101 in_progress 5)"
printf '[{"name":"type:chore"},{"name":"train"}]' > "$FX/labels-5.json"
expect_exit 0 rp yield --event pull_request --pr 5 --branch integration/train-14-20261101 --self 900 || exit 1
expect_grep 'labelled train, a train, and a train never yields; this run proceeds' \
  || { echo "    a train's run did not say it proceeds"; exit 1; }
grep -qx 'api repos/o/r/issues/5/labels' "$LOG" || { echo "    yield did not read the pull request's labels:"; cat "$LOG"; exit 1; }
[ "$(wrote '^api -X')" = 0 ] || { echo "    a train's run labelled, told or cancelled:"; cat "$LOG"; exit 1; }
# a member, labelled hold, still yields
printf '[{"name":"hold"}]' > "$FX/labels-5.json"
expect_exit 0 rp yield --event pull_request --pr 5 --branch feature/x --self 900 || exit 1
[ "$(wrote '^api -X POST repos/o/r/actions/runs/900/cancel$')" = 1 ] || { echo "    a member did not yield:"; cat "$LOG"; exit 1; }
# labels that cannot be read exempt nothing: the run yields as before, and says why
rm -f "$FX/labels-5.json"; : > "$FX/labels-5.down"
expect_exit 0 rp yield --event pull_request --pr 5 --branch integration/train-14-20261101 --self 900 || exit 1
expect_grep 'could not be read.*it is not known to be a train, so this run yields as before' \
  || { echo "    unreadable labels were not said"; exit 1; }
[ "$(wrote '^api -X POST repos/o/r/actions/runs/900/cancel$')" = 1 ] \
  || { echo "    a run whose labels could not be read did not yield:"; cat "$LOG"; exit 1; }
# preempt: the train's run is left, the member's is cancelled, and so is an unreadable one
reset
runs in_progress "$(run 101 release.yml push v9.9.9 in_progress)" \
                 "$(run 201 validate.yml pull_request integration/train-14-20261101 in_progress 7)" \
                 "$(run 205 validate.yml pull_request feature/b in_progress 9)" \
                 "$(run 206 validate.yml pull_request feature/c in_progress 10)"
printf '[{"name":"train"}]' > "$FX/labels-7.json"
printf '[{"name":"hold"}]' > "$FX/labels-9.json"
: > "$FX/labels-10.down"
expect_exit 0 rp preempt --self 900 || exit 1
cancelled="$(sed -n 's#^api -X POST repos/o/r/actions/runs/\([0-9]*\)/cancel$#\1#p' "$LOG" | tr '\n' ' ')"
[ "$cancelled" = "205 206 " ] || { echo "    preempt cancelled '$cancelled', not the runs 205 and 206:"; cat "$LOG"; exit 1; }
grep -q 'issues/7/comments\|issues/7/labels -f' "$LOG" && { echo "    the train was labelled or told"; cat "$LOG"; exit 1; }
expect_grep '^left   validate run 201 of #7 on integration/train-14-20261101 \(in_progress\): labelled train, a train, which never yields$' \
  || { echo "    the train's run was not named as left"; exit 1; }
expect_grep 'validate run 206 of #10: the labels of pull request #10 could not be read.*it is not known to be a train, so it yields as before' \
  || { echo "    a run whose labels could not be read was not named"; exit 1; }
expect_grep '2 pull-request validate run\(s\) yielded' || { echo "    preempt did not count the two that yielded"; exit 1; }

echo "    the release run, the record's verdict and the release's deploy make the path busy and nothing else does; a pull request's run yields to them by labelling, telling and cancelling itself, a tag push preempts the branch runs and force-cancels one whose cancel did not land, master, dispatches, record branches and trains never yield, the yielded runs are re-requested once the path is clear, an unreadable forge decides nothing, and labels that cannot be read exempt nothing"
