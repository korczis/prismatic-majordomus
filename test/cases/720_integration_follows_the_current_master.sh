# majordomus-covers: none
# Proves rule project.integration-follows-the-current-master (ADR 0101) through the real
# command line, with no network: `majordomus prs` against a scripted forge.
#
# The forge is a `gh` on PATH that answers the handful of questions the adapter asks from
# files this case writes, performs a merge into a local bare "origin" when asked, and logs
# every call. The origin serves `refs/pull/<n>/head` the way GitHub does. So the whole cycle
# runs — observe, classify, rank, dry-run, drain one merge, verify, re-observe — and the log
# is the evidence of what reached the forge and what did not.
#
#   1. nothing observed yet: status says so (exit 10) and asks for a refresh
#   2. refresh observes; status, plan and explain answer offline (the log does not grow)
#   3. #1 contains master and passed ci: ready; #2 is behind master: needs_refresh
#   4. a dry run says it would merge #1 and changes nothing, records nothing
#   5. drain --max 1 merges exactly #1, with --match-head-commit and never --admin, verifies
#      that master contains its head, and records the merge
#   6. after the merge the old observation is stale and says so; a refresh re-plans
#   7. the source never names --admin or a force push, and never reads the forge's mergeable
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-720.git"; W="$T/../work-720"; STATE="$T/../forge-720"; BIN="$T/../bin-720"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
# #2 branches from the first master, then master moves: #2 is behind
gitq checkout -qb feature/2; echo two > two.txt; gitq add two.txt; gitq commit -qm two
gitq push -q origin HEAD:refs/heads/feature/2 HEAD:refs/pull/2/head
gitq checkout -q master; echo more >> a.txt; gitq commit -qam more; gitq push -q origin HEAD:master
# #1 branches from the current master: it contains master
gitq checkout -qb feature/1; echo one > one.txt; gitq add one.txt; gitq commit -qm one
gitq push -q origin HEAD:refs/heads/feature/1 HEAD:refs/pull/1/head
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"
gitq checkout -q master
# the registry serves a supervised repository: the clone gets its layer, before the scripted
# forge is on the PATH (init has nothing to ask it)
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed in the scratch clone:"; tail -5 "$STATE/init.log"; exit 1; }

pr() {   # <number> <head sha> <branch> <created>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"%s","updatedAt":"%s","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$3" "$2" "$4" "$4"
}
printf '[%s,%s]\n' "$(pr 1 "$H1" feature/1 2026-09-01T00:00:00Z)" "$(pr 2 "$H2" feature/2 2026-09-02T00:00:00Z)" > "$STATE/prs.json"

cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case " \$* " in *" --admin "*) echo "ADMIN" >> "$STATE/log"; exit 1 ;; esac
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  # the rulesets that apply to master: none, as a branch with only a protection answers
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    if [ -f "$STATE/merged-1" ]; then sed 's/^\[.*},{/[{/' "$STATE/prs.json"; else cat "$STATE/prs.json"; fi ;;
  "pr merge")
    n="\$3"; t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$n" ;;
  "pr view")
    if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
calls() { wc -l < "$STATE/log" 2>/dev/null | tr -d ' ' || echo 0; }
: > "$STATE/log"

# ---------------------------------------------------------------- 1. nothing observed
rc=0; out="$(prs status 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    status with no observation exited $rc: $out"; exit 1; }
case "$out" in *"prs refresh"*) ;; *) echo "    the refusal does not say what to run: $out"; exit 1 ;; esac

# ---------------------------------------------------------------- 2-3. observe, then read offline
prs refresh >/dev/null || { echo "    refresh failed"; cat "$STATE/log"; exit 1; }
before="$(calls)"
q="$(prs status --format json)" || { echo "    status failed after a refresh"; exit 1; }
prs plan >/dev/null; prs explain 1 >/dev/null
[ "$(calls)" = "$before" ] || { echo "    a read-only prs command reached the forge:"; tail -n +"$((before+1))" "$STATE/log"; exit 1; }
disp() { printf '%s' "$q" | jq -r --argjson n "$1" '.assessments[] | select(.number == $n) | .disposition'; }
[ "$(disp 1)" = ready ] || { echo "    #1 is $(disp 1), not ready"; printf '%s\n' "$q" | jq '.assessments[0]'; exit 1; }
[ "$(disp 2)" = needs_refresh ] || { echo "    #2 is $(disp 2), not needs_refresh"; exit 1; }
[ "$(printf '%s' "$q" | jq -r .next_merge)" = 1 ] || { echo "    the next merge is not #1"; exit 1; }
[ "$(printf '%s' "$q" | jq -r '.assessments[0].evaluated_against.head_sha')" = "$H1" ] \
  || { echo "    the first assessment does not name the head it was decided against"; exit 1; }
prs explain 1 | grep "disposition:  ready" >/dev/null || { echo "    explain does not say ready"; exit 1; }
# the wire contract: the capability HTTP and MCP serve answers the same queue, through the
# registry, and names the same next merge and dispositions as the command line
reg="$("$RB" run integration.queue --repo "$W" --format json)" || { echo "    integration.queue did not run"; exit 1; }
[ "$(printf '%s' "$reg" | jq -r '.output.observed')" = true ] || { echo "    the capability says nothing is observed"; exit 1; }
[ "$(printf '%s' "$reg" | jq -r '.output.queue.next_merge')" = 1 ] || { echo "    the capability's next merge is not #1"; exit 1; }
[ "$(printf '%s' "$reg" | jq -c '[.output.queue.assessments[] | [.number, .disposition]]')" = "$(printf '%s' "$q" | jq -c '[.assessments[] | [.number, .disposition]]')" ] \
  || { echo "    the capability and the command line disagree about the queue"; exit 1; }
