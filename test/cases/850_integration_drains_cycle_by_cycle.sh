# majordomus-covers: none
# majordomus-timeout: 1800
# The executor re-plans after every merge, across several drains, and no merge acts on a plan
# taken before the merge that preceded it (rule project.integration-follows-the-current-master,
# ADR 0101). The whole loop runs through the command line against a scripted forge and a local
# bare origin, the harness case 720 built, with five pull requests whose dispositions change as
# master moves:
#
#   #3  ready, shares no path with another        -> merged in the first drain
#   #1  ready, older than #5, shares one.txt      -> behind once #3 lands: refreshed, its checks
#                                                    reported, merged in the third drain
#   #2  depends on #1 and is stacked on it        -> waits for #1, then behind: refreshed,
#                                                    checked, merged in the fifth drain
#   #4  its required check fails                  -> needs_repair throughout, never merged
#   #5  makes the change #1 makes                 -> ready, then behind, redundant once #1 lands
#
# A merge lands as a merge commit, as GitHub's merge method makes it, so after any merge every
# other open pull request is behind master and needs master brought in before it may merge
# (the rule: a pull request merges only when it contains the current master). One drain can
# therefore merge one pull request of a queue like this one; the next is refreshed, its checks
# run on the new head, and a later drain merges it. That is the cycle asserted here.
#
# The forge here answers from the origin as GitHub does: a pull request's head is its branch's
# tip, and refs/pull/<n>/head follows it; checks are per commit, so a refreshed head has none
# until this case reports them. Every call is logged, and the audit trail is read back.
#
# Asserted, in order:
#   1. the first observation: every disposition, and the rank. After lane, disposition and
#      risk, the rank counts the other ready pull requests that share an authored path (fewer
#      first: landing it invalidates less), and only then age: #3 shares nothing and goes
#      first; #1 and #5 share one.txt, and #1 is older
#   2. drain --max 5 merges #3 and then, observing again, finds nothing ready: #1 and #5 are
#      behind the master #3 produced, #2 still waits for #1
#   3. drain --refresh brings master into #1 (ranked before #5: older), pushes, and stops; with
#      its checks unreported it waits, with them passed it is ready, and the next drain merges
#      it; #5 is redundant then, and #2 behind
#   4. the same cycle for #2, after which nothing is ready and nothing is left but #4 and #5
#   5. every merge was selected against the master the merge before it produced, with the forge
#      observed twice in between: no merge acted on a plan taken before the previous merge
#   6. the decision events of the trail, in order, are exactly those; #4 and #5 never reached
#      the forge's merge, and no call carried --admin
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-850.git"; W="$T/../work-850"; STATE="$T/../forge-850"; BIN="$T/../bin-850"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE/ci" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
# the person's identity, in the clone: the executor merges and commits here, and a machine
# with none configured (a CI runner) must not decide the outcome
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
# drain --refresh regenerates the derived artifacts with the repository's own scripts/derive, as
# this repository's is; the fixture carries one that has nothing to derive
mkdir -p scripts; printf '#!/bin/sh\nexit 0\n' > scripts/derive; chmod +x scripts/derive
echo base > a.txt; gitq add a.txt scripts/derive; gitq commit -qm base; gitq push -q origin HEAD:master
M0="$(git rev-parse HEAD)"

branch() {   # <name> <from> <file> <content>: a branch with one commit, pushed
  gitq checkout -q -b "$1" "$2"; echo "$4" > "$3"; gitq add "$3"; gitq commit -qm "$1"
  gitq push -q origin "HEAD:refs/heads/$1"
}
branch feature/1 "$M0" one.txt one
branch feature/2 feature/1 two.txt two
branch feature/3 "$M0" three.txt three
branch feature/4 "$M0" four.txt four
branch feature/5 "$M0" one.txt one
gitq checkout -q master
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H3="$(git rev-parse feature/3)"
H4="$(git rev-parse feature/4)"; H5="$(git rev-parse feature/5)"
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed in the scratch clone:"; tail -5 "$STATE/init.log"; exit 1; }

# the pull requests: number, branch, created, body
printf '%s\t%s\t%s\t%s\n' \
  1 feature/1 2026-09-01T00:00:00Z "" \
  2 feature/2 2026-09-02T00:00:00Z "Depends on #1" \
  3 feature/3 2026-09-03T00:00:00Z "" \
  4 feature/4 2026-09-04T00:00:00Z "" \
  5 feature/5 2026-09-05T00:00:00Z "" > "$STATE/prs.tsv"
