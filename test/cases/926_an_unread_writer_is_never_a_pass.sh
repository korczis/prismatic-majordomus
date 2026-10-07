# majordomus-covers: none
# A check run of a context bound to an app whose writer was not read is never taken for the
# bound app's: the pull request is unknown, nothing merges, and a refresh that reads the
# writer heals it (ADR 0101; rule project.integration-follows-the-current-master). Through the
# real command line against a scripted forge, as case 857 does. The base binds `ci` to app
# 15368, the live shape of the protection. `gh pr list` reports #1's `ci` as a passing check
# run and names no app, as gh does; who wrote it comes only from `gh api graphql`. #2 is the
# control: its `ci` is the bound app's and still running, so it waits for checks in every
# section, which shows the writers were read and applied pull request by pull request.
#
#   1. the head moved between the two reads: the writers answer #1 under another head, with a
#      passing `ci` from the bound app. That answer is of another commit and is not applied:
#      #1 is unknown with reason required_checks:unknown, nothing is next, the queue's
#      diagnostics name the pull request and status exits 10, and `prs drain --max 1` asks
#      the forge for no merge
#   2. #1's contexts did not fit a page (hasNextPage true), in the page read and in the read
#      of that one pull request that follows it: the same, though the part that was listed
#      holds a passing `ci` from the bound app. Only the truncated pull request is asked for
#      alone, once
#   3. the forge refuses the writers (403): `prs refresh` exits 12 after one ask, the stored
#      observation is byte for byte what it was, status still answers from it, and a drain
#      exits 12 without a merge. A read that failed is never replaced by the list's checks
#   4. the writers answered whole, for the head the list gave: #1 is ready and next, and the
#      queue says nothing of unread writers. The state heals on a refresh; nothing sticks
#
# The scripted forge answers the writers read only as the adapter calls it: owner, name and
# the query as raw strings (-f), the page size or the number typed (-F), newest first.
# Anything else, a merge included, is logged UNEXPECTED and refused.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-926.git"; W="$T/../work-926"; STATE="$T/../forge-926"; BIN="$T/../bin-926"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
# the person's identity, in the clone: the executor records and commits here, and a machine
# with none configured (a CI runner) must not decide the outcome
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
for n in 1 2; do
  gitq checkout -qb "feature/$n" master; echo "$n" > "$n.txt"; gitq add "$n.txt"; gitq commit -qm "$n"
  gitq push -q origin "HEAD:refs/heads/feature/$n" "HEAD:refs/pull/$n/head"
done
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }
# a commit that is neither head: what the forge answers when #1 moved under the read
MOVED="1111111111111111111111111111111111111111"
[ "$MOVED" != "$H1" ] || { echo "    the fixture's other head is #1's own"; exit 1; }

# the list, as gh's own projection of the rollup gives it: no entry names an app
ci_listed='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","startedAt":"2026-09-01T00:00:00Z","completedAt":"2026-09-01T00:05:00Z","detailsUrl":"https://example.com/run/1","workflowName":"validate"}'
ci_running='{"__typename":"CheckRun","name":"ci","status":"IN_PROGRESS","conclusion":"","startedAt":"2026-09-01T00:00:00Z","completedAt":"0001-01-01T00:00:00Z","detailsUrl":"https://example.com/run/2","workflowName":"validate"}'
pr() {   # <number> <head> <rollup>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$2" "$1" "$1" "$3"
}
printf '[%s,%s]\n' "$(pr 2 "$H2" "$ci_running")" "$(pr 1 "$H1" "$ci_listed")" > "$STATE/prs.json"
echo '{"required_status_checks":{"checks":[{"context":"ci","app_id":15368}],"contexts":["ci"]}}' > "$STATE/protection.json"

# the writers, as the forge's GraphQL rollup answers: each check run with the app of its suite
by_actions='"checkSuite":{"app":{"databaseId":15368}}}'
ci_passed='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","startedAt":"2026-09-01T00:00:00Z","completedAt":"2026-09-01T00:05:00Z",'"$by_actions"
ci_pending='{"__typename":"CheckRun","name":"ci","status":"IN_PROGRESS","conclusion":null,"startedAt":"2026-09-01T00:00:00Z","completedAt":null,'"$by_actions"
written() {   # <number> <head> <more follow: true|false> <contexts>
  printf '{"number":%s,"headRefOid":"%s","statusCheckRollup":{"contexts":{"pageInfo":{"hasNextPage":%s,"endCursor":null},"nodes":[%s]}}}' \
    "$1" "$2" "$3" "$4"
}
writers() {   # <#1's node>: the one page of writers, newest first, #2 always whole and running
  printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s,%s]}}}}\n' \
    "$(written 2 "$H2" false "$ci_pending")" "$1" > "$STATE/writers.json"
}

