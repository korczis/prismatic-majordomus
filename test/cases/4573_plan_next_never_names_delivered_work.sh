# majordomus-covers: plan
# plan.next never names an issue whose delivery is merged or released, and plan.roadmap names
# that issue with its drift (I3754).
#
# On 2026-10-20 `plan next` answered I0702, whose ADR had merged and been released, because
# the ranking read the stamps alone. The next issue is now one function: the stamp ranking,
# each candidate set against its delivery at the trunk head (history and release records
# only), an issue the history shows merged, released or deployed passed over and listed.
# This drives every door to that answer over one fixture:
#
#   I0001  READY, ranks first (I0003 waits on it); its branch, then its merge
#   I0002  READY, the next-ranked one
#   I0003  BLOCKED on I0001
#   I0004  READY, ranked after I0002
#
# Each assertion names what it holds; the evidence tokens of I3754 are the two marked.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
fail() { printf '    %s\n' "$*"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4573.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# One MCP tool call over the real pipes, structured content on stdout (case 99's reader).
plan_tool() { # <tool> <arguments-json>
  local out="$S/frames.$$"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4573","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | "$RB" mcp 2>"$S/mcp.err" > "$out" || { sed 's/^/    | /' "$S/mcp.err"; fail "the server failed on $1"; }
  tail -n 1 "$out" | jq -e '.result.isError == false' >/dev/null \
    || fail "$1 returned an error frame: $(tail -n 1 "$out")"
  tail -n 1 "$out" | jq '.result.structuredContent'
}
next_json() { "$MJ" --json plan next; }
next_id() { next_json | jq -r '.issue.id // empty'; }

"$MJ" init >/dev/null
pj_init; pj_milestone M1
pj_issue I0001 M1; pj_issue I0002 M1; pj_issue I0003 M1 I0001; pj_issue I0004 M1
git add -A >/dev/null && git commit -qm "plan: four issues"
TRUNK="$(git rev-parse --abbrev-ref HEAD)"

# ---------------------------------------------------------------- nothing delivered
# Negative: with no issue delivered the answer is exactly the stamp ranking's, and the reading
# names the trunk commit it read.
next_json > "$S/n0.json"
jq -e --arg t "$TRUNK" --arg c "$(git rev-parse HEAD)" '
  .issue.id == "I0001" and .passed_over == []
  and .delivery.state == "read" and .delivery.trunk == $t and .delivery.commit == $c' \
  "$S/n0.json" >/dev/null || fail "with nothing delivered the pick is not the stamps': $(cat "$S/n0.json")"

# ---------------------------------------------------------------- a branch is work in flight
# Evidence plan_next_hands_out_branch_only_work. Negative: a READY issue named only by a
# branch, or by an open pull request in the forge observation, is still handed out.
git checkout -q -b feature/I0001-the-work
printf 'work\n' > work.txt && git add work.txt && git commit -qm "feat: the work (I0001)"
git checkout -q "$TRUNK"
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<'J'
{"schema":4,"repository":"someone/fixture","base":"master","base_sha":"0000000000000000000000000000000000000000",
 "observed_at":"2026-11-09T08:00:00Z","required_checks":null,"review_policy":null,"merge_methods":["merge"],
 "pull_requests":[{"number":5,"title":"feat: the work (I0001)","author":"someone","head_ref":"feature/I0001-the-work",
  "head_sha":"1111111111111111111111111111111111111111","base_ref":"master","draft":false,
  "created_at":"2026-11-09T07:00:00Z","updated_at":"2026-11-09T07:30:00Z"}]}
J
[ "$(next_id)" = I0001 ] || fail "a branch and an open pull request passed I0001 over: $(next_json)"
# and a commit that names it while changing only its record planned it, and delivered nothing
printf '# planned\n' >> .ai/repo/project/issues/I0001.yaml
git add -A >/dev/null && git commit -qm "chore(plan): plan I0001"
[ "$(next_id)" = I0001 ] || fail "a planning commit passed I0001 over: $(next_json)"

# ---------------------------------------------------------------- a merge passes it over
# Evidence plan_next_passes_over_delivered_work: the highest-ranked READY issue named by a
# merge on the trunk is passed over with stage merged and its merge commit, and the
# next-ranked READY issue is handed out.
#
# One server is asked before and after the merge, which lands between its two answers: the
# writer of its stdin waits for the first answer, merges, then asks again, so the same
# session must change its answer without a restart. The server runs in the foreground and
# ends with its stdin, so nothing it started outlives the case.
answered() { # <request id>: wait until the session has written its answer
  local n=0
  until grep -q "\"id\":$1[,}]" "$S/session" 2>/dev/null; do
    n=$((n + 1)); [ "$n" -lt 600 ] || return 1
    sleep 0.1
  done
}
ask_next() { # <request id>
  printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"majordomus_plan_next","arguments":{}}}\n' "$1"
}
: > "$S/session"
{
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4573","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  ask_next 2
  answered 2 || exit 0
  git merge -q --no-ff feature/I0001-the-work -m "Merge pull request #5 from someone/feature/I0001-the-work" \
    -m "feat: the work (I0001)" >&2
  ask_next 3
  answered 3 || exit 0
} | "$RB" mcp > "$S/session" 2> "$S/session.err"
answer() { grep "\"id\":$1[,}]" "$S/session" | tail -n 1 | jq '.result.structuredContent'; }
grep -q '"id":2[,}]' "$S/session" || { sed 's/^/    | /' "$S/session.err"; fail "the session never answered its first question"; }
[ "$(answer 2 | jq -r '.issue.id')" = I0001 ] || fail "the session's first answer is not I0001: $(answer 2)"
grep -q '"id":3[,}]' "$S/session" || { sed 's/^/    | /' "$S/session.err"; fail "the session never answered after the merge"; }
[ "$(answer 3 | jq -r '.issue.id')" = I0002 ] \
  || fail "the running session still names what the merge delivered: $(answer 3)"

