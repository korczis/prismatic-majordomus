# majordomus-covers: none
# majordomus-timeout: 600
# `prs drain --continuous` runs alone, refreshes before every action, and stops cleanly:
#
#   1. it refuses a dry run and an interval outside 30–900 s before anything happens
#   2. running, it merges the ready pull request in its first cycle and then waits
#   3. while it runs, a second executor is refused by the base branch's lease — and an
#      observer is not: `prs status` still answers
#   4. SIGTERM lets the step in progress finish: it exits 0, says it was asked to stop,
#      merged exactly once, and the lease file is gone
#   5. a drain killed outright (SIGKILL) leaves its record behind, and the next executor
#      takes the lease at once: the kernel released the lock with the process
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-741.git"; W="$T/../work-741"; STATE="$T/../forge-741"; BIN="$T/../bin-741"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
PID=""; trap '[ -n "$PID" ] && kill -9 "$PID" 2>/dev/null' EXIT
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
gitq checkout -qb feature/1; echo one > one.txt; gitq add one.txt; gitq commit -qm one
gitq push -q origin HEAD:refs/heads/feature/1 HEAD:refs/pull/1/head
H1="$(git rev-parse feature/1)"
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

printf '[{"number":1,"title":"change 1","author":{"login":"someone"},"headRefName":"feature/1","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}]\n' "$H1" > "$STATE/prs.json"
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "pr list") if [ -f "$STATE/merged-1" ]; then echo '[]'; else cat "$STATE/prs.json"; fi ;;
  "pr merge")
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/1/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #1" &&
      git push -q origin HEAD:master && touch "$STATE/merged-1" ;;
  "pr view") if [ -f "$STATE/merged-1" ]; then echo MERGED; else echo OPEN; fi ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
prs refresh >/dev/null || { echo "    refresh failed"; cat "$STATE/log"; exit 1; }
LOCK="$(git rev-parse --path-format=absolute --git-common-dir)/majordomus/locks/integration-master.lock"

# ---------------------------------------------------------------- 1. refused before acting
expect_exit 2 "$RB" prs --repo "$W" drain --continuous --dry-run
expect_exit 2 "$RB" prs --repo "$W" drain --continuous --interval 5
grep -q '^pr merge' "$STATE/log" && { echo "    a refused invocation merged"; exit 1; }

# ---------------------------------------------------------------- 2. running
# the executable itself in the background, not the `prs` function: `$!` of a function is the
# subshell running it, and a signal to that is a signal to bash, not to the drain
"$RB" prs --repo "$W" drain --continuous --interval 30 > "$STATE/run.out" 2> "$STATE/run.err" & PID=$!
i=0
until grep -q '^cycle 1:' "$STATE/run.out" 2>/dev/null; do
  kill -0 "$PID" 2>/dev/null || { echo "    the continuous drain exited early:"; cat "$STATE/run.out" "$STATE/run.err"; exit 1; }
  i=$((i + 1)); [ "$i" -lt 600 ] || { echo "    no cycle ended within 60 s"; cat "$STATE/run.out"; exit 1; }
  sleep 0.1
done
grep -q 'merged #1' "$STATE/run.out" || { echo "    the first cycle did not merge #1:"; cat "$STATE/run.out"; exit 1; }
[ -f "$LOCK" ] || { echo "    no lease is held while the drain runs ($LOCK)"; exit 1; }

# ---------------------------------------------------------------- 3. alone, and observable
rc=0; out="$(prs drain --max 1 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    a second executor was not refused (exit $rc): $out"; exit 1; }
case "$out" in *"another integration executor holds"*) ;; *) echo "    the refusal does not name the lease: $out"; exit 1 ;; esac
rc=0; prs status >/dev/null 2>&1 || rc=$?
case "$rc" in 0|10) ;; *) echo "    an observer was blocked by the lease (exit $rc)"; exit 1 ;; esac

# ---------------------------------------------------------------- 4. a clean stop
kill -TERM "$PID"
i=0
while kill -0 "$PID" 2>/dev/null; do
  i=$((i + 1)); [ "$i" -lt 150 ] || { echo "    the drain did not stop within 15 s of SIGTERM"; exit 1; }
  sleep 0.1
done
rc=0; wait "$PID" || rc=$?; PID=""
[ "$rc" = 0 ] || { echo "    the stopped drain exited $rc:"; cat "$STATE/run.out" "$STATE/run.err"; exit 1; }
grep -q 'stopped after 1 cycle(s), 1 merge(s): asked to stop' "$STATE/run.out" \
  || { echo "    the drain does not say why it stopped:"; tail -3 "$STATE/run.out"; exit 1; }
[ ! -e "$LOCK" ] || { echo "    the lease was left behind: $(cat "$LOCK")"; exit 1; }
[ "$(grep -c '^pr merge' "$STATE/log")" = 1 ] || { echo "    merged more than once"; exit 1; }

# ---------------------------------------------------------------- 5. a crash
"$RB" prs --repo "$W" drain --continuous --interval 30 > "$STATE/crash.out" 2> "$STATE/crash.err" & PID=$!
i=0
until grep -q '^cycle 1:' "$STATE/crash.out" 2>/dev/null; do
  kill -0 "$PID" 2>/dev/null || { echo "    the second continuous drain exited early:"; cat "$STATE/crash.out" "$STATE/crash.err"; exit 1; }
  i=$((i + 1)); [ "$i" -lt 600 ] || { echo "    no cycle ended within 60 s"; exit 1; }
  sleep 0.1
done
kill -9 "$PID"; wait "$PID" 2>/dev/null || :; PID=""
[ -f "$LOCK" ] || { echo "    a killed drain left no record: the case cannot tell a takeover from a free lease"; exit 1; }
rc=0; out="$(prs drain --max 1 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    the lease of a killed drain was not taken at once (exit $rc): $out"; exit 1; }
[ ! -e "$LOCK" ] || { echo "    the executor that took over left the lease behind"; exit 1; }
[ "$(grep -c '^pr merge' "$STATE/log")" = 1 ] || { echo "    merged more than once"; exit 1; }
exit 0
