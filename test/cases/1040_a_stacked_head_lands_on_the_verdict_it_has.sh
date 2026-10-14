# majordomus-covers: none
# A pull request stacked on one that landed merges on the verdict its head already has (ADR
# 0129, issue I2298). Through the command line, against the scripted forge and the local bare
# origin case 850 built. Three pull requests:
#
#   #1  contains master, its checks passed                  -> ready, merged first
#   #2  built on #1's head, says "Stacked on #1"            -> waits for #1; then it carries
#                                                              master and merges as it stands
#   #3  built on #1's head too, says "Stacked on #1"        -> carries master while master is
#                                                              #1's merge; behind once #2 landed
#
# The forge lands a pull request as a merge commit, so after #1 lands neither #2 nor #3
# contains master. What decides them is what merging them would write.
#
# Asserted, in order:
#   1. the first observation: #1 is ready and says contains_master; #2 and #3 wait for #1
#   2. drain --max 1 merges #1 alone. Observed again, #2 does not contain master, its relation
#      is carries_master, every gate passes, and it is ready for the reasons carries_master
#      and required_checks_passed; the executor would merge it next, and repair has nothing to
#      repair on it
#   3. drain --max 1, with no --refresh, merges #2 pinned to the head it always had: its branch
#      never moved, nothing was refreshed or repaired, and master's tree is that head's tree
#   4. #3 was built on #1 and never saw #2: master now holds a file its head lacks, so it is
#      behind, needs a refresh, and a repair would bring master into it
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1040.git"; W="$T/../work-1040"; STATE="$T/../forge-1040"; BIN="$T/../bin-1040"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE/ci" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
M0="$(git rev-parse HEAD)"

branch() {   # <name> <from> <file> <content>: a branch with one commit, pushed
  gitq checkout -q -b "$1" "$2"; echo "$4" > "$3"; gitq add "$3"; gitq commit -qm "$1"
  gitq push -q origin "HEAD:refs/heads/$1"
}
branch feature/1 "$M0" one.txt one
branch feature/2 feature/1 two.txt two
branch feature/3 feature/1 three.txt three
gitq checkout -q master
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H3="$(git rev-parse feature/3)"
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed in the scratch clone:"; tail -5 "$STATE/init.log"; exit 1; }

# the pull requests: number, branch, created, body
printf '%s\t%s\t%s\t%s\n' \
  1 feature/1 2026-09-01T00:00:00Z "" \
  2 feature/2 2026-09-02T00:00:00Z "Stacked on #1" \
  3 feature/3 2026-09-03T00:00:00Z "Stacked on #1" > "$STATE/prs.tsv"
# checks are per commit, and every head here passed: no head moves in this case, so no check
# is ever reported a second time
for h in "$H1" "$H2" "$H3"; do echo SUCCESS > "$STATE/ci/$h"; done

cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case " \$* " in *" --admin "*) echo "ADMIN" >> "$STATE/log"; exit 1 ;; esac
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    printf '['; sep=''
    while IFS='	' read -r n br created body; do
      [ -f "$STATE/merged-\$n" ] && continue
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      git -C "$ORIGIN" update-ref "refs/pull/\$n/head" "\$h"
      if [ -f "$STATE/ci/\$h" ]; then
        roll="[{\"__typename\":\"CheckRun\",\"name\":\"ci\",\"status\":\"COMPLETED\",\"conclusion\":\"\$(cat "$STATE/ci/\$h")\"}]"
      else roll='[]'; fi
      printf '%s{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"%s","updatedAt":"%s","body":"%s","statusCheckRollup":%s,"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h" "\$created" "\$created" "\$body" "\$roll"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  "pr merge")
    n="\$3"; want=''; prev=''
    for a in "\$@"; do [ "\$prev" = --match-head-commit ] && want="\$a"; prev="\$a"; done
    have="\$(git -C "$ORIGIN" rev-parse "refs/pull/\$n/head")"
    [ -z "\$want" ] || [ "\$want" = "\$have" ] || { echo "head moved" >&2; exit 1; }
    # the forge has no derived driver and no clone's configuration: a plain merge, which must
    # be clean for the merge to happen at all
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$n" ;;
  "pr view")
    case " \$* " in
      # the refresh's read of a declared dependency that is no longer open: satisfied by a
      # merge alone, as the forge's JSON
      *" --json number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles "*)
        [ -f "$STATE/merged-\$3" ] || { echo "UNEXPECTED" >> "$STATE/log"; exit 1; }
        printf '{"number":%s,"state":"MERGED","headRefOid":"%s","body":"","mergeCommit":null,"baseRefName":"master","author":{"login":"someone"},"isCrossRepository":false,"changedFiles":1}\n' \
          "\$3" "\$(git -C "$ORIGIN" rev-parse "refs/pull/\$3/head")" ;;
      *) if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
    esac ;;
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
: > "$STATE/log"
ev="$(git -C "$W" rev-parse --path-format=absolute --git-common-dir)/majordomus/integration/events.jsonl"
of() { printf '%s' "$q" | jq -c --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
expect_of() {   # <number> <jq of the assessment> <want, as compact JSON> <when>
  [ "$(of "$1" "$2")" = "$3" ] || {
    echo "    $4: #$1 has $2 = $(of "$1" "$2"), not $3"
    of "$1" '{disposition, relation, reasons}'; exit 1; }
}
merged() { grep '^pr merge' "$STATE/log" | awk '{print $3}' | tr '\n' ' '; }
master() { git -C "$ORIGIN" rev-parse master; }

# ---------------------------------------------------------------- 1. the first observation
prs refresh >/dev/null || { echo "    the first refresh failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)"
expect_of 1 .disposition '"ready"' "first observation"
expect_of 1 .reasons '["contains_master","required_checks_passed"]' "first observation"
expect_of 2 .disposition '"waiting_for_dependency"' "first observation"
expect_of 2 '(.reasons | index("depends_on:#1") != null)' true "first observation"
expect_of 3 .disposition '"waiting_for_dependency"' "first observation"
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 1 ] || { echo "    the first merge is not #1"; exit 1; }

# ---------------------------------------------------------------- 2. #1 lands; #2 carries master
out="$(prs drain --max 1)" || { echo "    the first drain failed: $out"; tail -8 "$STATE/log"; exit 1; }
[ "$(merged)" = "1 " ] || { echo "    the first drain merged [$(merged)], not #1 alone: $out"; exit 1; }
M1="$(master)"
git -C "$ORIGIN" merge-base --is-ancestor "$M1" "$H2" \
  && { echo "    the fixture is wrong: #2's head contains the master #1's merge made"; exit 1; }
prs refresh >/dev/null || { echo "    the refresh after #1 failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)"
expect_of 2 .relation.kind '"carries_master"' "after #1 landed"
expect_of 2 .relation.behind 1 "after #1 landed"
expect_of 2 .disposition '"ready"' "after #1 landed"
expect_of 2 .reasons '["carries_master","required_checks_passed"]' "after #1 landed"
expect_of 2 '([.gates[] | select(.passed | not) | .gate])' '[]' "after #1 landed"
expect_of 2 .evaluated_against.head_sha "\"$H2\"" "after #1 landed"
expect_of 3 .relation.kind '"carries_master"' "after #1 landed"
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 2 ] || { echo "    the next merge is #$(printf '%s' "$q" | jq -r .next_merge), not #2"; exit 1; }
[ "$(printf '%s' "$q" | jq -c .next_refresh)" = "[]" ] || { echo "    something needs a refresh: $(printf '%s' "$q" | jq -c .next_refresh)"; exit 1; }
prs explain 2 | grep -E "relation: +carries_master" >/dev/null || { echo "    explain does not say #2 carries master:"; prs explain 2 | sed 's/^/      /'; exit 1; }
out="$(prs repair 2 2>&1)" || true
case "$out" in *"yields its own tree"*) ;; *) echo "    repair does not say #2 has nothing to repair: $out"; exit 1 ;; esac

# ---------------------------------------------------------------- 3. #2 merges as it stands
out="$(prs drain --max 1)" || { echo "    the second drain failed: $out"; tail -8 "$STATE/log"; exit 1; }
[ "$(merged)" = "1 2 " ] || { echo "    the drains merged [$(merged)], not #1 then #2: $out"; exit 1; }
grep -q "^pr merge 2 --merge --match-head-commit $H2\$" "$STATE/log" \
  || { echo "    #2 was not merged pinned to the head it always had:"; grep '^pr merge' "$STATE/log"; exit 1; }
[ "$(git -C "$ORIGIN" rev-parse refs/heads/feature/2)" = "$H2" ] || { echo "    #2's branch moved"; exit 1; }
if jq -e 'select(.action | test("^(refresh|repair)"))' "$ev" >/dev/null; then
  echo "    something was refreshed or repaired on the way:"; jq -c 'select(.action | test("^(refresh|repair)")) | {action, pr}' "$ev"; exit 1
fi
[ "$(git -C "$ORIGIN" rev-parse 'master^{tree}')" = "$(git -C "$ORIGIN" rev-parse "$H2^{tree}")" ] \
  || { echo "    master's tree is not the tree #2's checks judged"; exit 1; }
jq -e 'select(.action == "merge_attempted" and .pr == 2 and (.reasons | index("carries_master") != null))' "$ev" >/dev/null \
  || { echo "    the trail does not say #2 merged because it carried master:"; jq -c 'select(.pr == 2) | {action, reasons}' "$ev"; exit 1; }
grep -q ADMIN "$STATE/log" && { echo "    the executor passed --admin"; exit 1; }

# ---------------------------------------------------------------- 4. #3 never saw #2
prs refresh >/dev/null || { echo "    the refresh after #2 failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)"
expect_of 3 .relation.kind '"behind"' "after #2 landed"
expect_of 3 .disposition '"needs_refresh"' "after #2 landed"
expect_of 3 '(.reasons | index("behind_master:3") != null)' true "after #2 landed"
[ "$(printf '%s' "$q" | jq -c .next_refresh)" = "[3]" ] || { echo "    #3 is not the next refresh: $(printf '%s' "$q" | jq -c .next_refresh)"; exit 1; }
out="$(prs repair 3 2>&1)" || true
case "$out" in *"dry run: would merge master"*) ;; *) echo "    repair would not bring master into #3, which is behind: $out"; exit 1 ;; esac
exit 0
