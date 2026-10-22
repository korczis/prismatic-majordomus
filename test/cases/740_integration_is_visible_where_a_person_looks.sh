# majordomus-covers: none
# majordomus-timeout: 900
# The integration queue is visible where a person looks — the Cockpit, the session
# briefing, `prs explain` — and every one of them renders the same recorded answer, with no
# network: a scripted forge as in case 720, one real drain, then only reads.
#
#   1. two ready pull requests; the drain merges the older (#1) and records that it passed
#      over #3; after the merge #3 is behind master but still the executor's (refreshable)
#   2. status and explain show #3's wait: actionable since when, passed over once, for #1
#   3. `prs brief` is one line — the queue, the lease, the last merge — and `majordomus
#      context` carries it under INTEGRATION; a checkout that never observed the forge gets
#      neither line nor section
#   4. /cockpit/integration renders the same queue over a real socket: the counts, the next
#      step, the lease, the last merge, the lanes, #3's wait, the recent actions — and the
#      navigation links to it; with nothing observed it says so instead of a blank page
#   5. the trail is the repository's, not the checkout's: a second worktree of the same
#      clone, which never drained anything, shows the merge the first one recorded in its
#      brief and its events, and the lease it took and gave back
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-740.git"; W="$T/../work-740"; STATE="$T/../forge-740"; BIN="$T/../bin-740"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
for n in 1 3; do
  gitq checkout -qb "feature/$n" master; echo "$n" > "f$n.txt"; gitq add "f$n.txt"; gitq commit -qm "$n"
  gitq push -q origin "HEAD:refs/heads/feature/$n" "HEAD:refs/pull/$n/head"
done
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

pr() {   # <number> <created>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"%s","updatedAt":"%s","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$(git rev-parse "feature/$1")" "$2" "$2"
}
pr 1 2026-09-01T00:00:00Z > "$STATE/pr-1.json"
pr 3 2026-09-03T00:00:00Z > "$STATE/pr-3.json"
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "pr list")
    printf '['; sep=''
    for f in "$STATE"/pr-*.json; do
      n="\${f##*/pr-}"; n="\${n%.json}"
      [ -f "$STATE/merged-\$n" ] && continue
      printf '%s' "\$sep"; cat "\$f"; sep=','
    done
    printf ']\n' ;;
  "pr merge")
    n="\$3"; t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$n" ;;
  "pr view") if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }

# ---------------------------------------------------------------- 1. one drain
prs refresh >/dev/null || { echo "    refresh failed"; cat "$STATE/log"; exit 1; }
out="$(prs drain --max 1)" || { echo "    drain failed: $out"; exit 1; }
case "$out" in *"merged #1"*) ;; *) echo "    the drain did not merge #1: $out"; exit 1 ;; esac
ev="$W/.git/majordomus/integration/events.jsonl"
jq -e 'select(.action == "selected" and .pr == 1 and .passed_over == [3])' "$ev" >/dev/null \
  || { echo "    the selection does not name #3 as passed over:"; cat "$ev"; exit 1; }
jq -e 'select(.action == "became_actionable" and .pr == 3)' "$ev" >/dev/null \
  || { echo "    #3 becoming actionable is not in the trail"; exit 1; }
prs refresh >/dev/null

# ---------------------------------------------------------------- 2. the wait, read offline
calls_before="$(wc -l < "$STATE/log" | tr -d ' ')"
q="$(prs status --format json)" || { echo "    status failed"; exit 1; }
[ "$(printf '%s' "$q" | jq -r '.assessments[] | select(.number == 3) | .disposition')" = needs_refresh ] \
  || { echo "    #3 is not needs_refresh after #1 landed"; printf '%s' "$q" | jq '.assessments'; exit 1; }
[ "$(printf '%s' "$q" | jq -r '.assessments[] | select(.number == 3) | .wait.passed_over')" = 1 ] \
  || { echo "    #3's wait does not count the time it was passed over"; printf '%s' "$q" | jq '.assessments[] | select(.number == 3) | .wait'; exit 1; }
[ "$(printf '%s' "$q" | jq -r '.assessments[] | select(.number == 3) | .wait.last_passed_over.for_pr')" = 1 ] \
  || { echo "    #3's wait does not name what it was passed over for"; exit 1; }
prs explain 3 | grep -q "waiting:.*passed over 1×" || { echo "    explain does not show the wait:"; prs explain 3; exit 1; }

# ---------------------------------------------------------------- 3. the briefing
line="$(prs brief)" || { echo "    prs brief failed"; exit 1; }
for want in "master at" "1 open" "lease free" "last merge #1"; do
  case "$line" in *"$want"*) ;; *) echo "    the brief lacks '$want': $line"; exit 1 ;; esac
