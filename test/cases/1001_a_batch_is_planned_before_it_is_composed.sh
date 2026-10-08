# majordomus-covers: none
# majordomus-timeout: 900
# A batch is planned by a read, composed only by the executor, and only once a person
# accepted the decision (ADR 0114 D1, D2, D3, D7; rule
# project.integration-follows-the-current-master, version 4).
#
# The five batches of 2026-10-07 and 2026-10-08 were composed by a session's hands. A member
# went in because its owner said it was green: #739 rode with one failing case and 331
# uncovered changed items, and #745's first real verdict came inside a batch. Which members
# a batch carried, at which heads and in what order, existed in a pull request's prose.
#
# What this holds, through the real command line in a fixture repository with a bare origin
# and a scripted `gh` whose every call is logged. Three pull requests are open: #1 and #2
# passed their required check on their own heads, #3's is still running.
#
#   1. `prs compose` with no flag is a read: it names #1 and #2 in order, leaves #3 out and
#      says why, asks the forge nothing at all, writes nothing to the trail and pushes
#      nothing
#   2. `prs compose --apply` is refused (exit 10) while the layer's ADR 0114 is `proposed`,
#      naming the ADR, before the forge is asked; and with the decision accepted it is still
#      refused until the trail holds a verified merge
#   3. with the layer's copy of the ADR `accepted` and one verified merge on the trail, it
#      composes: a new branch `int/batch-<id>` holding one merge per member and one commit,
#      a manifest that lists #1 and #2 in that order at their heads, a pull request whose
#      body carries exactly one `Supersedes #n` per member, and #3 left out and named. The
#      base is not moved and the forge is never asked to merge
#   4. the branch the executor composed is the batch the gate accepts
#
# The fixture's copy of the decision is this repository's own file with the status set, so
# the case holds whatever the owner has decided here. The act's rarer endings (a conflict, a
# failed derive, a pull request that cannot be opened, the one bump) are held by
# apps/majordomus-cli/tests/integration_compose.rs, not here.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1001.git"; W="$T/../work-1001"; STATE="$T/../forge-1001"; BIN="$T/../bin-1001"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b main "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
git config user.email t@example.com; git config user.name t
common="$(git rev-parse --path-format=absolute --git-common-dir)"
TRAIL="$common/majordomus/integration/events.jsonl"
# the composition runs the derive of the tree it merged; this tree has nothing derived
mkdir -p scripts
printf '#!/bin/sh\nexit 0\n' > scripts/derive; chmod +x scripts/derive
echo base > a.txt
gitq add -A; gitq commit -qm base; gitq push -q origin HEAD:main
M0="$(git rev-parse HEAD)"
branch() {   # <name> <file>: one commit on the base, pushed
  gitq checkout -q -b "$1" "$M0"; echo "$1" > "$2"
  gitq add -A; gitq commit -qm "$1"; gitq push -q origin "HEAD:refs/heads/$1"
}
branch feature/1 one.txt
branch feature/2 two.txt
branch feature/3 three.txt
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H3="$(git rev-parse feature/3)"
# master moves after all three were opened: every one of them is behind it
gitq checkout -q --detach "$M0"
echo later > later.txt; gitq add -A; gitq commit -qm later; gitq push -q origin HEAD:main
M1="$(git rev-parse HEAD)"

# the layer, as little of one as `prs` reads: a manifest naming the policy and the
# decisions, the size of a batch, and this repository's own ADR 0114 with the status set
ADR_SOURCE="$(printf '%s\n' "$ROOT"/.ai/repo/adrs/0114-*.md)"
[ -f "$ADR_SOURCE" ] || { echo "    ADR 0114 is not one file in this tree: $ADR_SOURCE"; exit 1; }
mkdir -p .ai/repo/adrs
printf 'schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  adrs: repo/adrs\n' > .ai/manifest.yaml
printf 'version: 1\nintegration:\n  batch:\n    max_members: 4\n' > .ai/repo/policy.yaml
decision() { sed "s/^status: .*/status: $1/" "$ADR_SOURCE" > .ai/repo/adrs/0114-a-batch-is-composed.md; }
decision proposed

