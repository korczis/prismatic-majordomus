# majordomus-covers: none
# A successor that was squashed onto master landed: git finds its merge commit there, though
# its head is nowhere in master's history (ADR 0101 §3, decision D3 of 2026-10-07; rule
# project.integration-follows-the-current-master). Through the real command line against a
# scripted forge:
#
#   1. #1, by an OWNER, says "Superseded by #2". #2 is two commits, squashed onto master as one
#      commit S. The forge's view of #2 is MERGED with mergeCommit S, and it was asked for with
#      the fields that carry it. #1 is superseded by #2, and its evidence says master contains
#      #2's merge commit
#   2. the control: the same view without a merge commit. #2's head is still not on master, so
#      #1 is possibly_redundant (successor_not_landed:#2): it was the merge commit, read from
#      the forge and found by git, that landed #2, never the forge's word MERGED
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-932.git"; W="$T/../work-932"; STATE="$T/../forge-932"; BIN="$T/../bin-932"
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

# #2: two commits, so that the squash equals neither of them as a patch
gitq checkout -qb feature/2 master
echo two > 2.txt; gitq add 2.txt; gitq commit -qm "change 2, first half"
echo more > 2b.txt; gitq add 2b.txt; gitq commit -qm "change 2, second half"
gitq push -q origin HEAD:refs/heads/feature/2 HEAD:refs/pull/2/head; gitq checkout -q master
gitq merge -q --squash feature/2 >/dev/null 2>&1; gitq commit -qm "change 2 (#2)"; gitq push -q origin HEAD:master
S="$(M)"
branch 1
init
if git --no-pager merge-base --is-ancestor "$(H 2)" master; then echo "    the fixture's squash left #2's head on master"; exit 1; fi
[ "$S" != "$(H 2)" ] || { echo "    the fixture's squash commit is #2's head"; exit 1; }
printf '[%s]\n' "$(pr 1 'Superseded by #2')" > "$STATE/open.json"
page false null "$(node 1 OWNER false null '')" > "$STATE/declarations.json"

# ---------------------------------------------------------------- 1. landed by its merge commit
view 2 MERGED "$S" master > "$STATE/view-2.json"
look
grep -q '^pr view 2 --json number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles' "$STATE/log" \
  || { echo "    #2 was not viewed with its merge commit asked for:"; grep '^pr view' "$STATE/log"; exit 1; }
is 1 superseded "its successor squashed onto master"
[ "$(field 1 .superseded_by)" = 2 ] || { echo "    #1 is not superseded by #2: $(field 1 .superseded_by)"; exit 1; }
[ "$(field 1 '.reasons[0]')" = 'superseded_by:#2' ] || { echo "    #1's first reason is $(field 1 '.reasons[0]')"; exit 1; }
says "$(evidence 1 landed)" "master contains its merge commit" "#1's landed evidence"
out="$(prs cleanup --format json)" || { echo "    cleanup failed: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -c '[.[] | [.pr, .disposition, .action]]')" = '[[1,"superseded","would_close"]]' ] \
  || { echo "    the dry cleanup does not list #1 to close: $out"; exit 1; }

# ---------------------------------------------------------------- 2. the control: no merge commit
view 2 MERGED '' master > "$STATE/view-2.json"
look
is 1 possibly_redundant "its successor MERGED by the forge's word alone"
[ "$(field 1 '.reasons[0]')" = 'successor_not_landed:#2' ] || { echo "    #1's first reason is $(field 1 '.reasons[0]'), not successor_not_landed:#2"; exit 1; }
[ "$(field 1 .superseded_by)" = null ] || { echo "    #1 names a successor git did not find in master"; exit 1; }
closes_nothing "its successor MERGED by the forge's word alone"
no_surprise
echo "    a squashed successor landed by its merge commit; the forge's word MERGED lands nothing"
