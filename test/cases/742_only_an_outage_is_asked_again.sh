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
#      observation, and the drain says the forge refused it. An outage ends the drain: the
#      next one, or the next cycle, decides again (ADR 0101 §10)
#   4. an outage that does not end is given up on after four attempts, and says so
#   5. a reply cut off on its way (`gh` says "unexpected end of JSON input") is an outage, not
#      an answer: the list is asked again and the observation is recorded (I4222)
#   6. a list the forge's gateway times out (HTTP 504, master's pages runs 40796031540,
#      40803295553 and 40823254353) is not the same page asked four times: it is read again in
#      pages of 25 through GraphQL, and the observation records the pull request with its
#      check (I4380)
#   7. GraphQL's own timeout, an HTTP 200 whose error says "This may be the result of a
#      timeout", is the same answer as the gateway's 504: one page of 25 is asked, never the
#      same hundred four times (I4380)
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

# the same pull request as one page of the paged open list answers it, in GraphQL's shape
printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[{"number":1,"title":"change 1","author":{"login":"someone"},"headRefName":"feature/1","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":{"nodes":[]},"createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z","body":"","statusCheckRollup":{"contexts":{"nodes":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}]}},"reviewDecision":null,"latestReviews":{"nodes":[]},"reviewRequests":{"nodes":[]},"autoMergeRequest":null,"isCrossRepository":false}]}}}}\n' "$H1" > "$STATE/open-page.json"

# The scripted forge. $STATE/mode says how `repo view` and the landing behave:
#   flaky   — `repo view` fails with a 502 once, then answers
#   denied  — `repo view` is refused with a 401, always
#   down    — `repo view` fails with a 503, always
#   mergebad — everything answers, the fast-forward of master (I3910) fails with a 502
#   gateway — `pr list` times out at the gateway (504), always; the paged read answers
#   graphql — `pr list` times out inside GraphQL (its error, not a 504), always; the paged
#             read answers
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
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
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    if [ "\$mode" = cut ] && [ ! -f "$STATE/cut" ]; then touch "$STATE/cut"; echo "unexpected end of JSON input" >&2; exit 1; fi
    # the stand-in's own declarations read lists through here too, and is not the forge's list
    if [ "\$mode" = gateway ] && [ -z "\${DECLARATIONS:-}" ]; then echo "HTTP 504: 504 Gateway Timeout (https://api.github.com/graphql)" >&2; exit 1; fi
    if [ "\$mode" = graphql ] && [ -z "\${DECLARATIONS:-}" ]; then echo "GraphQL: Something went wrong while executing your query. This may be the result of a timeout, or it could be a GitHub bug. Please include C0DE:1234:5678:9ABC:DEF0 when reporting this issue." >&2; exit 1; fi
    cat "$STATE/prs.json" ;;
  "pr merge"|"api -X") echo "HTTP 502: Bad Gateway" >&2; exit 1 ;;
  "pr view") echo OPEN ;;
  # the declarations read (ADR 0101 §6, D4): every open pull request is an owner's, a branch of
  # this repository, and no pull request mentions it. The list is this forge's own, unlogged
  # the paged open list (I4380): the one query that asks for review requests
  "api graphql")
    case "\$*" in *"reviewRequests(first:100)"*) cat "$STATE/open-page.json"; exit 0 ;; esac
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
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
EV="$W/.git/majordomus/integration/events.jsonl"; rm -f "$EV"
out="$(prs drain --max 1)" || true
requests="$(asked 'api -X PATCH')"
decisions="$(grep -c '"action":"merge_attempted"' "$EV" 2>/dev/null || true)"
[ "$requests" -ge 1 ] || { echo "    no merge was requested"; exit 1; }
[ "$requests" = "$decisions" ] \
  || { echo "    $requests merge request(s) for $decisions decision(s): a failed merge was asked again"; exit 1; }
[ "$requests" = 1 ] || { echo "    the drain asked the forge to merge $requests times during an outage, not once"; exit 1; }
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

# ---------------------------------------------------------------- 5. a reply cut off on its way
echo cut > "$STATE/mode"; : > "$STATE/log"; rm -f "$STATE/cut" "$W/.ai/local/state/integration/observation.json"
prs refresh >/dev/null || { echo "    a reply cut off once failed the refresh"; cat "$STATE/log"; exit 1; }
[ "$(asked 'pr list')" = 2 ] || { echo "    the cut-off list was not asked again exactly once: $(asked 'pr list') call(s)"; exit 1; }
[ -f "$W/.ai/local/state/integration/observation.json" ] || { echo "    no observation was recorded after the cut-off reply"; exit 1; }
# ---------------------------------------------------------------- 6 and 7. a list the forge times out
OBS="$W/.ai/local/state/integration/observation.json"
for mode in gateway graphql; do
  echo "$mode" > "$STATE/mode"; : > "$STATE/log"; rm -f "$OBS"
  prs refresh >/dev/null || { echo "    a list the forge timed out ($mode) failed the refresh"; cat "$STATE/log"; exit 1; }
  [ "$(asked 'pr list')" = 1 ] || { echo "    the timed-out page ($mode) was asked $(asked 'pr list') times, not once"; exit 1; }
  paged="$(grep -c 'reviewRequests(first:100).* n=25' "$STATE/log" || true)"
  [ "$paged" = 1 ] || { echo "    the open list ($mode) was not read in one page of 25: $paged paged read(s)"; cat "$STATE/log"; exit 1; }
  [ -f "$OBS" ] || { echo "    no observation was recorded after the paged read ($mode)"; exit 1; }
  jq -e --arg h "$H1" '[.pull_requests[] | select(.number == 1 and .head_sha == $h)
    | .checks[] | select(.name == "ci" and .state == "passed")] | length == 1' "$OBS" >/dev/null \
    || { echo "    the paged read ($mode) did not record #1 with its passing check"; jq '.pull_requests' "$OBS"; exit 1; }
done
exit 0