# the scripted forge: every call is logged; `pr create` records what it was asked to open
printf '%s\t%s\t%s\n' 1 feature/1 COMPLETED  2 feature/2 COMPLETED  3 feature/3 IN_PROGRESS > "$STATE/prs.tsv"
: > "$STATE/log"
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"main"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/main") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse main)" ;;
  "api repos/o/r/branches/main/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/main") echo '[]' ;;
  "pr list")
    printf '['; sep=''
    while IFS='	' read -r n br status; do
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      git -C "$ORIGIN" update-ref "refs/pull/\$n/head" "\$h"
      conclusion=''; [ "\$status" = COMPLETED ] && conclusion=SUCCESS
      printf '%s{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"main","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"%s","conclusion":"%s"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h" "\$n" "\$n" "\$status" "\$conclusion"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  # the declarations read (ADR 0101 §6, D4): every open pull request is an owner's, a branch of
  # this repository, and no pull request mentions it. The list is this forge's own, unlogged
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  "pr create")
    while [ \$# -gt 0 ]; do
      case "\$1" in
        --body) printf '%s\n' "\$2" > "$STATE/created-body" ;;
        --head) printf '%s\n' "\$2" > "$STATE/created-head" ;;
        --base) printf '%s\n' "\$2" > "$STATE/created-base" ;;
      esac
      shift
    done
    echo 'https://github.com/o/r/pull/77' ;;
  *) echo "WRITE-OR-UNEXPECTED \$*" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
batches() { git -C "$ORIGIN" for-each-ref --format='%(refname:short)' refs/heads/int | tr '\n' ' '; }
trail_sum() { if [ -f "$TRAIL" ]; then cksum < "$TRAIL"; else echo none; fi; }
prs refresh >/dev/null 2>"$STATE/refresh.err" || { echo "    the refresh failed:"; cat "$STATE/refresh.err"; exit 1; }

# ---------------------------------------------------------------- 1. the dry run is a read
: > "$STATE/log"
before="$(trail_sum)"
expect_exit 0 prs compose
expect_grep '^a batch on main [0-9a-f]{10}, at most 4 member\(s\): 2 eligible, 1 left out$'
expect_grep "^ +1\. #1 +$(printf '%s' "$H1" | cut -c1-10) "
expect_grep "^ +2\. #2 +$(printf '%s' "$H2" | cut -c1-10) "
expect_grep '^left out:$'
expect_grep "^  #3 +$(printf '%s' "$H3" | cut -c1-10)  its required checks have not passed on its head \(pending\)"
expect_grep '^dry run: would merge the members in that order'
expect_exit 0 prs compose --dry-run --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '[(.dry_run | tostring), .outcome.outcome, ([.plan.members[].number] | join(",")), (.plan.left_out[0].number | tostring), .plan.left_out[0].reason.kind] | join(" ")')" = "true would_compose 1,2 3 required_checks" ] \
  || { echo "    the plan is: $LAST_OUT"; exit 1; }
[ ! -s "$STATE/log" ] || { echo "    a dry run asked the forge:"; cat "$STATE/log"; exit 1; }
[ "$(trail_sum)" = "$before" ] || { echo "    a dry run wrote to the trail:"; tail -3 "$TRAIL"; exit 1; }
[ -z "$(batches)" ] || { echo "    a dry run pushed: $(batches)"; exit 1; }

# ---------------------------------------------------------------- 2. refused while proposed
got=0; out="$(prs compose --apply 2>&1)" || got=$?
[ "$got" = 10 ] || { echo "    --apply under a proposed decision exited $got, not 10: $out"; exit 1; }
case "$out" in *"prs compose --apply is refused while ADR 0114 (adr-0114) is not accepted"*) ;;
  *) echo "    the refusal does not name the decision: $out"; exit 1 ;; esac
