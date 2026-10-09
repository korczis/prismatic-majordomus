# majordomus-covers: none
# majordomus-timeout: 600
# Executors in two worktrees of one repository race as executors in one worktree do: the
# lease and the trail live under the common git directory, so a second checkout is not a
# second executor's licence (ADR 0101, WP27). Through the real command line against a
# scripted forge, as case 855 does:
#
#   - six `prs drain` processes released by one file, alternating between the primary
#     checkout and a linked worktree, against one ready pull request
#   - exactly one says it merged, the forge was asked once, master gained one merge commit
#   - every other one was refused by the lease (exit 12) or found nothing to merge (exit 0)
#   - the trail each checkout reads is the same trail: one merge_succeeded, and as many
#     leases given back as taken
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-856.git"; W="$T/../work-856"; W2="$T/../work-856-linked"
STATE="$T/../forge-856"; BIN="$T/../bin-856"
rm -rf "$ORIGIN" "$W" "$W2" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
PIDS=""; trap 'for p in $PIDS; do kill -9 "$p" 2>/dev/null; done' EXIT
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
gitq checkout -qb feature/1; echo one > one.txt; gitq add one.txt; gitq commit -qm one
gitq push -q origin HEAD:refs/heads/feature/1 HEAD:refs/pull/1/head
H1="$(git rev-parse feature/1)"
gitq checkout -q master
gitq worktree add -q --detach "$W2" master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }
(cd "$W2" && "$MJ" init >>"$STATE/init.log" 2>&1) || { echo "    majordomus init failed in the linked worktree"; exit 1; }

printf '[{"number":1,"title":"change 1","author":{"login":"someone"},"headRefName":"feature/1","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}]\n' "$H1" > "$STATE/prs.json"
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") if [ -f "$STATE/merged-1" ]; then echo '[]'; else cat "$STATE/prs.json"; fi ;;
  "pr merge")
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$3/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$3" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$3" ;;
  "pr view") if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
  # the declarations read (ADR 0101 §6, D4): every open pull request is an owner's, a branch of
  # this repository, and no pull request mentions it. The list is this forge's own, unlogged
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
for c in "$W" "$W2"; do
  "$RB" prs --repo "$c" refresh >/dev/null || { echo "    refresh failed in $c"; tail -5 "$STATE/log"; exit 1; }
done
LOCK="$(git rev-parse --path-format=absolute --git-common-dir)/majordomus/locks/integration-master.lock"

# six executors, alternating between the two checkouts, released together
for i in 1 2 3 4 5 6; do
  if [ $((i % 2)) = 1 ]; then c="$W"; else c="$W2"; fi
  ( until [ -f "$STATE/go" ]; do sleep 0.01; done
    exec "$RB" prs --repo "$c" drain --max 1 ) > "$STATE/out.$i" 2>&1 &
  PIDS="$PIDS $!"
done
sleep 0.5; touch "$STATE/go"
merged=0; refused=0; i=0
for p in $PIDS; do
  i=$((i + 1)); rc=0; wait "$p" || rc=$?
  out="$(cat "$STATE/out.$i")"
  case "$rc" in
    0) case "$out" in *"merged #1"*) merged=$((merged + 1)) ;; esac ;;
    12) case "$out" in
          *"another integration executor holds"*) refused=$((refused + 1)) ;;
          *) echo "    executor $i failed (exit 12) without naming the lease:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;;
        esac ;;
    *) echo "    executor $i exited $rc:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;;
  esac
done
PIDS=""
[ "$merged" = 1 ] || { echo "    $merged executors say they merged #1, not 1"; tail -n 3 "$STATE"/out.*; exit 1; }
[ "$(grep -c '^pr merge 1 ' "$STATE/log")" = 1 ] || { echo "    the forge was asked to merge #1 $(grep -c '^pr merge 1 ' "$STATE/log") times"; exit 1; }
[ "$(git -C "$ORIGIN" rev-list --count --merges master)" = 1 ] || { echo "    master does not carry exactly one merge commit"; exit 1; }
[ "$refused" -gt 0 ] || { echo "    no executor was refused: the processes never overlapped, so nothing raced"; exit 1; }
[ ! -e "$LOCK" ] || { echo "    the lease was left behind: $(cat "$LOCK")"; exit 1; }

# one trail, read the same from either checkout
t1="$("$RB" prs --repo "$W" events --format json 2>/dev/null)" || { echo "    events failed in the primary checkout"; exit 1; }
t2="$("$RB" prs --repo "$W2" events --format json 2>/dev/null)" || { echo "    events failed in the linked worktree"; exit 1; }
[ "$t1" = "$t2" ] || { echo "    the two checkouts read different trails"; exit 1; }
count() { printf '%s' "$t1" | jq --arg a "$1" '[.. | objects | select(.action? == $a)] | length'; }
[ "$(count merge_succeeded)" = 1 ] || { echo "    the trail holds $(count merge_succeeded) merge_succeeded, not 1"; exit 1; }
[ "$(count lease_acquired)" = "$(count lease_released)" ] && [ "$(count lease_acquired)" -ge 1 ] \
  || { echo "    leases taken $(count lease_acquired), given back $(count lease_released)"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    6 executors across 2 worktrees: one merge, $refused refused by the lease, one trail"