# checks per commit: every first head passes but #4's
for h in "$H1" "$H2" "$H3" "$H5"; do echo SUCCESS > "$STATE/ci/$h"; done
echo FAILURE > "$STATE/ci/$H4"

cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case " \$* " in *" --admin "*) echo "ADMIN" >> "$STATE/log"; exit 1 ;; esac
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    # each open pull request, its head the branch's tip now, and refs/pull/<n>/head following it
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
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$n" ;;
  "pr view")
    case " \$* " in
      # the refresh's read of a declared dependency that is no longer open (ADR 0101: it is
      # satisfied by a merge alone), as the forge's JSON
      *" --json number,state,headRefOid,body "*)
        [ -f "$STATE/merged-\$3" ] || { echo "UNEXPECTED" >> "$STATE/log"; exit 1; }
        printf '{"number":%s,"state":"MERGED","headRefOid":"%s","body":""}\n' \
          "\$3" "\$(git -C "$ORIGIN" rev-parse "refs/pull/\$3/head")" ;;
      *) if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
    esac ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
: > "$STATE/log"
# the trail is the repository's, under the common git directory (ADR 0101 §9)
ev="$(git -C "$W" rev-parse --path-format=absolute --git-common-dir)/majordomus/integration/events.jsonl"
disp() { printf '%s' "$q" | jq -r --argjson n "$1" '.assessments[] | select(.number == $n) | .disposition'; }
expect_disp() {   # <number> <disposition> <when>
  [ "$(disp "$1")" = "$2" ] || { echo "    $3: #$1 is $(disp "$1"), not $2"; printf '%s' "$q" | jq -c --argjson n "$1" '.assessments[] | select(.number == $n) | {disposition, reasons}'; exit 1; }
}
master() { git -C "$ORIGIN" rev-parse master; }

# ---------------------------------------------------------------- 1. the first observation
prs refresh >/dev/null || { echo "    the first refresh failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)"
expect_disp 1 ready "first observation"
expect_disp 2 waiting_for_dependency "first observation"
expect_disp 3 ready "first observation"
expect_disp 4 needs_repair "first observation"
expect_disp 5 ready "first observation"
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 3 ] || { echo "    the first merge is #$(printf '%s' "$q" | jq -r .next_merge), not #3, the one ready pull request that shares no path:"; printf '%s' "$q" | jq -c '.assessments[] | {number, disposition, rank: (.rank // null), factors: (.rank_factors // .factors // null)}' | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- 2. the first drain
out="$(prs drain --max 5)" || { echo "    the first drain failed: $out"; tail -8 "$STATE/log"; exit 1; }
merged() { grep '^pr merge' "$STATE/log" | awk '{print $3}' | tr '\n' ' '; }
[ "$(merged)" = "3 " ] || { echo "    the first drain merged [$(merged)], not #3 alone:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
grep -q "^pr merge 3 --merge --match-head-commit $H3\$" "$STATE/log" || { echo "    #3 was not merged pinned to its head"; exit 1; }
case "$out" in *"nothing is ready"*) ;; *) echo "    the first drain did not stop because nothing was ready: $out"; exit 1 ;; esac
q="$(prs status --format json)" || { echo "    status after the first drain says its observation is stale"; exit 1; }
expect_disp 1 needs_refresh "after #3 landed"
expect_disp 2 waiting_for_dependency "after #3 landed"
expect_disp 4 needs_repair "after #3 landed"
expect_disp 5 needs_refresh "after #3 landed"
sel() { jq -c --argjson n "$1" --arg a "${2:-selected}" 'select(.action == $a and .pr == $n) | .passed_over' "$ev" | head -n 1; }
[ "$(sel 3)" = "[1,5]" ] || { echo "    #3 was chosen over $(sel 3), not [1,5]"; exit 1; }

# refresh <n>: drain --refresh brings master into <n>, pushes, and stops; prints the new head
refresh() {
  local before after o
  before="$(git -C "$ORIGIN" rev-parse "refs/heads/feature/$1")"
  o="$(prs drain --refresh)" || { echo "    the refresh drain for #$1 failed: $o" >&2; tail -8 "$STATE/log" >&2; return 1; }
  after="$(git -C "$ORIGIN" rev-parse "refs/heads/feature/$1")"
  [ "$after" != "$before" ] || { echo "    the refresh drain did not push to #$1: $o" >&2; return 1; }
  git -C "$ORIGIN" merge-base --is-ancestor "$(master)" "$after" || { echo "    #$1's new head does not contain master" >&2; return 1; }
  printf '%s' "$after"
}
# checked <n> <head>: with its checks unreported it waits; reported, it is ready
checked() {
  prs refresh >/dev/null; q="$(prs status --format json)"
  expect_disp "$1" waiting_for_checks "#$1 refreshed, its checks unreported"
  echo SUCCESS > "$STATE/ci/$2"
  prs refresh >/dev/null; q="$(prs status --format json)"
  expect_disp "$1" ready "#$1 refreshed, its checks passed"
}

