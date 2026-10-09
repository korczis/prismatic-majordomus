# majordomus-covers: none
# The integration executor's non-mutating cycle moves nothing, and `majordomus prs
# prove-dry-run` proves it (WP29 of the PR-integration mission). Against a scripted forge and
# a local bare origin, with no network:
#
#   1. refresh, plan, drain --dry-run and cleanup (listing only) over a ready pull request and
#      a redundant one: the remote refs, the open pull requests, the audit trail, the lease
#      and the local refs are byte-identical before and after; the fetched mirrors equal what
#      origin serves; nothing reached the forge but reads; the classification is printed
#   2. a forge that moves while the cycle runs is caught: exit 10, naming what moved
#   3. the proof accepts no flag that could change anything: a usage error, exit 2
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-852.git"; W="$T/../work-852"; STATE="$T/../forge-852"; BIN="$T/../bin-852"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
# #2 is already in master: merging it changes no file, so it is redundant
gitq checkout -qb feature/2; echo two > two.txt; gitq add two.txt; gitq commit -qm two
gitq push -q origin HEAD:refs/heads/feature/2 HEAD:refs/pull/2/head
gitq checkout -q master; gitq merge -q --no-ff feature/2 -m "two, landed"; gitq push -q origin HEAD:master
# #1 branches from the current master and is ready
gitq checkout -qb feature/1; echo one > one.txt; gitq add one.txt; gitq commit -qm one
gitq push -q origin HEAD:refs/heads/feature/1 HEAD:refs/pull/1/head
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

pr() {   # <number> <head sha> <branch> <created>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"%s","updatedAt":"%s","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$3" "$2" "$4" "$4"
}
printf '[%s,%s]\n' "$(pr 1 "$H1" feature/1 2026-09-01T00:00:00Z)" "$(pr 2 "$H2" feature/2 2026-09-02T00:00:00Z)" > "$STATE/prs.json"

# The forge answers the reads from files and refuses every write: a merge, a close, an edit
# or anything else unexpected is logged and fails. INTRUDE=1 makes the first pull-request
# listing move origin, which is what a forge changing under the proof looks like.
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    if [ "\${INTRUDE:-0}" = 1 ] && [ ! -e "$STATE/intruded" ]; then
      touch "$STATE/intruded"
      git -C "$ORIGIN" update-ref refs/heads/intruder "\$(git -C "$ORIGIN" rev-parse master)"
    fi
    cat "$STATE/prs.json" ;;
  # the declarations read (ADR 0101 §6, D4): every open pull request is an owner's, a branch of
  # this repository, and no pull request mentions it. The list is this forge's own, unlogged
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  *) echo "WRITE-OR-UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
proof() { "$RB" prs prove-dry-run --repo "$W" "$@"; }

# ---------------------------------------------------------------- 1. nothing moves
: > "$STATE/log"
rc=0; out="$(proof 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    the proof failed on a quiet forge (exit $rc):"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
case "$out" in *"nothing moved"*) ;; *) echo "    the proof did not say nothing moved: $out"; exit 1 ;; esac
grep -q "WRITE-OR-UNEXPECTED" "$STATE/log" && { echo "    the cycle asked the forge for a write:"; cat "$STATE/log"; exit 1; }
grep -Eq "^pr (merge|close|edit)" "$STATE/log" && { echo "    the cycle merged, closed or edited"; exit 1; }
[ ! -e "$W/.ai/local/state/integration/events.jsonl" ] || { echo "    the cycle wrote the audit trail"; exit 1; }
# the same proof as a document: complete, equal snapshots, and the cycle really ran
p="$(proof --format json)" || { echo "    the JSON proof failed"; exit 1; }
[ "$(printf '%s' "$p" | jq -r '.ok')" = true ] || { echo "    the JSON proof is not ok: $p"; exit 1; }
[ "$(printf '%s' "$p" | jq -c '[.before.sections[].name]')" = '["remote","forge","trail","lease","local"]' ] \
  || { echo "    the snapshot does not have its five sections"; exit 1; }
[ "$(printf '%s' "$p" | jq -c '.before == .after')" = true ] || { echo "    the snapshots differ"; exit 1; }
printf '%s' "$p" | jq -e '.before.sections[] | select(.name == "remote") | .lines | any(test("refs/pull/1/head"))' >/dev/null \
  || { echo "    the remote snapshot does not list the pull refs"; exit 1; }
printf '%s' "$p" | jq -e '.before.sections[] | select(.name == "forge") | .lines | any(startswith("#2 "))' >/dev/null \
  || { echo "    the forge snapshot does not list #2"; exit 1; }
printf '%s' "$p" | jq -e '.steps[] | select(.step == "drain --dry-run") | .summary | test("would merge #1")' >/dev/null \
  || { echo "    the dry run did not select #1: $(printf '%s' "$p" | jq -c .steps)"; exit 1; }
printf '%s' "$p" | jq -e '.steps[] | select(.step == "cleanup") | .summary | test("#2 would_close")' >/dev/null \
  || { echo "    cleanup did not list the redundant #2: $(printf '%s' "$p" | jq -c .steps)"; exit 1; }
[ "$(printf '%s' "$p" | jq -c '.moved, .mirrors')" = "$(printf '[]\n[]')" ] || { echo "    moved or mirrors are not empty"; exit 1; }
# the refresh fetched, and the mirrors are exactly what origin serves
[ "$(git rev-parse refs/majordomus/prs/1)" = "$H1" ] || { echo "    the mirror of #1 is not its head"; exit 1; }
# the classification is printed, one line per open pull request
printf '%s\n' "$out" | grep -Eq "^  #1  ${H1:0:12}  ready" || { echo "    #1 is not printed as ready: $out"; exit 1; }
printf '%s\n' "$out" | grep -Eq "^  #2  ${H2:0:12}  redundant" || { echo "    #2 is not printed as redundant: $out"; exit 1; }

# ---------------------------------------------------------------- 2. a forge that moves is caught
rc=0; out="$(INTRUDE=1 proof 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a forge that moved during the cycle exited $rc, not 10: $out"; exit 1; }
printf '%s\n' "$out" | grep -Eq "^  \+ remote +[0-9a-f]+ refs/heads/intruder$" || { echo "    the moved ref is not named: $out"; exit 1; }
git -C "$ORIGIN" update-ref -d refs/heads/intruder

# ---------------------------------------------------------------- 3. no flag can mutate
for flag in --apply --execute --max; do
  rc=0; out="$(proof "$flag" 2>&1)" || rc=$?
  [ "$rc" = 2 ] || { echo "    $flag was not refused as a usage error (exit $rc): $out"; exit 1; }
done
grep -Eq "^pr (merge|close|edit)" "$STATE/log" && { echo "    a refused flag reached the forge"; exit 1; }
exit 0