case "$out" in *proposed*) ;; *) echo "    the refusal does not say the status it read: $out"; exit 1 ;; esac
case "$out" in *"a person's act"*) ;; *) echo "    the refusal does not say whose act accepting is: $out"; exit 1 ;; esac
[ ! -s "$STATE/log" ] || { echo "    a refused act asked the forge first:"; cat "$STATE/log"; exit 1; }
[ "$(trail_sum)" = "$before" ] || { echo "    a refused act wrote to the trail"; exit 1; }
[ -z "$(batches)" ] || { echo "    a refused act pushed: $(batches)"; exit 1; }
# the dry run is unaffected by the decision's status
expect_exit 0 prs compose
# accepted, and the trail holds no verified merge yet: refused by the rollout record
decision accepted
got=0; out="$(prs compose --apply 2>&1)" || got=$?
[ "$got" = 10 ] || { echo "    --apply before any verified merge exited $got, not 10: $out"; exit 1; }
case "$out" in *"verified merge(s) on the trail"*"prs drain --max 1"*) ;;
  *) echo "    the refusal does not name the rollout record: $out"; exit 1 ;; esac
[ ! -s "$STATE/log" ] || { echo "    an act refused by the rollout asked the forge first:"; cat "$STATE/log"; exit 1; }
[ -z "$(batches)" ] || { echo "    an act refused by the rollout pushed: $(batches)"; exit 1; }

# ---------------------------------------------------------------- 3. accepted, and one verified merge
mkdir -p "$(dirname "$TRAIL")"
printf '%s\n' '{"at":"2026-10-05T00:00:00Z","actor":"seed","action":"merge_succeeded","pr":90,"reasons":[],"detail":"seeded"}' > "$TRAIL"
expect_exit 0 prs compose --apply --format json
r="$LAST_OUT"
ID="$(printf '%s' "$M1" | cut -c1-10)-1-2"
BRANCH="int/batch-$ID"
[ "$(printf '%s' "$r" | jq -r '[(.dry_run | tostring), .outcome.outcome, .outcome.composed.branch, (.outcome.composed.pull_request | tostring)] | join(" ")')" = "false composed $BRANCH 77" ] \
  || { echo "    the batch was not composed: $r"; tail -5 "$STATE/log"; exit 1; }
