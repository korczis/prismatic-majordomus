# majordomus-covers: none
# A pull request whose cross-references could not be read whole is held: never merged, never
# closed, whatever the part that was read says (ADR 0101 §8, decision D4 of 2026-10-07; rule
# project.integration-follows-the-current-master). A truncated read must never release, and
# never close. Through the real command line against a scripted forge:
#
#   1. #1's cross-references say more follow on the page read, and on every page of the read
#      of #1 alone, each with a cursor: the adapter asks exactly fifty times (REFERENCE_PAGES)
#      and stops. The part it read already shows #3, MERGED by an OWNER, its head on master,
#      saying "Supersedes #1"
#   2. #1 is unknown, its first reason declarations_unread, superseded_by nothing, its evidence
#      references_truncated; the observation records `truncated`; status names #1 in a
#      diagnostic, and both say no refresh clears it: a person decides. #2, read whole, is
#      ready
#   3. a drain of two steps merges #2 and never #1; cleanup --apply closes nothing
#   4. the control: the forge serves a last, complete page, and #1 is superseded by #3. The
#      fixture's declaration counts as soon as the read is whole
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-938.git"; W="$T/../work-938"; STATE="$T/../forge-938"; BIN="$T/../bin-938"
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
git --no-pager merge-base --is-ancestor "$(H 3)" master || { echo "    the fixture's #3 is not on master"; exit 1; }
printf '[%s,%s]\n' "$(pr 2 '')" "$(pr 1 '')" > "$STATE/open.json"
declares="$(mention false "$(src 3 MERGED OWNER false "$MC3" 'Supersedes #1')")"
# the page read: #2 whole, #1 with more to follow
page false null "$(node 2 OWNER false null ''),$(node 1 OWNER true '"ref-zero"' "$declares")" > "$STATE/declarations.json"
# #1 alone: the first page, and the same answer behind every cursor it gives
of "$(node 1 OWNER true '"again"' "$declares")" > "$STATE/declarations-1.json"
cp "$STATE/declarations-1.json" "$STATE/declarations-1-every.json"

# ---------------------------------------------------------------- 1. fifty pages, and no more
look
grep -- ' -F number=' "$STATE/log" > "$STATE/alone" || :
[ "$(grep -c . "$STATE/alone")" = 50 ] \
  || { echo "    #1 was read alone $(grep -c . "$STATE/alone") time(s), not 50:"; sed 's/.*}}}}}}}}//' "$STATE/alone" | LC_ALL=C sort | uniq -c; exit 1; }
[ "$(grep -c -- ' -F number=1$' "$STATE/alone")" = 1 ] && [ "$(grep -c -- ' -F number=1 -f after=again$' "$STATE/alone")" = 49 ] \
  || { echo "    #1 was not read from the start once and then forty-nine times after the cursor given:"; sed 's/.*}}}}}}}}//' "$STATE/alone" | LC_ALL=C sort | uniq -c; exit 1; }
[ "$(count ' -F n=50')" = 1 ] || { echo "    the declarations page was asked for $(count ' -F n=50') time(s), not once"; exit 1; }

# ---------------------------------------------------------------- 2. held, and said so
is 1 unknown "its cross-references truncated"
[ "$(field 1 '.reasons[0]')" = declarations_unread ] || { echo "    #1's first reason is $(field 1 '.reasons[0]'), not declarations_unread"; exit 1; }
[ "$(field 1 .superseded_by)" = null ] || { echo "    #1 names a successor on a truncated read: $(field 1 .superseded_by)"; exit 1; }
[ -n "$(evidence 1 references_truncated)" ] || { echo "    #1's evidence does not say references_truncated:"; field 1 .evidence; exit 1; }
says "$(evidence 1 references_truncated)" "#1 is held" "#1's references_truncated evidence"
read_of() { jq -r --argjson n "$1" '.pull_requests[] | select(.number == $n) | .cross_references' "$OBS"; }
[ "$(read_of 1)" = truncated ] || { echo "    the observation records #1's cross-references as $(read_of 1), not truncated"; exit 1; }
[ "$(read_of 2)" = whole ] || { echo "    the observation records #2's cross-references as $(read_of 2), not whole"; exit 1; }
diag="$(printf '%s' "$q" | jq -r '.diagnostics[]' | grep 'were not all read')" \
  || { echo "    no diagnostic says the mentions were not all read:"; printf '%s' "$q" | jq -r '.diagnostics[]'; exit 1; }
says "$diag" "#1" "the diagnostic"
says "$diag" "truncated: #1" "the diagnostic"
says "$diag" "which no refresh clears: a person decides it" "the diagnostic"
says "$(field 1 .next_action)" "a person decides it" "#1's next action"
case "$(field 1 .next_action)" in *"majordomus prs refresh"*) echo "    #1's next action is a refresh, which cannot clear a truncated read: $(field 1 .next_action)"; exit 1 ;; esac
case "$diag" in *"#2"*) echo "    the diagnostic names #2, which was read whole: $diag"; exit 1 ;; esac
is 2 ready "its cross-references whole"
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 2 ] || { echo "    the next merge is $(printf '%s' "$q" | jq -r .next_merge), not #2"; exit 1; }

# ---------------------------------------------------------------- 3. never merged, never closed
out="$(prs cleanup --format json)" || { echo "    cleanup failed: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -c '[.[] | select(.pr == 1)]')" = '[]' ] || { echo "    cleanup has something for #1: $out"; exit 1; }
closes_nothing "its cross-references truncated"
touch "$STATE/merges-allowed"
drc=0; out="$(prs drain --max 2 2>&1)" || drc=$?
grep -q '^pr merge 2 ' "$STATE/log" || { echo "    the drain (exit $drc) did not merge #2: $out"; said; exit 1; }
grep -q '^pr merge 1 ' "$STATE/log" && { echo "    the drain asked to merge #1, whose cross-references were not read whole"; exit 1; }
git -C "$ORIGIN" merge-base --is-ancestor "$(H 2)" master || { echo "    #2 is not on origin's master after the drain (exit $drc): $out"; exit 1; }
if git -C "$ORIGIN" merge-base --is-ancestor "$(H 1)" master; then echo "    #1 is on origin's master"; exit 1; fi
closes_nothing "after the drain, its cross-references still truncated"
no_surprise

# ---------------------------------------------------------------- 4. the control: a whole read
page false null "$(node 1 OWNER false null "$declares")" > "$STATE/declarations.json"
look
[ "$(read_of 1)" = whole ] || { echo "    the control: the observation records #1's cross-references as $(read_of 1)"; exit 1; }
is 1 superseded "the control: its cross-references read whole"
[ "$(field 1 .superseded_by)" = 3 ] || { echo "    the control: #1 is not superseded by #3: $(field 1 .superseded_by)"; exit 1; }
grep -q '^CLOSE' "$STATE/log" && { echo "    something was closed"; exit 1; }
no_surprise
echo "    a truncated cross-reference read holds: fifty pages asked, nothing merged, nothing closed, until it is whole"