done
ctx="$("$MJ" context 2>/dev/null)" || true
printf '%s\n' "$ctx" | grep -q '^## INTEGRATION' || { echo "    context has no INTEGRATION section"; printf '%s\n' "$ctx" | grep '^## '; exit 1; }
# the same line — its "N s ago" moves with the clock, so the parts that do not are compared
section="$(printf '%s\n' "$ctx" | sed -n '/^## INTEGRATION/,/^## /p')"
for want in "1 open" "lease free" "last merge #1" "$(printf '%s' "$line" | sed 's/ observed .*//')"; do
  printf '%s\n' "$section" | grep -qF "$want" || { echo "    context's INTEGRATION lacks '$want':"; printf '%s\n' "$section"; exit 1; }
done
[ "$(wc -l < "$STATE/log" | tr -d ' ')" = "$calls_before" ] || { echo "    a read reached the forge"; exit 1; }
# a checkout that never observed the forge: no line, no section
E="$T/../empty-740"; rm -rf "$E"; mkdir -p "$E"; ( cd "$E" && gitq init -q && echo x > x && gitq add x && gitq commit -qm x && "$MJ" init >/dev/null 2>&1 )
[ -z "$("$RB" prs --repo "$E" brief)" ] || { echo "    prs brief said something about a repository it never observed"; exit 1; }
( cd "$E" && "$MJ" context 2>/dev/null ) | grep -q '^## INTEGRATION' && { echo "    context grew an INTEGRATION section with nothing observed"; exit 1; }

# ---------------------------------------------------------------- 5. one trail per repository
B="$T/../work-740-b"; rm -rf "$B"
gitq -C "$W" worktree add -q --detach "$B" master 2>/dev/null || { echo "    no second worktree"; exit 1; }
[ "$(git --no-pager -C "$B" rev-parse --path-format=absolute --git-common-dir)" = "$(git --no-pager -C "$W" rev-parse --path-format=absolute --git-common-dir)" ] \
  || { echo "    the second worktree does not share the first one's git directory"; exit 1; }
"$RB" prs --repo "$B" refresh >/dev/null || { echo "    refresh in the second worktree failed"; exit 1; }
lineb="$("$RB" prs --repo "$B" brief)" || { echo "    prs brief failed in the second worktree"; exit 1; }
case "$lineb" in *"last merge #1"*) ;; *) echo "    the second worktree's brief does not see the first one's merge: $lineb"; exit 1 ;; esac
evb="$("$RB" prs --repo "$B" events --format json)" || { echo "    prs events failed in the second worktree"; exit 1; }
for act in lease_acquired merge_attempted merge_succeeded lease_released; do
  printf '%s' "$evb" | jq -e --arg a "$act" 'map(select(.action == $a)) | length > 0' >/dev/null \
    || { echo "    the second worktree's events lack $act"; printf '%s\n' "$evb" | jq -r '.[].action'; exit 1; }
done
[ ! -e "$B/.ai/local/state/integration/events.jsonl" ] && [ ! -e "$W/.ai/local/state/integration/events.jsonl" ] \
  || { echo "    a checkout keeps a trail of its own"; exit 1; }

# ---------------------------------------------------------------- 4. the Cockpit
serve_up "$STATE/out.txt" "$STATE/err.txt" || exit 1
curl -s -m 60 "$U/cockpit/integration" > "$STATE/page.html"
for want in 'Integration' 'This queue' 'free' 'Needs repair' 'Waiting' 'needs_refresh' '#3' 'passed over 1×' 'Recent actions' 'merge_succeeded' 'prs drain --dry-run'; do
  grep -qF "$want" "$STATE/page.html" || { echo "    /cockpit/integration does not show '$want'"; exit 1; }
done
grep -q 'href="/cockpit/integration"' "$STATE/page.html" || { echo "    the navigation does not link to Integration"; exit 1; }
# the page and the capability are one answer
api="$(curl -s -m 60 "$U/api/v1/pull-requests")"
[ "$(printf '%s' "$api" | jq -r '.lease')" = null ] || { echo "    the capability says the lease is held"; exit 1; }
[ "$(printf '%s' "$api" | jq -r '.last_merge.pr')" = 1 ] || { echo "    the capability's last merge is not #1"; exit 1; }
# throughput is folded from the same trail: a 7-day window, the merge above counted in it, and
# every count an integer rather than a number someone typed
printf '%s' "$api" | jq -e '.throughput.window_days == 7 and .throughput.merges >= 1
  and ([.throughput.merges, .throughput.stale_decisions, .throughput.merge_failures,
        .throughput.verification_failures, .throughput.unreadable_events]
       | all(type == "number" and . == floor))' >/dev/null \
  || { echo "    the capability's throughput is missing, or does not count the merge it reports:"; printf '%s' "$api" | jq -c '.throughput'; exit 1; }
serve_down
# nothing observed: the page says so and still renders
cd "$E" || exit 1
serve_up "$STATE/out2.txt" "$STATE/err2.txt" || exit 1
code="$(curl -s -m 60 -o "$STATE/empty.html" -w '%{http_code}' "$U/cockpit/integration")"
[ "$code" = 200 ] || { echo "    the page with nothing observed answered $code"; exit 1; }
grep -q 'prs refresh' "$STATE/empty.html" || { echo "    the empty page does not say how to observe"; exit 1; }
serve_down
exit 0
