# majordomus-covers: capture session
# claims: accumulation-is-watched
# What grows on its own is watched where a worker is already listening (ADR 0150, rule
# project.accumulation-is-watched): the worktrees git registers, the local branches the trunk
# already reaches and the free space under them are measured against the budgets the policy
# declares, said by the provider's start and prompt events, and reduced — on the policy's
# consent, and only then — by the command that re-measures every candidate before it goes.
#
# What is proved, in a repository of the case's own with a remote of its own:
#   - every quantity is measured and judged against its budget; one with no budget is
#     reported as unbudgeted, and the exit code is the verdict;
#   - the start event's briefing says what is over budget and names the policy's answer, and
#     says nothing when the watch is off — by policy, or for the process tree;
#   - a crossed budget is told to the mesh as an advisory claim when the policy has it
#     announced, and taken back when the budget holds again;
#   - `act: report` removes nothing, and says which command would;
#   - `act: reduce` removes the worktrees of landed branches and deletes the names of landed
#     branches nothing has checked out — and keeps a branch with unmerged work, a branch at
#     the trunk's own tip, and the worktree of a branch that never reached a remote;
#   - what was done is recorded and the next measurement carries it; a second reduction
#     inside the cooldown does not run;
#   - a compaction event starts the reduction without anybody typing a command;
#   - the prompt event says the free disk is under its floor once, not on every prompt.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v lsof >/dev/null 2>&1 || skip "no lsof, which the reclaim refuses without"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
PATH="$(dirname "$MJ"):$PATH"; export PATH
# The harness switches the watch off for every case, so that none sweeps the machine it runs
# on. This one works in a repository of its own, and the watch is what it measures.
unset MAJORDOMUS_ACCUMULATION

S="$T/watch"
mkdir -p "$S"
cd "$S" || { echo "    could not enter the fixture"; exit 1; }
git init -q --bare origin.git
git init -q repo
cd repo || exit 1
R="$PWD"
git config user.email t@t; git config user.name t
git remote add origin ../origin.git
printf 'one\n' > file.txt
git add -A && git commit -qm trunk
git branch -M master
"$MJ" init >/dev/null && "$MJ" update >/dev/null || { echo "    the layer could not be made"; exit 1; }
# this case is about the watch; the server the start event would ensure is case 108's
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
"$MJ" capture install >/dev/null || { echo "    the hooks could not be installed"; exit 1; }
git add -A && git commit -qm layer
git push -q -u origin master