# the forge. writers.json is the page read's answer; writers-1.json the answer to the read of
# #1 alone, which nobody may ask for while it is absent; writers-refused makes both a 403
cat > "$BIN/gh" <<EOF
#!/bin/sh
S="$STATE"; O="$ORIGIN"
EOF
cat >> "$BIN/gh" <<'EOF'
echo "$*" >> "$S/log"; echo "$*" >> "$S/log.all"
unexpected() {
  echo "UNEXPECTED: $1" >> "$S/log"; echo "UNEXPECTED: $1" >> "$S/log.all"
  echo "the scripted forge does not answer this: $1" >&2; exit 1
}
case " $* " in *" --admin "*) unexpected "a call with --admin" ;; esac
case "$1 $2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "$(git -C "$O" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") cat "$S/protection.json" ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state open "*) cat "$S/prs.json" ;;
      *" --state closed "*) echo '[]' ;;
      *) unexpected "a list that is neither the open nor the closed pull requests" ;;
    esac ;;
  "api graphql")
    # the writers read, argument by argument: raw strings for the query, the owner and the
    # name, one typed value, and no cursor (nothing here has a second page to ask for)
    [ "$3 $5 $6 $7 $8 $9" = "-f -f owner=o -f name=r -F" ] || unexpected "the writers read's arguments"
    [ "$#" = 10 ] || unexpected "a writers read of $# arguments: no page follows another here"
    case "${10}" in
      n=50)
        case "$4" in
          'query=query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){'*'statusCheckRollup{contexts(first:100){pageInfo{hasNextPage endCursor}'*'checkSuite{app{databaseId}}'*) ;;
          *) unexpected "the query of the page read" ;;
        esac
        if [ -f "$S/writers-refused" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
        cat "$S/writers.json" ;;
      number=1)
        case "$4" in
          'query=query($owner:String!,$name:String!,$number:Int!,$after:String){repository(owner:$owner,name:$name){pullRequest(number:$number){'*'statusCheckRollup{contexts(first:100,after:$after){pageInfo{hasNextPage endCursor}'*'checkSuite{app{databaseId}}'*) ;;
          *) unexpected "the query of the read of one pull request" ;;
        esac
        if [ -f "$S/writers-refused" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
        [ -f "$S/writers-1.json" ] || unexpected "a read of #1 alone, whose contexts fitted the page"
        cat "$S/writers-1.json" ;;
      *) unexpected "a writers read typed ${10}" ;;
    esac ;;
  *) unexpected "$1 $2" ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
: > "$STATE/log"; : > "$STATE/log.all"

# the queue status prints, and the status it exits with: 10 says the queue is less than a
# full answer, and its diagnostics say why
queue() { rc=0; q="$(prs status --format json 2>/dev/null)" || rc=$?; [ -n "$q" ] || { echo "    status printed no queue (exit $rc)"; exit 1; }; }
look() {
  : > "$STATE/log"
  prs refresh >/dev/null 2>"$STATE/refresh.err" || { echo "    refresh failed:"; tail -3 "$STATE/refresh.err"; tail -5 "$STATE/log" | cut -c1-160; exit 1; }
  queue
}
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
asked() { grep -c -- "$1" "$STATE/log" || :; }   # how many calls of the section match
said() { printf '%s' "$q" | jq -r '.diagnostics[]'; }

# unread <when>: #1's writer was not read, so it is unknown and says so; #2 is unaffected
unread() {
  [ "$(field 1 .disposition)" = unknown ] \
    || { echo "    $1: #1 is $(field 1 .disposition), not unknown"; field 1 '{reasons, evidence}'; exit 1; }
  [ "$(field 1 '.reasons[0]')" = required_checks:unknown ] \
    || { echo "    $1: #1's reason is $(field 1 '.reasons[0]'), not required_checks:unknown"; exit 1; }
  [ "$(field 1 .required_checks)" = unknown ] || { echo "    $1: #1's required checks are $(field 1 .required_checks), not unknown"; exit 1; }
  detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
  case "$detail" in *"ci (app 15368): unknown"*) ;; *) echo "    $1: #1's evidence does not say the bound check is unknown: $detail"; exit 1 ;; esac
  case "$(field 1 '.next_action // ""')" in *"majordomus prs refresh"*) ;;
    *) echo "    $1: #1's next action does not name the refresh: $(field 1 '.next_action // ""')"; exit 1 ;; esac
  [ "$(field 2 .disposition)" = waiting_for_checks ] \
    || { echo "    $1: #2 (the bound app's run, still running) is $(field 2 .disposition), not waiting_for_checks"; field 2 '{reasons, evidence}'; exit 1; }
  [ "$(printf '%s' "$q" | jq -r .next_merge)" = null ] || { echo "    $1: #$(printf '%s' "$q" | jq -r .next_merge) is next"; exit 1; }
  said | grep -q "1 pull request(s) carry a check run of ci (app 15368) whose app was not read (#1)" \
    || { echo "    $1: the diagnostics do not name the unread writer and its pull request:"; said | cut -c1-200; exit 1; }
  [ "$rc" = 10 ] || { echo "    $1: status exited $rc over a queue with an unread writer, not 10"; exit 1; }
}
# unmerged <when>: a drain finds nothing ready and asks the forge for no merge
unmerged() {
  drc=0; out="$(prs drain --max 1 2>&1)" || drc=$?
  [ "$drc" = 0 ] || { echo "    $1: the drain exited $drc: $out"; tail -5 "$STATE/log" | cut -c1-160; exit 1; }
  case "$out" in *"nothing is ready"*) ;; *) echo "    $1: the drain did not stop because nothing was ready: $out"; exit 1 ;; esac
  if grep -q '^pr merge' "$STATE/log.all"; then echo "    $1: the drain asked the forge to merge:"; grep '^pr merge' "$STATE/log.all"; exit 1; fi
}

