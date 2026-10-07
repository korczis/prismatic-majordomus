# majordomus-covers: none
# The declarations are read page by page until every listed pull request was seen, and a pull
# request whose cross-references do not fit one page is read again alone, cursor by cursor,
# until they are whole (ADR 0101 §8, decision D4 of 2026-10-07; rule
# project.integration-follows-the-current-master). Through the real command line against a
# scripted forge that answers only the pages this case serves, as case 927 does:
#
#   1. the forge serves the open pull requests in two pages of 50: fifty nodes (#52 down to #4,
#      opened after the list was taken, then #2) and #1 behind `-f after=cursor-one`. Exactly
#      two page reads are logged, the second carrying the first page's cursor; the second page
#      says more follow and nothing more is asked
#   2. #1's cross-references say more follow, so #1 is read alone: a first page (an issue's
#      mention, and a cursor) and, behind `-f after=ref-one`, the page that holds #3, MERGED by
#      an OWNER, saying "Supersedes #1". Exactly two reads of #1, none of any other number
#   3. #1's cross-references are recorded whole and #1 is superseded by #3: the declaration
#      was on the last page of the last read. #2, whose cross-references fitted, is ready
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-933.git"; W="$T/../work-933"; STATE="$T/../forge-933"; BIN="$T/../bin-933"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
# the person's identity, in the clone: a machine with none configured (a CI runner) must not
# decide the outcome
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
branch() {   # <number>: a branch from master as it is now, pushed as its pull request's head
  gitq checkout -qb "feature/$1" master; echo "$1" > "$1.txt"; gitq add "$1.txt"; gitq commit -qm "change $1"
  gitq push -q origin "HEAD:refs/heads/feature/$1" "HEAD:refs/pull/$1/head"; gitq checkout -q master
}
land() {   # <number>: merged into master with a merge commit, as the forge merges
  gitq merge -q --no-ff "feature/$1" -m "Merge pull request #$1"; gitq push -q origin HEAD:master
}
H() { git --no-pager rev-parse "feature/$1"; }
M() { git --no-pager rev-parse master; }
init() { "$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }; }

# ---------------------------------------------------------------- the fixtures' wire shapes
# Every JSON key this case hands the adapter is written in the helpers below and nowhere else.
ci='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:05:00Z"}'
pr() {   # <number> <body> [login] [rollup]: one entry of `gh pr list --state open`
  printf '{"number":%s,"title":"change %s","author":{"login":"%s"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"%s","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "${3:-someone}" "$1" "$(H "$1")" "$1" "$1" "$2" "${4:-$ci}"
}
# a pull request that mentions another, as a cross-reference names it (the `source` of
# forge.rs DECLARATIONS_QUERY). <fork> is the pull request's own isCrossRepository: its head
# lives in a fork
src() {   # <number> <state> <association> <fork: true|false> <merge commit, or ''> <body> [login]
  local mc=null
  if [ -n "$5" ]; then mc="{\"oid\":\"$5\"}"; fi
  printf '{"__typename":"PullRequest","number":%s,"state":"%s","body":"%s","headRefOid":"%s","mergeCommit":%s,"isCrossRepository":%s,"authorAssociation":"%s","author":{"login":"%s"},"baseRefName":"master","changedFiles":%s}' \
    "$1" "$2" "$6" "$(H "$1")" "$mc" "$4" "$3" "${7:-someone}" "${FILES:-1}"
}
# one cross-reference. <elsewhere> is the event's isCrossRepository: the mention was made in
# another repository, where `#N` is not this repository's #N
mention() {   # <elsewhere: true|false> <source>
  printf '{"isCrossRepository":%s,"source":%s}' "$1" "$2"
}
an_issue='{"isCrossRepository":false,"source":{"__typename":"Issue"}}'
# one open pull request as the declarations read answers it: its author's association ('-'
# leaves the key out, as a forge that does not say), whether its head lives in a fork (FORK,
# false unless set; '-' leaves the key out) and one page of its cross-references
node() {   # <number> <association | -> <more follow: true|false> <cursor as JSON> <mentions, comma-joined>
  local assoc='' here="\"isCrossRepository\":${FORK:-false},"
  if [ "$2" != - ]; then assoc="\"authorAssociation\":\"$2\","; fi
  if [ "${FORK:-}" = - ]; then here=''; fi
  printf '{"number":%s,%s'"$here"'"timelineItems":{"pageInfo":{"hasNextPage":%s,"endCursor":%s},"nodes":[%s]}}' \
    "$1" "$assoc" "$3" "$4" "$5"
}
page() {   # <more follow: true|false> <cursor as JSON> <nodes, comma-joined>: one DECLARATIONS_QUERY answer
  printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":%s,"endCursor":%s},"nodes":[%s]}}}}\n' "$1" "$2" "$3"
}
of() {   # <node>: one DECLARATIONS_OF_QUERY answer
  printf '{"data":{"repository":{"pullRequest":%s}}}\n' "$1"
}
view() {   # <number> <state> <merge commit, or ''> <base>: `gh pr view --json` RESOLVED_FIELDS
  local mc=null
  if [ -n "$3" ]; then mc="{\"oid\":\"$3\"}"; fi
  printf '{"number":%s,"state":"%s","headRefOid":"%s","body":"","mergeCommit":%s,"baseRefName":"%s","author":{"login":"someone"},"isCrossRepository":%s,"changedFiles":%s}\n' \
    "$1" "$2" "$(H "$1")" "$mc" "$4" "${FORK:-false}" "${FILES:-1}"
}
# the two queries, as the adapter sends them (forge.rs DECLARATIONS_QUERY and
# DECLARATIONS_OF_QUERY): one line each, and the forge below answers no other
# shellcheck disable=SC2016  # GraphQL variables, not the shell's
printf '%s' 'query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor}nodes{number authorAssociation isCrossRepository timelineItems(first:100,itemTypes:[CROSS_REFERENCED_EVENT]){pageInfo{hasNextPage endCursor}nodes{...on CrossReferencedEvent{isCrossRepository source{__typename ...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository authorAssociation author{login} baseRefName changedFiles}}}}}}}}}' > "$STATE/declarations.query"
# shellcheck disable=SC2016
printf '%s' 'query($owner:String!,$name:String!,$number:Int!,$after:String){repository(owner:$owner,name:$name){pullRequest(number:$number){number authorAssociation isCrossRepository timelineItems(first:100,after:$after,itemTypes:[CROSS_REFERENCED_EVENT]){pageInfo{hasNextPage endCursor}nodes{...on CrossReferencedEvent{isCrossRepository source{__typename ...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository authorAssociation author{login} baseRefName changedFiles}}}}}}}}' > "$STATE/declarations-of.query"

# ---------------------------------------------------------------- the forge
# The base requires `ci` and binds it to no app, so no writers read is asked (case 927): every
# `gh api graphql` here is a declarations read, and it is answered only in the adapter's exact
# form: the query, the owner and the name as raw strings, one typed value, and nothing after it
# or one cursor this forge gave.
#   -F n=50                      declarations.json, or declarations-after-<cursor>.json
#   -F number=<n>                declarations-<n>.json, or declarations-<n>-after-<cursor>.json,
#                                or declarations-<n>-every.json for any cursor without a file
#   declarations-refused         every declarations read is a 403
#   declarations-502             the next declarations read is a 502, once
#   pr view <n> --json <RESOLVED_FIELDS>   view-<n>.json
#   merges-allowed               `gh pr merge` merges as the forge does; absent, it is UNEXPECTED
# A list of the closed pull requests, a page or a view nobody put there, another query, a typed
# owner: all of it is UNEXPECTED, and the command that asked fails.
{
  printf '#!/bin/sh\n'
  printf "STATE='%s'\nORIGIN='%s'\n" "$STATE" "$ORIGIN"
  cat <<'EOF'
echo "$*" >> "$STATE/log"
unexpected() { echo "UNEXPECTED: $1" >> "$STATE/log"; echo "gh: this forge does not answer: $1" >&2; exit 1; }
case " $* " in *" --admin "*) unexpected "a call with --admin" ;; esac
case "$1 $2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state closed "*) unexpected "a list of the closed pull requests: declarations are read from cross-references" ;;
      *" --state open "*) cat "$STATE/open.json" ;;
      # the cleanup's read of the branches merged pull requests left behind: none here
      *" --state merged "*) echo '[]' ;;
      *) unexpected "this pull request list" ;;
    esac ;;
  "api graphql")
    [ "$#" -ge 10 ] && [ "$3 $5 $6 $7 $8 $9" = "-f -f owner=o -f name=r -F" ] \
      || unexpected "a declarations read in another form"
    after=''
    if [ "$#" = 12 ] && [ "${11}" = -f ]; then
      case "${12}" in
        after=*[!A-Za-z0-9-]*|after=) unexpected "a cursor this forge never gave" ;;
        after=*) after="${12#after=}" ;;
        *) unexpected "a twelfth argument that is no cursor" ;;
      esac
    elif [ "$#" != 10 ]; then unexpected "a declarations read with $# arguments"; fi
    case "${10}" in
      n=50)
        [ "$4" = "query=$(cat "$STATE/declarations.query")" ] || unexpected "another query than the declarations page"
        answer="$STATE/declarations${after:+-after-$after}.json" ;;
      number=*[!0-9]*|number=) unexpected "a declarations read typed ${10}" ;;
      number=*)
        n="${10#number=}"
        [ "$4" = "query=$(cat "$STATE/declarations-of.query")" ] || unexpected "another query than the declarations of one pull request"
        answer="$STATE/declarations-$n${after:+-after-$after}.json"
        if [ -n "$after" ] && [ ! -f "$answer" ]; then answer="$STATE/declarations-$n-every.json"; fi ;;
      *) unexpected "a declarations read typed ${10}" ;;
    esac
    if [ -f "$STATE/declarations-refused" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    if [ -f "$STATE/declarations-502" ]; then
      rm -f "$STATE/declarations-502"; echo 'gh: HTTP 502: Bad Gateway (https://api.github.com/graphql)' >&2; exit 1
    fi
    [ -f "$answer" ] || unexpected "a page nobody should ask for (${answer##*/})"
    cat "$answer" ;;
  "pr view")
    case " $* " in
      # the executor's read just before a closure: the state and the head, as --jq renders them
      *" --json state,headRefOid "*)
        jq -r --argjson n "$3" '.[] | select(.number == $n) | "OPEN " + .headRefOid' "$STATE/open.json" ;;
      # the refresh's read of a successor or dependency an open body names that is not open
      *" --json number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles "*)
        [ -f "$STATE/view-$3.json" ] || unexpected "a view of #$3, which this case did not put there"
        cat "$STATE/view-$3.json" ;;
      # the executor's read after a merge it asked for
      *" --json state "*)
        [ -f "$STATE/merges-allowed" ] || unexpected "a view of #$3's state, where nothing merges"
        if [ -f "$STATE/merged-$3" ]; then echo MERGED; else echo OPEN; fi ;;
      *) unexpected "a view of #$3 asking other fields" ;;
    esac ;;
  "pr merge")
    [ -f "$STATE/merges-allowed" ] || unexpected "a merge of #$3"
    n="$3"; want=''; prev=''
    for a in "$@"; do [ "$prev" = --match-head-commit ] && want="$a"; prev="$a"; done
    have="$(git -C "$ORIGIN" rev-parse "refs/pull/$n/head")"
    [ -z "$want" ] || [ "$want" = "$have" ] || { echo "head moved" >&2; exit 1; }
    t="$(mktemp -d)"
    git clone -q "$ORIGIN" "$t/c" 2>/dev/null && cd "$t/c" &&
      git fetch -q origin "refs/pull/$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-$n" &&
      jq -c --argjson n "$n" '[.[] | select(.number != $n)]' "$STATE/open.json" > "$STATE/open.next" &&
      mv "$STATE/open.next" "$STATE/open.json" ;;
  "pr close") echo "CLOSE $3" >> "$STATE/log" ;;
  *) unexpected "$1 $2" ;;