exp="$("$RB" run integration.explain --repo "$W" --input '{"number":2}' --format json)" || { echo "    integration.explain did not run"; exit 1; }
[ "$(printf '%s' "$exp" | jq -r '.output.assessment.disposition')" = needs_refresh ] || { echo "    integration.explain does not say needs_refresh for #2"; exit 1; }

# ---------------------------------------------------------------- 4. a dry run
out="$(prs drain --dry-run --max 3)"
case "$out" in *"would merge #1"*) ;; *) echo "    the dry run did not say it would merge #1: $out"; exit 1 ;; esac
grep -q "^pr merge" "$STATE/log" && { echo "    the dry run merged"; exit 1; }
[ ! -e "$W/.ai/local/state/integration/events.jsonl" ] || { echo "    the dry run recorded an action"; exit 1; }

# ---------------------------------------------------------------- 5. one merge
out="$(prs drain --max 1)" || { echo "    drain failed: $out"; cat "$STATE/log"; exit 1; }
case "$out" in *"merged #1"*) ;; *) echo "    drain did not merge #1: $out"; exit 1 ;; esac
[ "$(grep -c '^pr merge' "$STATE/log")" = 1 ] || { echo "    drain --max 1 merged more than once"; exit 1; }
grep -q "^pr merge 1 --merge --match-head-commit $H1\$" "$STATE/log" \
  || { echo "    the merge was not pinned to the decided head:"; grep '^pr merge' "$STATE/log"; exit 1; }
grep -q ADMIN "$STATE/log" && { echo "    the executor passed --admin"; exit 1; }
git -C "$ORIGIN" merge-base --is-ancestor "$H1" master || { echo "    master does not contain #1's head"; exit 1; }
# the merge was decided twice: two pr list calls between the dry run and the merge
n_list="$(sed -n '/^pr list/p' "$STATE/log" | wc -l | tr -d ' ')"
[ "$n_list" -ge 4 ] || { echo "    the executor did not observe again before merging ($n_list observations)"; exit 1; }
ev="$W/.ai/local/state/integration/events.jsonl"
jq -e 'select(.action == "merge_succeeded" and .pr == 1 and .master_after != null)' "$ev" >/dev/null \
  || { echo "    the merge is not in the audit trail"; cat "$ev"; exit 1; }
prs events | grep merge_succeeded >/dev/null || { echo "    prs events does not show the merge"; exit 1; }

# ---------------------------------------------------------------- 6. the old plan is void
rc=0; prs status >/dev/null 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    status after a merge did not flag its observation as stale (exit $rc)"; exit 1; }
prs refresh >/dev/null
q="$(prs status --format json)"
[ "$(printf '%s' "$q" | jq '.tallies.open')" = 1 ] || { echo "    #1 is still in the queue after its merge"; exit 1; }
[ "$(disp 2)" = needs_refresh ] || { echo "    #2 is $(disp 2) after the merge"; exit 1; }

# ---------------------------------------------------------------- 6b. the MCP tools, as a client calls them
# The three tools the registry projects over MCP answer from the same recorded observation the
# command line renders: the queue without #1, #2's disposition, and the merge in the trail.
mreq() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  mreq 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case720","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  mreq 2 tools/call '{"name":"majordomus_pull_requests","arguments":{}}'
  mreq 3 tools/call '{"name":"majordomus_pull_request_explain","arguments":{"number":2}}'
  mreq 4 tools/call '{"name":"majordomus_integration_events","arguments":{}}'
} > "$T/mcp.in"
rc=0; (cd "$W" && "$RB" mcp < "$T/mcp.in" > "$T/mcp.out" 2> "$T/mcp.err") || rc=$?
[ "$rc" = 0 ] || { echo "    the MCP server exited $rc"; tail -5 "$T/mcp.err"; exit 1; }
frame() { jq -c --argjson id "$1" 'select(.id == $id)' "$T/mcp.out"; }
frame 2 | jq -e '.result.isError == false and .result.structuredContent.observed == true and ([.result.structuredContent.queue.assessments[].number] | index(1) | not)' >/dev/null \
  || { echo "    majordomus_pull_requests does not answer the refreshed queue:"; frame 2 | head -c 400; exit 1; }
frame 3 | jq -e '.result.isError == false and .result.structuredContent.assessment.disposition == "needs_refresh"' >/dev/null \
  || { echo "    majordomus_pull_request_explain does not say #2 needs a refresh:"; frame 3 | head -c 400; exit 1; }
frame 4 | jq -e '.result.isError == false and (.result.structuredContent | tostring | test("merge_succeeded"))' >/dev/null \
  || { echo "    majordomus_integration_events does not carry the merge:"; frame 4 | head -c 400; exit 1; }

# ---------------------------------------------------------------- 7. the source
src="$ROOT/apps/majordomus-cli/src"
# a push is only ever a fast-forward: no force flag on any line that pushes
if grep -nE '"--admin"|force-with-lease|"push"[^;]*"(--force|-f|\+)' "$src/integration/"*.rs "$src/commands/prs.rs"; then
  echo "    integration code names --admin or a force push"; exit 1
fi
grep -n '"mergeable' "$src/integration/forge.rs" && { echo "    the forge adapter reads mergeable"; exit 1; }
exit 0