# set <key> <value>: one scalar of the policy's accumulation block
set_acc() {
  awk -v k="$1" -v v="$2" '
    /^accumulation:/ { inb = 1; print; next }
    inb && /^[^ #]/ { inb = 0 }
    inb && $1 == k ":" { printf "  %s: %s\n", k, v; done = 1; next }
    { print }
    END { if (!done) exit 3 }' .ai/repo/policy.yaml > "$T/pol" \
    && cp "$T/pol" .ai/repo/policy.yaml || { echo "    accumulation.$1 is not in the skeleton's policy"; exit 1; }
}
# every key the watch reads is one the skeleton declares
for key in watch act worktrees_max merged_branches_max disk_free_min_mb cooldown_minutes announce_on_mesh; do
  grep -qE "^  $key: " .ai/repo/policy.yaml || { echo "    the skeleton's policy does not declare accumulation.$key"; exit 1; }
done
[ "$(awk '/^accumulation:/ {b=1} b && $1 == "act:" {print $2; exit}' .ai/repo/policy.yaml)" = report ] \
  || { echo "    a new repository consents to removal without being asked"; exit 1; }
# doctor holds every policy key to the allow list; the hooks this fixture never installed are
# its other findings here and not this case's subject
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -E '^(FAIL|WARN) +policy|^(FAIL|WARN) .*accumulation' "$T/doctor.out" \
  && { echo "    doctor does not accept the accumulation block"; exit 1; }
grep -qE '^OK +policy' "$T/doctor.out" || { echo "    doctor did not judge the policy at all"; tail -5 "$T/doctor.out"; exit 1; }
echo "    the skeleton declares the block, with act: report, and doctor accepts it"

landed() {  # landed <branch> [worktree]: a branch pushed and merged into the trunk
  git switch -q -c "$1" master
  printf '%s\n' "$1" > "$1.txt"
  git add -A && git commit -qm "$1"
  git push -q -u origin "$1"
  git switch -q master
  git merge -q --no-ff -m "merge $1" "$1"
  git push -q origin master
  [ "${2:-}" != worktree ] || "$RB" worktree create "$1" >/dev/null 2>&1 \
    || { echo "    could not create the worktree of $1"; exit 1; }
}
landed landed-a worktree
landed landed-b worktree
landed name-only
# merged and never pushed: its commits are on this disk only, whatever the trunk says
git switch -q -c never-pushed master
printf 'np\n' > np.txt && git add -A && git commit -qm never-pushed
git switch -q master && git merge -q --no-ff -m "merge never-pushed" never-pushed && git push -q origin master
"$RB" worktree create never-pushed >/dev/null 2>&1 || { echo "    could not create the worktree of never-pushed"; exit 1; }
# work that has not landed
git switch -q -c open master
printf 'open\n' > open.txt && git add -A && git commit -qm open && git push -q -u origin open
git switch -q master
"$RB" worktree create open >/dev/null 2>&1 || { echo "    could not create the worktree of open"; exit 1; }
# where new work starts: a name at the trunk's own tip, with nothing on it yet
git branch at-the-tip master

acc() { "$RB" accumulation --repo "$R" --format json 2>/dev/null; }
measured() { acc | jq -r --arg q "$1" '.quantities[] | select(.id == $q) | .measured'; }
state() { acc | jq -r --arg q "$1" '.quantities[] | select(.id == $q) | .state'; }

# ---------------------------------------------------------------- measured, and judged
set_acc worktrees_max 2
set_acc merged_branches_max 1
set_acc disk_free_min_mb 0
[ "$(measured worktrees)" = 5 ] || { echo "    5 worktrees are registered and $(measured worktrees) were counted"; exit 1; }
[ "$(state worktrees)" = over ] || { echo "    5 worktrees against a budget of 2 are $(state worktrees)"; exit 1; }
# landed-a, landed-b, name-only, never-pushed and at-the-tip are reached by the trunk; open is not
[ "$(measured merged_branches)" = 5 ] || { echo "    5 branches are merged and $(measured merged_branches) were counted"; exit 1; }
[ "$(state disk_free)" = ok ] || { echo "    a floor of zero is crossed: $(state disk_free)"; exit 1; }
expect_exit 10 "$RB" accumulation --repo "$R"
expect_grep '^over +worktrees +5 worktree\(s\), budget at most 2 — majordomus worktree cleanup --remove --branches'
expect_grep 'accumulation: 2 of 3 quantities over budget; policy says report'
echo "    every quantity is measured against its budget, and the exit code is the verdict"

# The capability is projected as the MCP tool majordomus_accumulation, and a tool nothing
# sends a tools/call is indistinguishable from one that does not work
# (scripts/ci/mcp-tool-run-check). One standalone session — no port, no lease, nothing
# written — and its typed answer must agree with the command's.
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case2986","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_accumulation","arguments":{}}}\n'
} | env -u MAJORDOMUS_SHARE "$RB" mcp --standalone --repo "$R" --share "$ROOT/share" 2>"$T/mcp.err" \
  | sed -n 2p > "$T/mcp.json" || true
jq -e '.result.isError != true and .result.structuredContent.schema == "majordomus/accumulation/v1"
       and .result.structuredContent.over == 2 and .result.structuredContent.within_budget == false
       and ([.result.structuredContent.quantities[] | select(.id == "worktrees" and .measured == 5 and .state == "over")] | length) == 1' \
  "$T/mcp.json" >/dev/null 2>&1 || {
  echo "    majordomus_accumulation did not give the command's verdict over MCP:"
  head -c 600 "$T/mcp.json" | sed 's/^/      /'; echo
  sed 's/^/      | /' "$T/mcp.err" | head -5; exit 1; }
echo "    the MCP tool gives the same verdict about the same quantities"

cp .ai/repo/policy.yaml "$T/policy.keep"
grep -v '^  worktrees_max: ' "$T/policy.keep" > .ai/repo/policy.yaml
[ "$(state worktrees)" = unbudgeted ] || { echo "    a quantity with no budget is $(state worktrees)"; exit 1; }
[ "$(measured worktrees)" = 5 ] || { echo "    a quantity with no budget was not measured"; exit 1; }
cp "$T/policy.keep" .ai/repo/policy.yaml
echo "    a quantity the policy declares no budget for is measured and not judged"