esac
EOF
} > "$BIN/gh"
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
OBS="$W/.ai/local/state/integration/observation.json"
said() { tail -8 "$STATE/log" | cut -c1-160; }
# status exits non-zero when it has diagnostics; the queue it prints is this case's subject
look() {
  expect_exit 0 prs refresh || { echo "    the refresh failed; the forge was last asked:"; said; exit 1; }
  q="$(prs status --format json 2>/dev/null)" || :
  [ -n "$q" ] || { echo "    status printed no queue"; exit 1; }
}
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
count() { local n; n="$(grep -c -- "$1" "$STATE/log")" || :; printf '%s' "${n:-0}"; }
is() {   # <number> <disposition> <when>
  [ "$(field "$1" .disposition)" = "$2" ] \
    || { echo "    $3: #$1 is $(field "$1" .disposition), not $2"; field "$1" '{reasons, evidence}'; exit 1; }
}
# the details of #<number>'s supersession evidence with this status word, one per line
evidence() { field "$1" ".evidence[] | select(.kind == \"supersession\" and .status == \"$2\") | .detail"; }
says() {   # <text> <fragment> <what>: the text holds the fragment
  case "$1" in *"$2"*) ;; *) echo "    $3 does not say '$2': $1"; exit 1 ;; esac
}
# no reason of #<number> comes from a declaration: it is neither held nor superseded by one
undeclared() {   # <number> <when>
  local r; r="$(field "$1" '.reasons | join(" ")')"
  case "$r" in
    *successor_*|*superseded_by*|*declarations_unread*) echo "    $2: #$1 carries a declaration's reason: $r"; exit 1 ;;
  esac
  [ "$(field "$1" .superseded_by)" = null ] || { echo "    $2: #$1 names a successor: $(field "$1" .superseded_by)"; exit 1; }
}
# `prs cleanup --apply` closes nothing. It exits 0, or 10 when it leaves something to a person
closes_nothing() {   # <when>
  local rc=0
  prs cleanup --apply >"$STATE/apply.out" 2>&1 || rc=$?
  case "$rc" in 0|10) ;; *) echo "    $1: cleanup --apply exited $rc:"; tail -5 "$STATE/apply.out"; said; exit 1 ;; esac
  if grep -q '^CLOSE' "$STATE/log"; then echo "    $1: cleanup --apply closed:"; grep '^CLOSE' "$STATE/log"; exit 1; fi
}
no_surprise() {
  if grep -q UNEXPECTED "$STATE/log"; then
    echo "    the forge was asked something this harness does not answer:"
    grep -B1 UNEXPECTED "$STATE/log" | cut -c1-200 | head -4; exit 1
  fi
}

