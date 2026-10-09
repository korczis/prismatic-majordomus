# majordomus-covers: none
# The base's required checks are its branch protection and its rulesets together, bound to the
# app that must write them, and a base that requires nothing proves nothing (ADR 0101, owner
# decision D5). Through the real command line against a scripted forge, as case 720 does:
#
#   1. the protection requires `ci`; a ruleset adds `lint`, bound to app 15368. A context is
#      bound, so the refresh asks the forge who wrote each check run: one `gh api graphql`
#      read, the first page of the open pull requests, with the owner and the name as strings
#      (`-f`) and only the page size typed (`-F n=50`). The scripted forge answers that read
#      argument for argument, the declarations read every refresh makes (ADR 0101 §6, D4),
#      and nothing else of `api graphql`. #1 reports `lint` as a commit
#      status (another writer), so `lint` has not reported: it waits for checks. #2 reports
#      `lint` as a check run app 15368 wrote: ready, because the writer was read and is the
#      bound app, not because nobody asked: the recorded observation carries the app, and the
#      evidence names each context with its verdict.
#   1b. the same forge, and the writer of #2's `lint` is now app 99: the run is not the bound
#      app's, so `lint` is missing, #2 waits for checks and nothing is next. What made #2
#      ready in 1 was the writer and nothing else.
#   2. a base with no protection and no ruleset requires nothing: every pull request is
#      unknown with reason no_required_checks, nothing is next, and the diagnostics say why
#   3. a ruleset read the forge refuses leaves the requirements unread (checks and reviews,
#      both of which rulesets can carry): unknown, never ready
#   4. the bases of 2 and 3 bind nothing, and no writer was asked for there; the forge was
#      asked nothing this harness does not answer
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-857.git"; W="$T/../work-857"; STATE="$T/../forge-857"; BIN="$T/../bin-857"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
# the person's identity, in the clone: a machine with none configured (a CI runner) must not
# decide the outcome
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

ci='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:05:00Z"}'
lint_status='{"__typename":"StatusContext","context":"lint","state":"SUCCESS","startedAt":"2026-09-01T00:06:00Z"}'
lint_run='{"__typename":"CheckRun","name":"lint","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:06:00Z"}'
pr() {   # <number> <head> <rollup>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$2" "$1" "$1" "$3"
}
printf '[%s,%s]\n' "$(pr 1 "$H1" "$ci,$lint_status")" "$(pr 2 "$H2" "$ci,$lint_run")" > "$STATE/prs.json"
# who wrote each check run, as the forge's GraphQL rollup answers: the same entries, each
# check run with the app of its suite
by_actions='"checkSuite":{"app":{"databaseId":15368}}}'
ci_written="${ci%\}},$by_actions"; lint_written="${lint_run%\}},$by_actions"
written() {   # <number> <head> <contexts>
  printf '{"number":%s,"headRefOid":"%s","statusCheckRollup":{"contexts":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}' \
    "$1" "$2" "$3"
}
writers() {   # <#2's contexts>: the one page of the open pull requests, newest first
  printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s,%s]}}}}\n' \
    "$(written 2 "$H2" "$1")" "$(written 1 "$H1" "$ci_written,$lint_status")" > "$STATE/writers.json"
}
writers "$ci_written,$lint_written"
# the query of that read, as the adapter sends it (forge.rs WRITERS_QUERY): one line
# shellcheck disable=SC2016  # GraphQL variables, not the shell's
printf '%s' 'query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor}nodes{number headRefOid statusCheckRollup{contexts(first:100){pageInfo{hasNextPage endCursor}nodes{__typename ...on CheckRun{name status conclusion startedAt completedAt checkSuite{app{databaseId}}}...on StatusContext{context state startedAt:createdAt}}}}}}}}' > "$STATE/writers.query"
echo '{"required_status_checks":{"contexts":["ci"],"checks":[{"context":"ci"}]}}' > "$STATE/protection.json"
echo '[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"lint","integration_id":15368}]}}]' > "$STATE/rules.json"