# ---------------------------------------------------------------- the start event says it
out="$(printf '{"session_id":"cc-1","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'Accumulation: over budget' \
  || { echo "    the briefing does not say a budget is crossed:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -qE '^  over +worktrees ' || { echo "    the briefing does not name the quantity"; exit 1; }
printf '%s\n' "$out" | grep -qF 'the policy says report' || { echo "    the briefing does not say what the policy allows"; exit 1; }
out="$(printf '{"session_id":"cc-1","source":"resume"}' | MAJORDOMUS_ACCUMULATION=off ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'Accumulation:' && { echo "    MAJORDOMUS_ACCUMULATION=off did not switch the watch off"; exit 1; }
set_acc watch false
out="$(printf '{"session_id":"cc-1","source":"resume"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'Accumulation:' && { echo "    watch: false did not switch the watch off"; exit 1; }
set_acc watch true
set_acc worktrees_max 50; set_acc merged_branches_max 50
out="$(printf '{"session_id":"cc-1","source":"resume"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'Accumulation:' && { echo "    a briefing spoke of budgets that all hold"; exit 1; }
set_acc worktrees_max 2; set_acc merged_branches_max 1
echo "    the start event says what is over budget, and nothing when it is off or within budget"

# ---------------------------------------------------------------- the mesh hears it
# What the start event tells the mesh is proved at the seam: the executable the hooks run is
# wrapped, so that every `mesh` invocation is recorded and everything else is the real one.
# Whether a claim then travels between machines is the mesh's own subject (case 130).
W="$T/wrapped"
{ printf '#!/bin/sh\n'
  printf 'case "$1" in mesh) printf "%%s\\n" "$*" >> "%s"; exit 0 ;; esac\n' "$T/mesh.log"
  printf 'exec "%s" "$@"\n' "$RB"
} > "$W" && chmod +x "$W"
: > "$T/mesh.log"
said() {  # said <pattern>: the wrapped executable was asked for it, within ten seconds
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    grep -qE -- "$1" "$T/mesh.log" && return 0
    sleep 1
  done
  return 1
}
printf '{"session_id":"cc-1","source":"resume"}' | MAJORDOMUS_BIN="$W" ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
sleep 1
[ ! -s "$T/mesh.log" ] || { echo "    the mesh was told without accumulation.announce_on_mesh:"; cat "$T/mesh.log"; exit 1; }
set_acc announce_on_mesh true
printf '{"session_id":"cc-1","source":"resume"}' | MAJORDOMUS_BIN="$W" ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
said '^mesh claim .*--session accumulation-watch --advisory --intent accumulation over budget on .*worktrees 5 \(at most 2\); merged_branches 5 \(at most 1\)' \
  || { echo "    a crossed budget was not claimed on the mesh:"; cat "$T/mesh.log"; exit 1; }
grep -qE 'local/state/accumulation$' "$T/mesh.log" || { echo "    the claim does not name the watch's own state as its scope"; cat "$T/mesh.log"; exit 1; }
[ -f .ai/local/state/accumulation/announced ] || { echo "    the watch does not remember that it announced"; exit 1; }
set_acc worktrees_max 50; set_acc merged_branches_max 50
printf '{"session_id":"cc-1","source":"resume"}' | MAJORDOMUS_BIN="$W" ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
said '^mesh session close .*--session accumulation-watch' \
  || { echo "    the announcement was not taken back when the budgets held again:"; cat "$T/mesh.log"; exit 1; }
[ ! -e .ai/local/state/accumulation/announced ] || { echo "    the watch still believes it is announcing"; exit 1; }
set_acc worktrees_max 2; set_acc merged_branches_max 1
set_acc announce_on_mesh false
echo "    a crossed budget is an advisory claim on the mesh, and is taken back when it holds again"

# ---------------------------------------------------------------- report is not consent
expect_exit 10 "$RB" accumulation --repo "$R" --reduce
expect_grep 'accumulation.act is report: nothing is removed here'
[ -d "$("$RB" worktree path landed-a)" ] || { echo "    act: report removed a worktree"; exit 1; }
git show-ref --verify --quiet refs/heads/name-only || { echo "    act: report deleted a branch name"; exit 1; }
[ ! -e .ai/local/state/accumulation/last.json ] || { echo "    a refusal left a record of a reduction"; exit 1; }
echo "    act: report removes nothing and names the command"

# ---------------------------------------------------------------- reduce, on consent
set_acc act reduce
set_acc cooldown_minutes 30
expect_exit 0 "$RB" accumulation --repo "$R" --reduce --if-due
for b in landed-a landed-b; do
  [ ! -d "$("$RB" worktree path "$b")" ] || { echo "    the worktree of $b is still there"; printf '%s\n' "$LAST_OUT"; exit 1; }
  git show-ref --verify --quiet "refs/heads/$b" && { echo "    the name $b is still there"; printf '%s\n' "$LAST_OUT"; exit 1; }
done
git show-ref --verify --quiet refs/heads/name-only && { echo "    a landed name with no worktree was kept"; printf '%s\n' "$LAST_OUT"; exit 1; }
echo "    the worktrees of landed branches go, and the names of landed branches after them"

[ -d "$("$RB" worktree path open)" ] && git show-ref --verify --quiet refs/heads/open \
  || { echo "    work that has not landed was removed"; exit 1; }
git show-ref --verify --quiet refs/heads/at-the-tip || { echo "    a branch at the trunk's own tip was deleted"; exit 1; }
expect_grep "refused +at-the-tip .*trunk's own tip"
[ -d "$("$RB" worktree path never-pushed)" ] && git show-ref --verify --quiet refs/heads/never-pushed \
  || { echo "    a branch that reached no remote was removed"; exit 1; }
expect_grep 'refused +never-pushed .*no upstream'
git -C "$R" cat-file -e "$(git rev-parse master)^{commit}" && [ -f landed-a.txt ] && [ -f name-only.txt ] \
  || { echo "    the trunk lost what the deleted names pointed at"; exit 1; }
echo "    unlanded work, a name at the trunk's tip and a branch on one disk only are kept and named"

rec=.ai/local/state/accumulation/last.json
[ -f "$rec" ] || { echo "    the reduction left no record"; exit 1; }
[ "$(jq -r '.removed | length' "$rec")" = 2 ] && [ "$(jq -r '.deleted | length' "$rec")" = 3 ] \
  || { echo "    the record does not say what was done:"; cat "$rec"; exit 1; }
[ "$(acc | jq -r '.last_reduction.deleted | length')" = 3 ] \
  || { echo "    the next measurement does not carry the last reduction"; exit 1; }
landed landed-c worktree
expect_exit 0 "$RB" accumulation --repo "$R" --reduce --if-due
expect_grep 'not due'
[ -d "$("$RB" worktree path landed-c)" ] || { echo "    a reduction ran inside the cooldown"; exit 1; }
echo "    what was done is recorded, the next measurement carries it, and the cooldown holds"

# ---------------------------------------------------------------- an event starts it
set_acc cooldown_minutes 0
printf '{"session_id":"cc-1"}' | ./.claude/hooks/majordomus-session-compact >/dev/null 2>&1 || true
gone=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
  [ -d "$("$RB" worktree path landed-c)" ] || { gone=1; break; }
  sleep 1
done
[ "$gone" = 1 ] || { echo "    the compaction event started no reduction within 30s:"; cat .ai/local/state/accumulation/reduce.log 2>/dev/null | sed 's/^/      /'; exit 1; }
echo "    a compaction event reduces what is spare without anybody typing a command"

# ---------------------------------------------------------------- the prompt event and the disk
set_acc act report
set_acc cooldown_minutes 30
set_acc disk_free_min_mb 999999999999
out="$(printf '{"prompt":"hello","prompt_id":"w1","session_id":"cc-1"}' | ./.claude/hooks/majordomus-capture 2>/dev/null)"
printf '%s\n' "$out" | grep -qE 'accumulation watch: [0-9]+ MiB free on this volume, budget at least 999999999999' \
  || { echo "    the prompt event did not say the disk is under its floor: $out"; exit 1; }
out="$(printf '{"prompt":"again","prompt_id":"w2","session_id":"cc-1"}' | ./.claude/hooks/majordomus-capture 2>/dev/null)"
[ -z "$out" ] || { echo "    the prompt event repeated itself inside the cooldown: $out"; exit 1; }
set_acc disk_free_min_mb 0
rm -f .ai/local/state/accumulation/said-at
out="$(printf '{"prompt":"fine","prompt_id":"w3","session_id":"cc-1"}' | ./.claude/hooks/majordomus-capture 2>/dev/null)"
[ -z "$out" ] || { echo "    the prompt event spoke with the disk above its floor: $out"; exit 1; }
echo "    the prompt event says the disk is under its floor once per cooldown, and nothing above it"