branch 3; land 3; MC3="$(M)"
branch 1; branch 2
init
# newest first, as the forge lists them
printf '[%s,%s]\n' "$(pr 2 '')" "$(pr 1 '')" > "$STATE/open.json"
# page one: fifty pull requests, newest first. #52..#4 were opened after the list was taken
# (the list does not name them, and a node it does not name is nobody's); #2 is the last
page_one=""; n=52
while [ "$n" -ge 4 ]; do page_one="$page_one$(node "$n" OWNER false null ''),"; n=$((n - 1)); done
page true '"cursor-one"' "$page_one$(node 2 OWNER false null '')" > "$STATE/declarations.json"
# page two: #1, the oldest, its cross-references not whole. It says more pull requests follow;
# every listed number is seen by then, and a third page is not there to be served
page true '"cursor-two"' "$(node 1 OWNER true '"ref-zero"' "$an_issue")" > "$STATE/declarations-after-cursor-one.json"
# #1 alone: two pages, the declaration on the second
of "$(node 1 OWNER true '"ref-one"' "$an_issue")" > "$STATE/declarations-1.json"
of "$(node 1 OWNER false null "$(mention false "$(src 3 MERGED OWNER false "$MC3" 'Supersedes #1')")")" \
  > "$STATE/declarations-1-after-ref-one.json"
