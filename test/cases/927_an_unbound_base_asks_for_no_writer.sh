# majordomus-covers: none
# Who wrote a check is read only when the base binds a context to an app, and then it is read
# page by page until every listed pull request was seen (ADR 0101; rule
# project.integration-follows-the-current-master). Through the real command line against a
# scripted forge, as case 857 does. The forge here answers `gh api graphql` only when it is
# called exactly as the adapter calls it, and only for the pages this case serves:
#
#   1. a base that binds nothing asks for no writer. Three shapes of it: contexts only, a
#      `checks` entry with no `app_id`, and `app_id` -1 (the forge's "any source"). Each time
#      the refresh succeeds, the log holds no `api graphql`, and a passing check run (#1) and
#      a passing commit status (#2) named `ci` are both ready: what the unbound stubs of the
#      other cases rely on
#   2. the base binds `ci` to app 15368 and the forge serves the writers in two pages of 50:
#      fifty nodes (#51 down to #2, newest first, as the query orders them) and then #1 behind
#      `-f after=<cursor>`. The log holds exactly two `api graphql` calls, each with owner and
#      name as raw strings (`-f`) and only the page size typed (`-F n=50`); the second carries
#      the first page's cursor. #1, on the last page, is attributed: ready, its evidence
#      naming the app. The second page says more follow, and nothing more is asked: paging
#      stops once every listed pull request was seen, never at a count
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-927.git"; W="$T/../work-927"; STATE="$T/../forge-927"; BIN="$T/../bin-927"
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

# what `gh pr list` says of the two: `ci` passed on both, as a check run on #1 and as a commit
# status on #2. Neither entry says who wrote it
ci_run='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","startedAt":"2026-09-01T00:01:00Z","completedAt":"2026-09-01T00:05:00Z"}'
ci_status='{"__typename":"StatusContext","context":"ci","state":"SUCCESS","startedAt":"2026-09-01T00:05:00Z"}'
pr() {   # <number> <head> <rollup>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$2" "$1" "$1" "$3"
}
# newest first, as the forge lists them
printf '[%s,%s]\n' "$(pr 2 "$H2" "$ci_status")" "$(pr 1 "$H1" "$ci_run")" > "$STATE/prs.json"
echo '[]' > "$STATE/rules.json"

# The forge. A writers read is answered only in the adapter's exact form: the page query, the
# owner and the name as raw strings, the page size typed, and nothing after it (the first
# page, page-first.json) or the cursor of the first page (page-after-<cursor>.json). A page
# this case did not put there, another query, another order of arguments, a typed owner: all
# of it is UNEXPECTED, and the refresh fails.
{
  printf '#!/bin/sh\n'
  printf "STATE='%s'\nORIGIN='%s'\n" "$STATE" "$ORIGIN"
  cat <<'EOF'
echo "$*" >> "$STATE/log"
unexpected() { echo "UNEXPECTED" >> "$STATE/log"; echo "gh: this forge does not answer: $1" >&2; exit 1; }
case "$1 $2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") cat "$STATE/protection.json" ;;
  "api repos/o/r/rules/branches/master") cat "$STATE/rules.json" ;;
  "pr list")
    case " $* " in
      *" --state closed "*) echo '[]' ;;
      *" --state open "*) cat "$STATE/prs.json" ;;
      *) unexpected "this pull request list" ;;
    esac ;;
  "api graphql")
    [ "$3" = -f ] && [ "$5" = -f ] && [ "$6" = owner=o ] && [ "$7" = -f ] && [ "$8" = name=r ] \
      && [ "$9" = -F ] && [ "${10}" = n=50 ] || unexpected "a writers read in another form"
    case "$4" in
      'query=query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor}nodes{number headRefOid statusCheckRollup{contexts(first:100){pageInfo{hasNextPage endCursor}nodes{__typename ...on CheckRun{name status conclusion startedAt completedAt checkSuite{app{databaseId}}}...on StatusContext{context state startedAt:createdAt}}}}}}}}') ;;
      *) unexpected "another query than the writers page" ;;
    esac
    if [ "$#" = 10 ]; then page="$STATE/page-first.json"
    elif [ "$#" = 12 ] && [ "${11}" = -f ]; then
      case "${12}" in
        after=*[!A-Za-z0-9-]*|after=) unexpected "a cursor this forge never gave" ;;
        after=*) page="$STATE/page-after-${12#after=}.json" ;;
        *) unexpected "a twelfth argument that is no cursor" ;;
      esac
    else unexpected "a writers read with $# arguments"; fi
    [ -f "$page" ] || unexpected "a page nobody should ask for (${page##*/})"
    cat "$page" ;;
  *) unexpected "$1 $2" ;;
esac
EOF
} > "$BIN/gh"
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
said() { tail -8 "$STATE/log" | cut -c1-160; }
# status exits non-zero when it has diagnostics; the queue it prints is this case's subject
look() {
  expect_exit 0 prs refresh || { echo "    the refresh failed; the forge was last asked:"; said; exit 1; }
  q="$(prs status --format json 2>/dev/null)" || :
  [ -n "$q" ] || { echo "    status printed no queue"; exit 1; }
}
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
asked() { local n; n="$(grep -c '^api graphql ' "$STATE/log")" || :; printf '%s' "${n:-0}"; }
ready() {   # <number> <what it is>
  [ "$(field "$1" .disposition)" = ready ] \
    || { echo "    #$1 ($2) is $(field "$1" .disposition), not ready"; field "$1" '{reasons, evidence}'; exit 1; }
  [ "$(field "$1" .required_checks)" = passed ] \
    || { echo "    #$1's required checks are $(field "$1" .required_checks), not passed"; exit 1; }
}

