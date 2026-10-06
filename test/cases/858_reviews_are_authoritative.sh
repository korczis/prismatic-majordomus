# majordomus-covers: none
# An approval is of the commit it was given on, and the forge's word for a review is not the
# last word (ADR 0101). Through the real command line against a scripted forge, as case 720
# does, with a base whose protection requires one approving review:
#
#   #1  approved on its head                      -> ready
#   #2  approved on an earlier commit; the forge still says APPROVED (dismissal is off)
#                                                 -> stale: waiting for review, never ready
#   #3  REVIEW_REQUIRED, a reviewer asked          -> pending: waiting for review
#   #4  CHANGES_REQUESTED                          -> waiting for review
#
# and the explanation names each review with the commit it was given on.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-858.git"; W="$T/../work-858"; STATE="$T/../forge-858"; BIN="$T/../bin-858"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
for n in 1 2 3 4; do
  gitq checkout -qb "feature/$n" master; echo "$n" > "$n.txt"; gitq add "$n.txt"; gitq commit -qm "$n"
  [ "$n" = 2 ] && { OLD2="$(git rev-parse HEAD)"; echo more >> "$n.txt"; gitq commit -qam "$n again"; }
  gitq push -q origin "HEAD:refs/heads/feature/$n" "HEAD:refs/pull/$n/head"
done
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H3="$(git rev-parse feature/3)"; H4="$(git rev-parse feature/4)"
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

ci='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:05:00Z"}'
review() { printf '{"author":{"login":"%s"},"state":"%s","commit":{"oid":"%s"}}' "$1" "$2" "$3"; }
pr() {   # <number> <head> <decision> <latestReviews> <reviewRequests>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[%s],"reviewDecision":"%s","latestReviews":[%s],"reviewRequests":[%s],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$2" "$1" "$1" "$ci" "$3" "$4" "$5"
}
printf '[%s,%s,%s,%s]\n' \
  "$(pr 1 "$H1" APPROVED "$(review ana APPROVED "$H1")" "")" \
  "$(pr 2 "$H2" APPROVED "$(review ana APPROVED "$OLD2")" "")" \
  "$(pr 3 "$H3" REVIEW_REQUIRED "" '{"login":"bo"}')" \
  "$(pr 4 "$H4" CHANGES_REQUESTED "$(review cy CHANGES_REQUESTED "$H4")" "")" > "$STATE/prs.json"

cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection")
    echo '{"required_status_checks":{"contexts":["ci"]},"required_pull_request_reviews":{"required_approving_review_count":1,"dismiss_stale_reviews":false}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") cat "$STATE/prs.json" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
prs refresh >/dev/null || { echo "    refresh failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)" || { echo "    status failed after a refresh"; exit 1; }
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
expect() {   # <number> <review> <disposition>
  [ "$(field "$1" .review)/$(field "$1" .disposition)" = "$2/$3" ] \
    || { echo "    #$1 is $(field "$1" .review)/$(field "$1" .disposition), not $2/$3"; field "$1" '{reasons, evidence}'; exit 1; }
}
expect 1 approved ready
expect 2 stale waiting_for_review
expect 3 pending waiting_for_review
expect 4 changes_requested waiting_for_review
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 1 ] || { echo "    the next merge is not #1, the one approved on its head"; exit 1; }
out="$(prs explain 2)"
case "$out" in *stale*) ;; *) echo "    explain #2 does not say stale:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
case "$out" in *"ana on another commit"*) ;; *) echo "    explain #2 does not name the review and its commit:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;; esac
grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    approved on its head merges; approved on another commit, asked, or changes requested waits"