[ "$(jq '.data.repository.pullRequests.nodes | length' "$STATE/declarations.json")" = 50 ] \
  || { echo "    the fixture's first page does not hold 50 pull requests"; exit 1; }

look
# ---------------------------------------------------------------- 1. two pages of pull requests
grep -- ' -F n=50' "$STATE/log" > "$STATE/pages" || :
[ "$(grep -c . "$STATE/pages")" = 2 ] || { echo "    the declarations page was not asked for exactly twice:"; cut -c1-40 "$STATE/pages"; said; exit 1; }
sed -n 1p "$STATE/pages" | grep -q ' -f owner=o -f name=r -F n=50$' \
  || { echo "    the first page was not asked for with raw owner and name and no cursor"; said; exit 1; }
sed -n 2p "$STATE/pages" | grep -q ' -f owner=o -f name=r -F n=50 -f after=cursor-one$' \
  || { echo "    the second page was not asked for after the first page's cursor"; said; exit 1; }
# ---------------------------------------------------------------- 2. #1 alone, cursor by cursor
grep -- ' -F number=' "$STATE/log" > "$STATE/alone" || :
[ "$(grep -c . "$STATE/alone")" = 2 ] || { echo "    not exactly two reads of one pull request alone:"; sed 's/.*}}}}}}}}//' "$STATE/alone"; exit 1; }
sed -n 1p "$STATE/alone" | grep -q ' -f owner=o -f name=r -F number=1$' \
  || { echo "    #1 was not first read alone from the start"; sed 's/.*}}}}}}}}//' "$STATE/alone"; exit 1; }
sed -n 2p "$STATE/alone" | grep -q ' -f owner=o -f name=r -F number=1 -f after=ref-one$' \
  || { echo "    #1's second page was not asked for after its first page's cursor"; sed 's/.*}}}}}}}}//' "$STATE/alone"; exit 1; }
grep -q -e '-F owner=' -e '-F name=' -e '-F after=' "$STATE/log" \
  && { echo "    a declarations read passed a string as a typed field (-F), which gh would convert"; exit 1; }
# ---------------------------------------------------------------- 3. whole, and decided
read_of() { jq -r --argjson n "$1" '.pull_requests[] | select(.number == $n) | .cross_references' "$OBS"; }
[ "$(read_of 1)" = whole ] || { echo "    the observation records #1's cross-references as $(read_of 1), not whole"; exit 1; }
[ "$(read_of 2)" = whole ] || { echo "    the observation records #2's cross-references as $(read_of 2), not whole"; exit 1; }
is 1 superseded "its declaration on the last page of its cross-references"
[ "$(field 1 .superseded_by)" = 3 ] || { echo "    #1 is not superseded by #3: $(field 1 .superseded_by)"; exit 1; }
is 2 ready "its cross-references fitting the page"
no_surprise
echo "    every page of pull requests and every page of one's cross-references is read, once each"