# ---------------------------------------------------------------- 1. nothing bound, nothing asked
shape=0
for protection in \
  '{"required_status_checks":{"contexts":["ci"]}}' \
  '{"required_status_checks":{"contexts":["ci"],"checks":[{"context":"ci"}]}}' \
  '{"required_status_checks":{"contexts":["ci"],"checks":[{"context":"ci","app_id":-1}]}}'
do
  shape=$((shape + 1))
  printf '%s\n' "$protection" > "$STATE/protection.json"
  look
  [ "$(asked)" = 0 ] \
    || { echo "    unbound shape $shape ($protection): the writers were asked for $(asked) time(s)"; exit 1; }
  ready 1 "a passing check run named ci, unbound shape $shape"
  ready 2 "a passing commit status named ci, unbound shape $shape"
  detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
  case "$detail" in
    *"(app "*) echo "    unbound shape $shape names an app in #1's evidence: $detail"; exit 1 ;;
    *"ci: passed"*) ;;
    *) echo "    #1's evidence does not say ci passed: $detail"; exit 1 ;;
  esac
done
[ "$shape" = 3 ] || { echo "    $shape unbound shapes were tried, not 3"; exit 1; }
[ "$(printf '%s' "$q" | jq -r .next_merge)" != null ] || { echo "    nothing is next on an unbound base whose check passed"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    an unbound base asked the forge something more:"; grep -B1 UNEXPECTED "$STATE/log" | cut -c1-160 | head -4; exit 1; }

# ---------------------------------------------------------------- 2. bound: two pages, the last listed one attributed
echo '{"required_status_checks":{"contexts":["ci"],"checks":[{"context":"ci","app_id":15368}]}}' > "$STATE/protection.json"
by_actions='"checkSuite":{"app":{"databaseId":15368}}}'
ci_written="${ci_run%\}},$by_actions"
written() {   # <number> <head> <contexts>
  printf '{"number":%s,"headRefOid":"%s","statusCheckRollup":{"contexts":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}' \
    "$1" "$2" "$3"
}
# page one: fifty pull requests, newest first. #51..#3 were opened after the list was taken
# (the list does not name them, and a node it does not name is nobody's); #2 is the last,
# with the commit status it has
page_one=""; n=51
while [ "$n" -ge 3 ]; do
  page_one="$page_one$(written "$n" "$(printf '%040d' "$n")" "$ci_written"),"
  n=$((n - 1))
done
page_one="$page_one$(written 2 "$H2" "$ci_status")"
printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":true,"endCursor":"cursor-one"},"nodes":[%s]}}}}\n' \
  "$page_one" > "$STATE/page-first.json"
# page two: #1, the oldest. It says more follow; every listed number is seen by then, and a
# third page is not there to be served
printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":true,"endCursor":"cursor-two"},"nodes":[%s]}}}}\n' \
  "$(written 1 "$H1" "$ci_written")" > "$STATE/page-after-cursor-one.json"
[ "$(jq '.data.repository.pullRequests.nodes | length' "$STATE/page-first.json")" = 50 ] \
  || { echo "    the fixture's first page does not hold 50 pull requests"; exit 1; }
[ "$(jq '.data.repository.pullRequests.nodes | length' "$STATE/page-after-cursor-one.json")" = 1 ] \
  || { echo "    the fixture's second page does not hold 1 pull request"; exit 1; }

look
[ "$(asked)" = 2 ] || { echo "    the writers were asked for $(asked) time(s), not twice (one per page):"; grep '^api graphql ' "$STATE/log" | cut -c1-60; said; exit 1; }
grep '^api graphql ' "$STATE/log" > "$STATE/asked"
sed -n 1p "$STATE/asked" | grep -q ' -f owner=o -f name=r -F n=50$' \
  || { echo "    the first page was not asked for with raw owner and name and no cursor:"; sed -n 1p "$STATE/asked" | sed 's/.*}}}}}}}}//'; exit 1; }
sed -n 2p "$STATE/asked" | grep -q ' -f owner=o -f name=r -F n=50 -f after=cursor-one$' \
  || { echo "    the second page was not asked for after the first page's cursor:"; sed -n 2p "$STATE/asked" | sed 's/.*}}}}}}}}//'; exit 1; }
grep -q -e '-F owner=' -e '-F name=' -e '-F after=' "$STATE/asked" \
  && { echo "    a writers read passed a string as a typed field (-F), which gh would convert"; exit 1; }
grep -q 'number=' "$STATE/asked" && { echo "    a pull request read whole was asked for again by number"; exit 1; }

ready 1 "bound, attributed on the second page"
detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci (app 15368): passed"*) ;; *) echo "    #1's evidence does not name the app that wrote ci: $detail"; exit 1 ;; esac
# #2's commit status is no check run of app 15368, whoever reports it: the bound context has
# not reported there, and the page that says so was read
[ "$(field 2 .disposition)" != ready ] || { echo "    #2 is ready on a commit status, with ci bound to an app"; exit 1; }
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 1 ] || { echo "    the next merge is $(printf '%s' "$q" | jq -r .next_merge), not #1"; exit 1; }

grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | cut -c1-160 | head -4; exit 1; }
echo "    an unbound base asks for no writer; a bound one reads page by page until every listed pull request was seen"
