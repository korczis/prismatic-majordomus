# majordomus-covers: none
# majordomus-timeout: 600
# The forge is asked again only when it had an outage, a bounded number of times, and the
# merge is never asked again — through the real command line against a scripted `gh`:
#
#   1. a 502 on the first question is retried after a wait, and the observation is recorded
#   2. a 401 is the answer: one question, no wait, exit 12
#   3. a merge that failed with a 502 is not retried — a merge that timed out may have
#      landed, and the verification after it is what finds out — so every merge request
#      that reaches the forge is a decision of its own, recorded and taken from a fresh
#      observation, and the drain says the forge refused it (the drain may decide again,
#      within its step bound: that is a new decision, not the old request asked twice)
#   4. an outage that does not end is given up on after four attempts, and says so
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-742.git"; W="$T/../work-742"; STATE="$T/../forge-742"; BIN="$T/../bin-742"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
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

# The scripted forge. $STATE/mode says how `repo view` and `pr merge` behave:
#   flaky   — `repo view` fails with a 502 once, then answers
#   denied  — `repo view` is refused with a 401, always
#   down    — `repo view` fails with a 503, always
#   mergebad — everything answers, `pr merge` fails with a 502
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
mode="\$(cat "$STATE/mode")"
case "\$1 \$2" in
  "repo view")
    case "\$mode" in
      flaky) if [ ! -f "$STATE/flaked" ]; then touch "$STATE/flaked"; echo "HTTP 502: Bad Gateway (https://api.github.com/graphql)" >&2; exit 1; fi ;;
      denied) echo "HTTP 401: Bad credentials (https://api.github.com/graphql)" >&2; exit 1 ;;
      down) echo "HTTP 503: Service Unavailable" >&2; exit 1 ;;
    esac
    echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "pr list") cat "$STATE/prs.json" ;;
  "pr merge") echo "HTTP 502: Bad Gateway" >&2; exit 1 ;;
  "pr view") echo OPEN ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
asked() { grep -c "^$1" "$STATE/log" 2>/dev/null || true; }
now() { date +%s; }

# ---------------------------------------------------------------- 1. an outage that ends
echo flaky > "$STATE/mode"; : > "$STATE/log"
t0="$(now)"
prs refresh >/dev/null || { echo "    a single 502 failed the refresh"; cat "$STATE/log"; exit 1; }
[ "$(asked 'repo view')" = 2 ] || { echo "    the 502 was not asked again exactly once: $(asked 'repo view') call(s)"; exit 1; }
[ "$(( $(now) - t0 ))" -ge 2 ] || { echo "    the retry did not wait"; exit 1; }
[ -f "$W/.ai/local/state/integration/observation.json" ] || { echo "    no observation was recorded"; exit 1; }

# ---------------------------------------------------------------- 2. a refusal is the answer
echo denied > "$STATE/mode"; : > "$STATE/log"
rc=0; out="$(prs refresh 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    a 401 exited $rc"; exit 1; }
[ "$(asked 'repo view')" = 1 ] || { echo "    a 401 was asked again: $(asked 'repo view') call(s)"; exit 1; }
case "$out" in *"Bad credentials"*) ;; *) echo "    the refusal is not passed on: $out"; exit 1 ;; esac
case "$out" in *"attempts"*) echo "    a refusal claims to have been retried: $out"; exit 1 ;; esac

# ---------------------------------------------------------------- 3. the merge is not retried
echo mergebad > "$STATE/mode"; : > "$STATE/log"
EV="$W/.ai/local/state/integration/events.jsonl"; rm -f "$EV"
out="$(prs drain --max 1)" || true
requests="$(asked 'pr merge')"
decisions="$(grep -c '"action":"merge_attempted"' "$EV" 2>/dev/null || true)"
[ "$requests" -ge 1 ] || { echo "    no merge was requested"; exit 1; }
[ "$requests" = "$decisions" ] \
  || { echo "    $requests merge request(s) for $decisions decision(s): a failed merge was asked again"; exit 1; }
[ "$requests" -le 3 ] || { echo "    the drain went past its step bound: $requests merges"; exit 1; }
# each decision was taken from its own two observations, not from a retry loop
[ "$(asked 'pr list')" -ge $((2 * requests)) ] || { echo "    a merge was requested without a fresh observation"; exit 1; }
case "$out" in *"the forge refused the merge"*) ;; *) echo "    the drain does not report the refusal: $out"; exit 1 ;; esac
grep -q -- '--admin' "$STATE/log" && { echo "    --admin reached the forge"; exit 1; }

# ---------------------------------------------------------------- 4. an outage that does not end
echo down > "$STATE/mode"; : > "$STATE/log"
rc=0; out="$(prs refresh 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    an outage exited $rc"; exit 1; }
[ "$(asked 'repo view')" = 4 ] || { echo "    the outage was asked $(asked 'repo view') times, not 4"; exit 1; }
case "$out" in *"(after 4 attempts)"*) ;; *) echo "    the failure does not say it was retried: $out"; exit 1 ;; esac
exit 0
