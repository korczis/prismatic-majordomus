# majordomus-covers: none
# majordomus-timeout: 600
# Executors started at one instant on one base: exactly one merges, nothing is merged twice
# (ADR 0101, WP27). The threads of one process race in the unit tests; here separate
# processes do, through the real command line against a scripted forge, as case 741 does.
#
# Three rounds. Each round opens one ready pull request on the current master and starts four
# `prs drain` processes released by one file. In every round:
#
#   - exactly one process says it merged the pull request, and the forge was asked once
#   - every other one was refused by the base's lease, by name (exit 12), or found
#     nothing left to merge (exit 0); none failed otherwise
#   - master gains exactly one merge commit, and no lease is left behind
#
# and across the rounds at least one process was refused: the processes did overlap, so the
# rounds tested a race rather than four executors run one after another.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-855.git"; W="$T/../work-855"; STATE="$T/../forge-855"; BIN="$T/../bin-855"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
PIDS=""; trap 'for p in $PIDS; do kill -9 "$p" 2>/dev/null; done' EXIT
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

# the forge: one open pull request per round, merged by its number; a merge is a real merge
# commit pushed to the origin, so a second merge of the same change would be a second commit
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") n="\$(cat "$STATE/open")"; if [ -f "$STATE/merged-\$n" ]; then echo '[]'; else cat "$STATE/prs.json"; fi ;;
  "pr merge")
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$3/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$3" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$3" ;;
  "pr view") if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
LOCK="$(git rev-parse --path-format=absolute --git-common-dir)/majordomus/locks/integration-master.lock"

refused=0
for r in 1 2 3; do
  # a change on the current master, ready
  gitq fetch -q origin; gitq checkout -q -B "feature/$r" origin/master
  echo "$r" > "$r.txt"; gitq add "$r.txt"; gitq commit -qm "change $r"
  gitq push -q origin "HEAD:refs/heads/feature/$r" "HEAD:refs/pull/$r/head"
  H="$(git rev-parse HEAD)"; gitq checkout -q master; gitq merge -q --ff-only origin/master
  echo "$r" > "$STATE/open"
  printf '[{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}]\n' \
    "$r" "$r" "$r" "$H" > "$STATE/prs.json"
  "$RB" prs --repo "$W" refresh >/dev/null || { echo "    round $r: refresh failed"; tail -5 "$STATE/log"; exit 1; }
  before="$(git -C "$ORIGIN" rev-list --count --merges master)"

  # four executors, each waiting on one file, released together
  rm -f "$STATE/go"; PIDS=""
  for i in 1 2 3 4; do
    ( until [ -f "$STATE/go" ]; do sleep 0.01; done
      exec "$RB" prs --repo "$W" drain --max 1 ) > "$STATE/out.$r.$i" 2>&1 &
    PIDS="$PIDS $!"
  done
  sleep 0.5; touch "$STATE/go"
  merged=0; i=0
  for p in $PIDS; do
    i=$((i + 1)); rc=0; wait "$p" || rc=$?
    out="$(cat "$STATE/out.$r.$i")"
    case "$rc" in
      0) case "$out" in *"merged #$r"*) merged=$((merged + 1)) ;; esac ;;
      12) case "$out" in
            *"another integration executor holds"*) refused=$((refused + 1)) ;;
            *) echo "    round $r: executor $i failed (exit 12) without naming the lease:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;;
          esac ;;
      *) echo "    round $r: executor $i exited $rc:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1 ;;
    esac
  done
  PIDS=""
  [ "$merged" = 1 ] || { echo "    round $r: $merged executors say they merged #$r, not 1"; tail -n 3 "$STATE"/out."$r".*; exit 1; }
  [ "$(grep -c "^pr merge $r " "$STATE/log")" = 1 ] || { echo "    round $r: the forge was asked to merge #$r $(grep -c "^pr merge $r " "$STATE/log") times"; exit 1; }
  after="$(git -C "$ORIGIN" rev-list --count --merges master)"
  [ "$after" = $((before + 1)) ] || { echo "    round $r: master gained $((after - before)) merge commits, not 1"; exit 1; }
  [ ! -e "$LOCK" ] || { echo "    round $r: the lease was left behind: $(cat "$LOCK")"; exit 1; }
done

[ "$refused" -gt 0 ] || { echo "    no executor was ever refused: the processes never overlapped, so nothing raced"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    3 rounds of 4 racing executors: one merge each, $refused refused by the lease"