# ---------------------------------------------------------------- 1. the head moved between the reads
writers "$(written 1 "$MOVED" false "$ci_passed")"
look
unread "a head that moved"
[ "$(asked '^api graphql .* -f owner=o -f name=r -F n=50$')" = 1 ] \
  || { echo "    the writers were not read once, as the adapter reads them:"; grep '^api' "$STATE/log" | cut -c1-120; exit 1; }
[ "$(asked ' -F number=')" = 0 ] || { echo "    a pull request whose contexts fitted the page was read alone"; exit 1; }
unmerged "a head that moved"

# ---------------------------------------------------------------- 2. contexts that do not fit
# more follow and no cursor says where: unreadable, in the page read and in the read of #1
writers "$(written 1 "$H1" true "$ci_passed")"
printf '{"data":{"repository":{"pullRequest":%s}}}\n' "$(written 1 "$H1" true "$ci_passed")" > "$STATE/writers-1.json"
look
unread "contexts that did not fit"
[ "$(asked '^api graphql .* -f owner=o -f name=r -F number=1$')" = 1 ] \
  || { echo "    #1, truncated, was not read alone exactly once:"; grep '^api' "$STATE/log" | cut -c1-40; grep -c ' -F number=' "$STATE/log" || :; exit 1; }
[ "$(asked ' -F number=')" = 1 ] || { echo "    a pull request other than the truncated one was read alone"; exit 1; }
unmerged "contexts that did not fit"

# ---------------------------------------------------------------- 3. the writers refused
# the answer that would pass #1 is in place: a read that fails must not fall back to anything
OBS="$W/.ai/local/state/integration/observation.json"
[ -f "$OBS" ] || { echo "    no stored observation at $OBS"; exit 1; }
before="$(cksum < "$OBS")"
writers "$(written 1 "$H1" false "$ci_passed")"; rm -f "$STATE/writers-1.json"
touch "$STATE/writers-refused"
: > "$STATE/log"
expect_exit 12 prs refresh
expect_grep 'HTTP 403'
[ "$(asked '^api graphql ')" = 1 ] || { echo "    a refusal was asked for $(asked '^api graphql ') time(s), not once: it is an answer, not an outage"; exit 1; }
[ "$(cksum < "$OBS")" = "$before" ] || { echo "    a refresh whose writers read was refused rewrote the stored observation"; exit 1; }
queue
unread "the writers refused, read from the stored observation"
expect_exit 12 prs drain --max 1
if grep -q '^pr merge' "$STATE/log.all"; then echo "    a drain that could not read the writers asked for a merge"; exit 1; fi
[ "$(cksum < "$OBS")" = "$before" ] || { echo "    a drain whose writers read was refused rewrote the stored observation"; exit 1; }

# ---------------------------------------------------------------- 4. the answer corrected
rm -f "$STATE/writers-refused"
look
[ "$(field 1 .disposition)" = ready ] \
  || { echo "    #1, its writer read as the bound app, is $(field 1 .disposition), not ready"; field 1 '{reasons, evidence}'; exit 1; }
[ "$(field 1 .required_checks)" = passed ] || { echo "    #1's required checks are $(field 1 .required_checks), not passed"; exit 1; }
detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci (app 15368): passed"*) ;; *) echo "    #1's evidence does not say the bound check passed: $detail"; exit 1 ;; esac
[ "$(field 2 .disposition)" = waiting_for_checks ] || { echo "    #2 is $(field 2 .disposition), not waiting_for_checks"; exit 1; }
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 1 ] || { echo "    the next merge is not #1 once its writer was read"; exit 1; }
if said | grep -q "was not read"; then echo "    the queue still speaks of an unread writer:"; said | cut -c1-200; exit 1; fi
[ "$rc" = 0 ] || { echo "    status exited $rc over a queue with every writer read:"; said | cut -c1-200; exit 1; }
[ "$(asked ' -F number=')" = 0 ] || { echo "    a pull request was read alone though every rollup fitted the page"; exit 1; }

if grep -q '^pr merge' "$STATE/log.all"; then echo "    a merge reached the forge:"; grep '^pr merge' "$STATE/log.all"; exit 1; fi
if grep -q UNEXPECTED "$STATE/log.all"; then
  echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log.all" | cut -c1-160 | head -6; exit 1
fi
echo "    a head that moved, contexts that did not fit, a refused read: unknown, never merged; healed by one refresh"
