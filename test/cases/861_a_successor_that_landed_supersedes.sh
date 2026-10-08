# majordomus-covers: none
# A pull request a successor replaced is `superseded` once that successor landed, and only
# then, and only when someone the repository lets declare one declared it; strong evidence
# from git is `redundant`; weak evidence is never closed (ADR 0101 §3 and §6, owner decision
# D2, and decisions D1 to D4 of 2026-10-07). Through the real command line against a scripted
# forge, as case 720 does. Every author here is an OWNER; cases 928 to 939 hold who may
# declare, and what a partial read does:
#
#   1. #2 said "Supersedes #1" and was merged with a merge commit, so it is no longer open: the
#      observation reads it among the pull requests that mention #1 (its cross-references,
#      never a search of the closed ones), and #1 — which now conflicts with what #2 brought —
#      is superseded, superseded_by 2
#   2. #3 says "Superseded by #4", and #4 was closed unmerged: the observation views #4, and
#      the hold is released: #3 is decided on its own, with not_landed evidence; #5 says
#      "Superseded by #6", still open: #5 waits, never merges
#   3. cleanup lists exactly #1 to close; with --apply only #1 is closed, its comment opens
#      with its successor and does not claim #1's own work is on master, and the trail records
#      closed_superseded
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-861.git"; W="$T/../work-861"; STATE="$T/../forge-861"; BIN="$T/../bin-861"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
branch() {   # <number> <file> <content>: a branch from master, pushed as its pull request's head
  gitq checkout -qb "feature/$1" master; echo "$3" > "$2"; gitq add "$2"; gitq commit -qm "change $1"
  gitq push -q origin "HEAD:refs/heads/feature/$1" "HEAD:refs/pull/$1/head"; gitq checkout -q master
}
branch 1 a.txt one; branch 2 a.txt two; branch 3 three.txt three; branch 4 four.txt four
branch 5 five.txt five; branch 6 six.txt six
H() { git --no-pager rev-parse "feature/$1"; }
# #2 lands with a merge commit, as the forge merges: #1 conflicts with master now
gitq merge -q --no-ff "feature/2" -m "Merge pull request #2"; gitq push -q origin HEAD:master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

ci='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:05:00Z"}'
pr() {   # <number> <body>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"%s","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$(H "$1")" "$1" "$1" "$2" "$ci"
}
# a successor an open body names that is not open, as `gh pr view --json` shows it (forge.rs
# RESOLVED_FIELDS)
view() {   # <number> <state>
  printf '{"number":%s,"state":"%s","headRefOid":"%s","body":"","mergeCommit":null,"baseRefName":"master","author":{"login":"someone"},"isCrossRepository":false,"changedFiles":1}\n' "$1" "$2" "$(H "$1")"
}
# a pull request that mentions another, as a cross-reference names it (the `source` of forge.rs
# DECLARATIONS_QUERY)
src() {   # <number> <state> <merge commit> <body>
  printf '{"__typename":"PullRequest","number":%s,"state":"%s","body":"%s","headRefOid":"%s","mergeCommit":{"oid":"%s"},"isCrossRepository":false,"authorAssociation":"OWNER","author":{"login":"someone"},"baseRefName":"master","changedFiles":1}' \
    "$1" "$2" "$4" "$(H "$1")" "$3"
}
# one open pull request as the declarations read answers it: its author an OWNER, its head in
# this repository, and the one page of its cross-references
node() {   # <number> <sources that mention it, comma-joined>
  printf '{"number":%s,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}' "$1" "$2"
}
printf '[%s,%s,%s,%s]\n' "$(pr 1 '')" "$(pr 3 'Superseded by #4')" "$(pr 5 'Superseded by #6')" "$(pr 6 '')" > "$STATE/open.json"
# #1's cross-references name #2, merged, by an OWNER, a branch of this repository
printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s,%s,%s,%s]}}}}\n' \
  "$(node 6 '')" "$(node 5 '')" "$(node 3 '')" \
  "$(node 1 "{\"isCrossRepository\":false,\"source\":$(src 2 MERGED "$(git --no-pager rev-parse master)" 'Supersedes #1')}")" > "$STATE/declarations.json"
view 4 CLOSED > "$STATE/view-4.json"

cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    # the closed pull requests are no source of declarations: nobody lists them
    case " \$* " in
      *" --state closed "*) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
      # the cleanup's read of the branches merged pull requests left behind: none here
      *" --state merged "*) echo '[]' ;;
      *) cat "$STATE/open.json" ;;
    esac ;;
  # the base binds no check to an app, so the one GraphQL read is the declarations: every open
  # pull request with its author's association and the pull requests that mention it
  "api graphql")
    case "\$4" in
      *timelineItems*) cat "$STATE/declarations.json" ;;
      *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
    esac ;;
  "pr view")
    case " \$* " in
      # the executor's read just before a closure: the state and the head, as --jq renders them
      *" --json state,headRefOid "*)
        jq -r --argjson n "\$3" '.[] | select(.number == \$n) | "OPEN " + .headRefOid' "$STATE/open.json" ;;
      *) if [ -f "$STATE/view-\$3.json" ]; then cat "$STATE/view-\$3.json"; else echo "UNEXPECTED" >> "$STATE/log"; exit 1; fi ;;
    esac ;;
  "pr close") echo "CLOSE \$3" >> "$STATE/log" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }

# ---------------------------------------------------------------- 1. a successor that landed
prs refresh >/dev/null || { echo "    refresh failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json 2>/dev/null)" || :
[ -n "$q" ] || { echo "    status printed no queue"; exit 1; }
[ "$(field 1 .disposition)" = superseded ] || { echo "    #1 is $(field 1 .disposition), not superseded"; field 1 '{reasons, evidence}'; exit 1; }
[ "$(field 1 .superseded_by)" = 2 ] || { echo "    #1 is not superseded by #2: $(field 1 .superseded_by)"; exit 1; }
[ "$(field 1 '.reasons[0]')" = 'superseded_by:#2' ] || { echo "    #1's first reason is $(field 1 '.reasons[0]')"; exit 1; }
[ "$(field 1 '.evidence[] | select(.kind == "supersession") | .status')" = landed ] \
  || { echo "    #1's supersession evidence does not say landed"; field 1 .evidence; exit 1; }
grep -q -- '^pr list --state closed' "$STATE/log" \
  && { echo "    the closed pull requests were searched for declarations:"; grep '^pr list' "$STATE/log"; exit 1; }
grep -q -- '^api graphql -f query=query(.*timelineItems.* -f owner=o -f name=r -F n=50$' "$STATE/log" \
  || { echo "    the declarations were not read from the cross-references:"; cut -c1-120 "$STATE/log"; exit 1; }
ex="$(prs explain 1)" || { echo "    explain 1 failed"; exit 1; }
case "$ex" in *"superseded:   by #2"*) ;; *) echo "    explain does not name #1's successor: $ex"; exit 1 ;; esac

# ---------------------------------------------------------------- 2. a successor closed unmerged, and one open
# #4 replaced nothing, so #3 is decided on its own relation to a master that moved under it
# when #2 landed: it needs a refresh, and no word a declaration gives
[ "$(field 3 .disposition)" = needs_refresh ] \
  || { echo "    #3 is $(field 3 .disposition), not needs_refresh, though its successor was closed unmerged"; field 3 '{reasons, evidence}'; exit 1; }
[ "$(field 3 '.reasons[0]')" = 'behind_master:2' ] || { echo "    #3's first reason is $(field 3 '.reasons[0]'), not behind_master:2"; exit 1; }
case "$(field 3 '.reasons | join(" ")')" in
  *successor_*|*superseded_by*) echo "    #3 still carries a reason from #4: $(field 3 '.reasons | join(" ")')"; exit 1 ;;
esac
[ "$(field 3 .superseded_by)" = null ] || { echo "    #3 names a successor that did not land"; exit 1; }
[ "$(field 3 '.evidence[] | select(.kind == "supersession") | .status')" = not_landed ] \
  || { echo "    #3's supersession evidence does not say not_landed"; field 3 .evidence; exit 1; }
[ "$(field 5 .disposition)" = waiting_for_dependency ] || { echo "    #5 is $(field 5 .disposition), not waiting_for_dependency"; exit 1; }
[ "$(field 5 '.reasons[0]')" = 'successor_open:#6' ] || { echo "    #5's first reason is $(field 5 '.reasons[0]')"; exit 1; }
# only the named successor that is not open was viewed, with its merge commit asked for: not
# #6 (open), not #2 (read among #1's cross-references)
[ "$(grep -c '^pr view' "$STATE/log")" = 1 ] && grep -q '^pr view 4 --json number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles' "$STATE/log" \
  || { echo "    the successors viewed are not exactly #4:"; grep '^pr view' "$STATE/log"; exit 1; }

# ---------------------------------------------------------------- 3. cleanup
out="$(prs cleanup --format json)" || { echo "    cleanup failed: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -c '[.[] | [.pr, .disposition, .action]]')" = '[[1,"superseded","would_close"]]' ] \
  || { echo "    the dry cleanup does not list exactly #1 to close: $out"; exit 1; }
grep -q '^CLOSE' "$STATE/log" && { echo "    a cleanup without --apply closed something"; exit 1; }
prs cleanup --apply >/dev/null || { echo "    cleanup --apply failed"; exit 1; }
[ "$(grep -c '^CLOSE' "$STATE/log")" = 1 ] && grep -q '^CLOSE 1$' "$STATE/log" \
  || { echo "    not exactly #1 was closed:"; grep '^CLOSE' "$STATE/log"; exit 1; }
grep -q '^pr close 1 --comment Superseded by #2, which landed\.' "$STATE/log" \
  || { echo "    #1's closing comment does not open with its successor:"; grep -A1 '^pr close' "$STATE/log"; exit 1; }
grep -q 'its work is already on' "$STATE/log" \
  && { echo "    the closing comment of a superseded pull request claims its own work is on master"; exit 1; }
ev="$W/.git/majordomus/integration/events.jsonl"
jq -e 'select(.action == "closed_superseded" and .pr == 1)' "$ev" >/dev/null \
  || { echo "    the trail does not record #1 as closed_superseded"; cat "$ev"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    a successor that landed supersedes; one open holds; one closed unmerged releases its hold"