# the forge: protection.json absent is an unprotected branch; rules.json holding FAIL is a
# refusal. Of `api graphql` it answers the declarations read (every open pull request an
# owner's, a branch of this repository, mentioned by none) and one writers call only: the
# first page, each argument where the adapter puts it. A second page (`-f after=`), the read of one pull
# request (`-F number=`), a typed owner or name (`-F owner=`) or another query is UNEXPECTED.
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection")
    if [ -f "$STATE/protection.json" ]; then cat "$STATE/protection.json"
    else echo '{"message":"Branch not protected"}'; echo 'gh: Branch not protected (HTTP 404)' >&2; exit 1; fi ;;
  "api repos/o/r/rules/branches/master")
    if grep -q FAIL "$STATE/rules.json"; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    cat "$STATE/rules.json" ;;
  "pr list") cat "$STATE/prs.json" ;;
  "api graphql")
    case "\$4" in *timelineItems*) jq -c '{data:{repository:{pullRequests:{pageInfo:{hasNextPage:false,endCursor:null},nodes:[.[]|{number,authorAssociation:"OWNER",isCrossRepository:false,timelineItems:{pageInfo:{hasNextPage:false,endCursor:null},nodes:[]}}]}}}}' "$STATE/prs.json"; exit 0 ;; esac
    if [ "\$#" = 10 ] && [ "\$3 \$5 \$6 \$7 \$8 \$9 \${10}" = "-f -f owner=o -f name=r -F n=50" ] \\
      && [ "\$4" = "query=\$(cat "$STATE/writers.query")" ]; then cat "$STATE/writers.json"
    else echo "UNEXPECTED" >> "$STATE/log"; exit 1; fi ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
# status exits non-zero when nothing can be ready (its diagnostics say why); the queue it
# prints is this case's subject either way
look() { prs refresh >/dev/null || { echo "    refresh failed"; tail -5 "$STATE/log" | cut -c1-160; exit 1; }; q="$(prs status --format json 2>/dev/null)" || :; [ -n "$q" ] || { echo "    status printed no queue"; exit 1; }; }
# how many times the writers were asked for, so far
asked() { grep -c '^api graphql .*statusCheckRollup' "$STATE/log" || :; }
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }

# ---------------------------------------------------------------- 1. protection and rulesets
look
[ "$(field 2 .disposition)" = ready ] || { echo "    #2 (lint as the bound app's check run) is $(field 2 .disposition), not ready"; field 2 '{reasons, evidence}'; exit 1; }
[ "$(field 1 .disposition)" = waiting_for_checks ] \
  || { echo "    #1 (lint as a commit status from another writer) is $(field 1 .disposition), not waiting_for_checks"; field 1 '{reasons, evidence}'; exit 1; }
[ "$(field 1 .required_checks)" = missing ] || { echo "    #1's required checks are $(field 1 .required_checks), not missing"; exit 1; }
detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci: passed"*"lint (app 15368): missing"*) ;; *) echo "    #1's evidence does not name each context: $detail"; exit 1 ;; esac
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 2 ] || { echo "    the next merge is not #2"; exit 1; }
detail="$(field 2 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci: passed"*"lint (app 15368): passed"*) ;; *) echo "    #2's evidence does not say the bound app's lint passed: $detail"; exit 1 ;; esac
# the writer was read: one call, argument for argument (the stub answers no other), and the
# observation the verdict was taken from records the app on #2's lint
[ "$(asked)" = 1 ] || { echo "    the writers were asked for $(asked) times in one refresh of two pull requests, not once"; exit 1; }
grep -q '^api graphql -f query=query(.*statusCheckRollup.* -f owner=o -f name=r -F n=50$' "$STATE/log" \
  || { echo "    the writers were not read as the adapter reads them:"; grep '^api graphql' "$STATE/log" | cut -c1-160; exit 1; }