# ---------------------------------------------------------------- 3. #1: refreshed, checked, merged
H1b="$(refresh 1)" || exit 1
[ "$(sel 1 refresh_selected)" = "[5]" ] || { echo "    master was brought into #1 over $(sel 1 refresh_selected), not [5]"; exit 1; }
[ "$(merged)" = "3 " ] || { echo "    the refresh drain merged something"; exit 1; }
checked 1 "$H1b"
out="$(prs drain --max 5)" || { echo "    the third drain failed: $out"; exit 1; }
[ "$(merged)" = "3 1 " ] || { echo "    the third drain merged [$(merged)], not #1:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
grep -q "^pr merge 1 --merge --match-head-commit $H1b\$" "$STATE/log" || { echo "    #1 was not merged pinned to its refreshed head"; exit 1; }
q="$(prs status --format json)"
expect_disp 2 needs_refresh "after #1 landed"
expect_disp 4 needs_repair "after #1 landed"
expect_disp 5 redundant "after #1 landed: its change is on master"

# ---------------------------------------------------------------- 4. #2: refreshed, checked, merged
H2b="$(refresh 2)" || exit 1
checked 2 "$H2b"
out="$(prs drain --max 5)" || { echo "    the fifth drain failed: $out"; exit 1; }
[ "$(merged)" = "3 1 2 " ] || { echo "    the fifth drain merged [$(merged)], not #2"; exit 1; }
grep -q "^pr merge 2 --merge --match-head-commit $H2b\$" "$STATE/log" || { echo "    #2 was not merged pinned to its refreshed head"; exit 1; }
case "$out" in *"nothing is ready"*) ;; *) echo "    the last drain did not end with nothing ready: $out"; exit 1 ;; esac
for h in "$H3" "$H1b" "$H2b"; do git -C "$ORIGIN" merge-base --is-ancestor "$h" "$(master)" || { echo "    master does not contain $h"; exit 1; }; done

# ---------------------------------------------------------------- 5. no plan outlives a merge
replanned() {   # <merged first> <selected next>
  local a b n
  a="$(jq -r --argjson n "$1" 'select(.action == "merge_succeeded" and .pr == $n) | .master_after' "$ev")"
  b="$(jq -r --argjson n "$2" 'select(.action == "selected" and .pr == $n) | .master_before' "$ev")"
  [ -n "$a" ] && [ "$a" = "$b" ] || { echo "    #$2 was selected against master ${b:-?}, not the ${a:-?} #$1's merge produced"; exit 1; }
  n="$(awk -v x="^pr merge $1 " -v y="^pr merge $2 " '$0 ~ x {on=1; next} $0 ~ y {on=0} on && /^pr list/' "$STATE/log" | wc -l | tr -d ' ')"
  [ "$n" -ge 2 ] || { echo "    the forge was observed $n time(s) between the merges of #$1 and #$2"; exit 1; }
}
replanned 3 1
replanned 1 2

# ---------------------------------------------------------------- 6. the trail and the forge
seq="$(jq -r 'select(.action | test("^(selected|refresh_selected|stale_decision|merge_attempted|merge_succeeded|merge_failed|verification_failed|refreshed|refresh_failed)$")) | "\(.action) \(.pr)"' "$ev" | tr '\n' ',')"
want="selected 3,merge_attempted 3,merge_succeeded 3,refresh_selected 1,refreshed 1,selected 1,merge_attempted 1,merge_succeeded 1,refresh_selected 2,refreshed 2,selected 2,merge_attempted 2,merge_succeeded 2,"
[ "$seq" = "$want" ] || { echo "    the decision events differ from the scenario:"; echo "      got:  $seq"; echo "      want: $want"; exit 1; }
grep -qE '^pr merge (4|5) ' "$STATE/log" && { echo "    #4 or #5 reached the forge's merge"; exit 1; }
grep -q ADMIN "$STATE/log" && { echo "    a call carried --admin"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    the executor asked the forge something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    five drains: #3; #1 refreshed and merged; #2 refreshed and merged; each against the master before it; #4 and #5 never"