MERGE="$(git rev-parse HEAD)"
[ "$(git rev-list --parents -n 1 HEAD | wc -w | tr -d ' ')" = 3 ] || fail "the trunk's head is not the merge"
next_json > "$S/n1.json"
jq -e --arg m "$MERGE" '
  .issue.id == "I0002"
  and (.passed_over | length) == 1
  and .passed_over[0].issue == "I0001" and .passed_over[0].stage == "merged"
  and .passed_over[0].status == "READY" and .passed_over[0].commit == $m
  and (.passed_over[0].remedy | contains("plan start I0001"))
  and .delivery.commit == $m' "$S/n1.json" >/dev/null \
  || fail "the merged I0001 was not passed over for I0002: $(cat "$S/n1.json")"

# every door names the same issue for the same trunk commit
[ "$("$RB" plan next --format json | jq -r '.issue.id')" = I0002 ] || fail "the executable's plan next disagrees"
[ "$(plan_tool majordomus_plan_next '{}' | jq -r '.issue.id')" = I0002 ] || fail "majordomus_plan_next disagrees"
[ "$(plan_tool majordomus_plan_status '{}' | jq -r '.next_ready.id')" = I0002 ] || fail "plan.status's next_ready disagrees"
"$MJ" --json plan status | grep -q '"next_ready":"I0002"' || fail "the shell's plan status disagrees: $("$MJ" --json plan status)"
"$MJ" plan status | grep -q '^next ready issue: I0002 ' || fail "the shell's plan status line disagrees: $("$MJ" plan status)"
"$MJ" plan next > "$S/text"
[ "$(grep -oE 'I[0-9]+' "$S/text" | head -n 1)" = I0002 ] || fail "plan next does not open with I0002: $(cat "$S/text")"
grep -q 'majordomus plan start I0002' "$S/text" || fail "plan next does not say how to start I0002"
grep -qE '^  I0001 +merged ' "$S/text" || fail "plan next does not list I0001 as passed over: $(cat "$S/text")"

# ---------------------------------------------------------------- the roadmap names the drift
plan_tool majordomus_plan_roadmap '{}' > "$S/roadmap.json"
jq -e --arg m "$MERGE" '
  .delivered == [{"milestone": "M1", "issues": [{"issue": "I0001", "status": "READY",
    "stage": "merged", "commit": $m, "remedy": .delivered[0].issues[0].remedy}]}]
  and .delivery.state == "read"' "$S/roadmap.json" >/dev/null \
  || fail "plan.roadmap does not list I0001 under M1: $(jq '{delivered, delivery}' "$S/roadmap.json")"

# ---------------------------------------------------------------- a transition answers the same
# The line `plan start` prints, and plan.transition's next_ready, are the same choice: after
# I0002 is taken the stamps alone would name I0001; the trunk says I0004. (A release of the
# merge passes it over as `released`; the unit test plan_next_passes_over_delivered_work
# holds that, so that this case writes no release record.)
"$MJ" plan start I0002 > "$S/start" || fail "plan start I0002 failed: $(cat "$S/start")"
grep -q '^next ready issue: I0004$' "$S/start" || fail "plan start names another next issue: $(cat "$S/start")"
git add -A >/dev/null && git commit -qm "plan: I0002 active"
plan_tool majordomus_plan_transition '{"issue":"I0002","transition":"verify"}' > "$S/moved"
jq -e '.to == "VERIFY" and .next_ready == "I0004"' "$S/moved" >/dev/null \
  || fail "plan.transition names another next issue: $(cat "$S/moved")"

# ---------------------------------------------------------------- an unread history says so
# A clone whose trunk names no commit cannot read delivery: the answer says so and is the
# stamp ranking's.
mkdir "$S/fresh" && cd "$S/fresh" && git init -q
"$MJ" init >/dev/null
pj_init; pj_milestone M1; pj_issue I0001 M1; pj_issue I0002 M1
git add -A >/dev/null
next_json > "$S/unread.json"
jq -e '.issue.id == "I0001" and .passed_over == [] and .delivery.state == "unread"
  and (.delivery.detail | length) > 0' "$S/unread.json" >/dev/null \
  || fail "an unreadable history was not named unread: $(cat "$S/unread.json")"
"$MJ" plan next | grep -q '^delivery unread: ' || fail "plan next does not say delivery was unread"
exit 0