OBS="$W/.ai/local/state/integration/observation.json"
[ -f "$OBS" ] || { echo "    the refresh recorded no observation at $OBS"; exit 1; }
wrote() { jq -c --argjson n "$1" '[.pull_requests[] | select(.number == $n) | .checks[] | select(.name == "lint") | .app_id]' "$OBS"; }
[ "$(wrote 2)" = '[15368]' ] || { echo "    the observation records #2's lint as written by $(wrote 2), not [15368]"; exit 1; }
[ "$(wrote 1)" = '[null]' ] || { echo "    the observation records #1's lint (a commit status) as written by $(wrote 1), not [null]"; exit 1; }

# ---------------------------------------------------------------- 1b. another app's run
writers "$ci_written,${lint_run%\}},\"checkSuite\":{\"app\":{\"databaseId\":99}}}"
look
[ "$(wrote 2)" = '[99]' ] || { echo "    the observation records #2's lint as written by $(wrote 2), not [99]"; exit 1; }
[ "$(field 2 .disposition)" = waiting_for_checks ] \
  || { echo "    #2 (lint as app 99's check run) is $(field 2 .disposition), not waiting_for_checks"; field 2 '{reasons, evidence}'; exit 1; }
[ "$(field 2 .required_checks)" = missing ] || { echo "    #2's required checks are $(field 2 .required_checks) with lint from app 99, not missing"; exit 1; }
detail="$(field 2 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci: passed"*"lint (app 15368): missing"*) ;; *) echo "    #2's evidence does not say the bound app's lint is missing: $detail"; exit 1 ;; esac
[ "$(printf '%s' "$q" | jq -r .next_merge)" = null ] || { echo "    something is next though no pull request has the bound app's lint"; exit 1; }
[ "$(asked)" = 2 ] || { echo "    the writers were asked for $(asked) times over two refreshes, not twice"; exit 1; }

# ---------------------------------------------------------------- 2. nothing required (D5)
rm -f "$STATE/protection.json"; echo '[]' > "$STATE/rules.json"
look
for n in 1 2; do
  [ "$(field "$n" .disposition)" = unknown ] || { echo "    #$n is $(field "$n" .disposition) on a base that requires nothing, not unknown"; exit 1; }
  [ "$(field "$n" '.reasons[0]')" = no_required_checks ] || { echo "    #$n's reason is $(field "$n" '.reasons[0]'), not no_required_checks"; exit 1; }
  [ "$(field "$n" '.evidence[] | select(.kind == "required_checks") | .status')" = none_required ] \
    || { echo "    #$n's evidence does not say none_required"; exit 1; }
done
[ "$(printf '%s' "$q" | jq -r .next_merge)" = null ] || { echo "    something is next on a base that requires nothing"; exit 1; }
printf '%s' "$q" | jq -r '.diagnostics[]' | grep -q "requires no check" || { echo "    the diagnostics do not say the base requires no check"; exit 1; }

# ---------------------------------------------------------------- 3. an unread ruleset
echo '{"required_status_checks":{"contexts":["ci"]}}' > "$STATE/protection.json"; echo FAIL > "$STATE/rules.json"
look
for n in 1 2; do
  [ "$(field "$n" .disposition)" = unknown ] || { echo "    #$n is $(field "$n" .disposition) with the rulesets unread, not unknown"; exit 1; }
  # the rulesets carry the review policy too, so both are unread; the review is asked first
  case "$(field "$n" '.reasons[0]')" in review_policy_unread|required_checks_unread) ;;
    *) echo "    #$n's reason is $(field "$n" '.reasons[0]'), not an unread requirement"; exit 1 ;; esac
  [ "$(field "$n" '.required_checks')" = unknown ] || { echo "    #$n's required checks are $(field "$n" .required_checks), not unknown"; exit 1; }
done

# ---------------------------------------------------------------- 4. nothing more was asked
[ "$(asked)" = 2 ] || { echo "    the writers were asked for $(asked) times, not twice: a base that binds nothing asks nothing"; exit 1; }
if grep -q UNEXPECTED "$STATE/log"; then
  echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | cut -c1-160 | head -4; exit 1
fi
echo "    protection and rulesets together, a bound check read with its writer; nothing required or unread is never ready"