[ "$(batches)" = "$BRANCH " ] || { echo "    origin's batch branches are: $(batches)"; exit 1; }
TIP="$(git -C "$ORIGIN" rev-parse "refs/heads/$BRANCH")"
# one merge per member whose second parent is the member's head, then one commit
line="$(git -C "$ORIGIN" rev-list --first-parent --reverse "$M1..$TIP" | tr '\n' ' ')"
set -- $line
[ $# = 3 ] || { echo "    the batch's first-parent line holds $# commit(s), not two merges and one commit"; exit 1; }
[ "$(git -C "$ORIGIN" rev-parse "$1^2" "$2^2" | tr '\n' ' ')" = "$H1 $H2 " ] \
  || { echo "    the merges do not carry #1 and #2 at their heads, in order"; git -C "$ORIGIN" log --oneline --graph -6 "$TIP"; exit 1; }
[ "$(git -C "$ORIGIN" rev-list --parents -1 "$3" | wc -w | tr -d ' ')" = 2 ] || { echo "    the composition commit is a merge"; exit 1; }
# the manifest lists the members in that order, at those heads and merge commits
MANIFEST=".ai/repo/integration/batches/$ID.yaml"
git -C "$ORIGIN" show "$TIP:$MANIFEST" > "$STATE/manifest.yaml" 2>/dev/null \
  || { echo "    the batch carries no $MANIFEST"; git -C "$ORIGIN" ls-tree -r --name-only "$TIP" | head; exit 1; }
grep -q '^schema: integration-batch/v1$' "$STATE/manifest.yaml" || { echo "    the manifest is not integration-batch/v1:"; cat "$STATE/manifest.yaml"; exit 1; }
[ "$(sed -n 's/^  - number: //p' "$STATE/manifest.yaml" | tr '\n' ' ')" = "1 2 " ] \
  || { echo "    the manifest's members are not #1 then #2:"; cat "$STATE/manifest.yaml"; exit 1; }
[ "$(sed -n "s/^    head: '\(.*\)'\$/\1/p" "$STATE/manifest.yaml" | tr '\n' ' ')" = "$H1 $H2 " ] \
  || { echo "    the manifest's heads are not the members':"; cat "$STATE/manifest.yaml"; exit 1; }
[ "$(sed -n "s/^    merge_commit: '\(.*\)'\$/\1/p" "$STATE/manifest.yaml" | tr '\n' ' ')" = "$1 $2 " ] \
  || { echo "    the manifest's merge commits are not the merges on the line:"; cat "$STATE/manifest.yaml"; exit 1; }
grep -q "^base_master: '$M1'\$" "$STATE/manifest.yaml" || { echo "    the manifest does not name the master it was composed on"; exit 1; }
# the composition commit changes the manifest and nothing a member's branch could
[ "$(git -C "$ORIGIN" diff --name-only "$3^" "$3" | tr '\n' ' ')" = "$MANIFEST " ] \
  || { echo "    the composition commit changes: $(git -C "$ORIGIN" diff --name-only "$3^" "$3" | tr '\n' ' ')"; exit 1; }
# the pull request: opened for that branch against the base, one Supersedes per member
[ "$(cat "$STATE/created-head" 2>/dev/null)" = "$BRANCH" ] || { echo "    the pull request was opened for '$(cat "$STATE/created-head" 2>/dev/null)', not $BRANCH"; exit 1; }
[ "$(cat "$STATE/created-base" 2>/dev/null)" = main ] || { echo "    the pull request targets '$(cat "$STATE/created-base" 2>/dev/null)', not main"; exit 1; }
[ "$(grep '^Supersedes #' "$STATE/created-body" | tr '\n' ' ')" = "Supersedes #1 Supersedes #2 " ] \
  || { echo "    the body does not carry one Supersedes per member:"; cat "$STATE/created-body"; exit 1; }
# the member whose required check is pending is left out, and named
[ "$(printf '%s' "$r" | jq -r '[.plan.left_out[] | "\(.number) \(.reason.kind) \(.reason.state)"] | join(";")')" = "3 required_checks pending" ] \
  || { echo "    #3 is not left out for its pending check: $(printf '%s' "$r" | jq -c .plan.left_out)"; exit 1; }
if git -C "$ORIGIN" merge-base --is-ancestor "$H3" "$TIP"; then echo "    #3 rode the batch with its required check pending"; exit 1; fi
# under the lease and on the trail, in order; the base is not moved; the forge merged nothing
[ "$(jq -r .action "$TRAIL" | tr '\n' ' ')" = "merge_succeeded lease_acquired observed compose_selected compose_attempted composed lease_released " ] \
  || { echo "    the trail is: $(jq -r .action "$TRAIL" | tr '\n' ' ')"; exit 1; }
[ "$(git -C "$ORIGIN" rev-parse main)" = "$M1" ] || { echo "    composing moved the base"; exit 1; }
if grep -q 'WRITE-OR-UNEXPECTED\|^pr merge\|^pr close' "$STATE/log"; then echo "    the forge was asked to act:"; grep 'WRITE\|^pr merge\|^pr close' "$STATE/log"; exit 1; fi
[ "$(grep -c '^pr create' "$STATE/log")" = 1 ] || { echo "    the pull request was not opened exactly once"; exit 1; }
[ "$(git -C "$W" worktree list | wc -l | tr -d ' ')" = 1 ] || { echo "    a scratch worktree was left behind:"; git -C "$W" worktree list; exit 1; }
# the same members on the same master are the same batch: a second composition is refused
got=0; out="$(prs compose --apply 2>&1)" || got=$?
[ "$got" != 0 ] || { echo "    the same batch was composed twice: $out"; exit 1; }
[ "$(git -C "$ORIGIN" rev-parse "refs/heads/$BRANCH")" = "$TIP" ] || { echo "    a second composition moved the batch branch"; exit 1; }

# ---------------------------------------------------------------- 4. the gate accepts it
gitq fetch -q origin
expect_exit 0 prs batch-check --base origin/main --head "$TIP"
expect_grep "^batch-check: ok: .* is the batch $MANIFEST says: #1, #2\$"
echo "    the dry run moved and asked nothing, --apply was refused while ADR 0114 was proposed and before a verified merge, and then composed $BRANCH with its manifest, its Supersedes lines and #3 left out"
