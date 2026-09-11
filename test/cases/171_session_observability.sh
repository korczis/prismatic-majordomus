# majordomus-covers: none
# The subsystem can be watched, not only used.
#
# `continuity.state` answers about one episode: the one `state/session-current.yaml`
# resolves to. That is right for a briefing and blind for an operator, because the pointer is
# a symlink the most recent start event re-aims. On 2026-09-11 this repository held five open
# episodes in one checkout and no surface the tool has could name more than one of them; four
# workers were invisible, and an episode nobody can see is an episode that never closes.
#
# So the store is planted here with three episodes — one the pointer names, one belonging to
# a worktree that still exists, one whose worktree is gone — a `.tmp.` file of the kind a
# killed close leaves in the tracked sessions section, and a ledger whose starts outnumber
# its closes. None of this can be reached by running the tool correctly, which is the point:
# these are the states the observability surface exists to name, and a fixture is the only
# way to stand in one of them on purpose.
#
# What is asserted is the difference between the two readings, not merely that each answers:
# `continuity.state` names one episode and `lifecycle.episodes` names three, from the same
# store, in the same process.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: curl not installed"; exit 0; }

mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R" "$W/sibling"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=warn; export MAJORDOMUS_LOG
( cd "$R" && "$MJ" init >/dev/null )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null

S="$R/.ai/local/state"
mkdir -p "$S/sessions-open"

episode() {   # episode PROVIDER_SESSION  SESSION_ID  STARTED_AT  WORKTREE
  cat > "$S/sessions-open/$1.yaml" <<EOF
session_id: $2
started_at: $3
owner: "tester"
provider: "claude-code"
provider_session: "$1"
repository_id: $R/.git
worktree: $4
branch: $(git -C "$R" symbolic-ref --short HEAD)
start_head: $(git -C "$R" rev-parse HEAD)
start_working_tree: clean
EOF
}
episode aaaa-here    s-20260911000001-aaaa 2026-09-11T00:00:01Z "$R"
episode bbbb-sibling s-20260911000002-bbbb 2026-09-11T00:00:02Z "$W/sibling"
episode cccc-gone    s-20260911000003-cccc 2026-09-11T00:00:03Z "$W/vanished"
ln -sf "sessions-open/aaaa-here.yaml" "$S/session-current.yaml"

# The arithmetic ADR 0052 asks for, standing in the state it exists to catch. Five episodes
# started; `dddd` closed and left a record, so it is accounted for; `aaaa`, `bbbb` and `cccc`
# are still open, so they are accounted for too. `eeee` started, is not open, and never
# closed — an end event that reached the tool and produced nothing looks exactly like this,
# and it is the one episode the count can see and no directory listing can.
#
# The first draft of this fixture planted four starts and asserted the books would not
# balance. They balanced, because 4 - 1 - 3 = 0: the fixture was wrong and the code was
# right, which is the good way round and worth leaving written down.
{
  for id in aaaa bbbb cccc dddd eeee; do
    printf '{"ts":"2026-09-11T00:00:01Z","event":"session.started","head":"x","branch":"b","by":"t","session":"s-20260911000001-%s"}\n' "$id"
  done
  printf '{"ts":"2026-09-11T01:00:00Z","event":"task.checkpoint","head":"x","branch":"b","by":"t","session":"s-20260911000001-aaaa","task_id":"t-20260911-0001"}\n'
  printf '{"ts":"2026-09-11T02:00:00Z","event":"session.closed","head":"x","branch":"b","by":"t","session":"s-20260911000001-dddd"}\n'
} > "$S/ledger.jsonl"

# A close that was killed between writing its temporary file and renaming it.
sessions_dir="$R/.ai/repo/sessions"
mkdir -p "$sessions_dir"
printf 'half a record\n' > "$sessions_dir/.tmp.KILLED"

trap '"$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT
( cd "$R" && "$RB" serve ensure --port 0 --idle 60 --wait 40 --format json ) > "$T/ensure.json" 2> "$T/ensure.err" \
  || { echo "    the server did not come up:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
url="$(jq -r '.url' "$T/ensure.json")"
[ -n "$url" ] && [ "$url" != null ] || { echo "    no address"; cat "$T/ensure.json"; exit 1; }

api() { curl -fsS "$url/api/v1/$1" > "$T/$2" || { echo "    GET /api/v1/$1 failed"; exit 1; }; }

# ------------------------------------------------- 1. one reading sees one, the other sees all
api continuity continuity.json
api lifecycle/episodes episodes.json
jq -e '.session.session_id == "s-20260911000001-aaaa"' "$T/continuity.json" >/dev/null || {
  echo "    continuity.state did not resolve the episode the pointer names:"; jq '.session' "$T/continuity.json"; exit 1; }
jq -e '(.episodes | length) == 3' "$T/episodes.json" >/dev/null || {
  echo "    lifecycle.episodes did not read the store; it found:"; jq '[.episodes[].session_id]' "$T/episodes.json"; exit 1; }
jq -e '.current == "s-20260911000001-aaaa"' "$T/episodes.json" >/dev/null || { echo "    the pointer was not followed"; exit 1; }

standing() { jq -r --arg id "$1" '.episodes[] | select(.session_id == $id) | .standing' "$T/episodes.json"; }
[ "$(standing s-20260911000001-aaaa)" = current  ] || { echo "    the pointed-at episode is not \`current\`"; exit 1; }
[ "$(standing s-20260911000002-bbbb)" = foreign  ] || { echo "    an episode of another worktree is not \`foreign\`"; exit 1; }
[ "$(standing s-20260911000003-cccc)" = stranded ] || { echo "    an episode whose worktree is gone is not \`stranded\`"; exit 1; }

# every episode carries a note: a standing a reader cannot act on is the defect being fixed
jq -e 'all(.episodes[]; .note != "")' "$T/episodes.json" >/dev/null || { echo "    an episode has no note"; exit 1; }
# and the ledger is joined by the episode id it stamps, not by a time range
jq -e '.episodes[] | select(.session_id == "s-20260911000001-aaaa") | .events == 2 and .last_event == "task.checkpoint"' \
  "$T/episodes.json" >/dev/null || { echo "    the ledger was not joined by episode id:"; jq '.episodes[0]' "$T/episodes.json"; exit 1; }
# the task relation is the ledger's, and it runs from the episode to the task rather than the
# other way round: an episode names what it touched, and no task decides whether the
# episode's own records get written (ADR 0052)
jq -e '.episodes[] | select(.session_id == "s-20260911000001-aaaa") | .tasks == ["t-20260911-0001"]' \
  "$T/episodes.json" >/dev/null || { echo "    the task relation was not read from the ledger:"; jq '[.episodes[] | {session_id, tasks}]' "$T/episodes.json"; exit 1; }
jq -e '[.episodes[] | select(.session_id == "s-20260911000002-bbbb") | .tasks // []] | flatten | length == 0' \
  "$T/episodes.json" >/dev/null || { echo "    an episode that touched no task was given one"; exit 1; }

# ------------------------------------------------- 2. recovery names what cannot fix itself
api lifecycle/recovery recovery.json
jq -e '[.stranded[] | select(.session_id == "s-20260911000003-cccc")] | length == 1' "$T/recovery.json" >/dev/null || {
  echo "    recovery did not name the stranded episode:"; jq '.stranded' "$T/recovery.json"; exit 1; }
jq -e 'all(.stranded[]; .remedy != "" and .reason != "")' "$T/recovery.json" >/dev/null || {
  echo "    a stranded episode was reported with no remedy; a finding with no remedy is a complaint"; exit 1; }
jq -e '[.orphans[] | select(.path | endswith(".tmp.KILLED"))] | length == 1' "$T/recovery.json" >/dev/null || {
  echo "    the temporary file left in the tracked sessions section was not found:"; jq '.orphans' "$T/recovery.json"; exit 1; }
jq -e '.balance.started == 5 and .balance.closed == 1 and .balance.open == 3' \
  "$T/recovery.json" >/dev/null || { echo "    the started/closed/open counts are wrong:"; jq '.balance' "$T/recovery.json"; exit 1; }
jq -e '.balance.unaccounted == 1 and .balance.agrees == false and (.balance.note | length) > 0' \
  "$T/recovery.json" >/dev/null || {
  echo "    an episode that started, never closed and left no open record was counted as accounted for:"
  jq '.balance' "$T/recovery.json"; exit 1; }
jq -e '[.findings[] | select(contains("neither closed nor left a record"))] | length >= 1' \
  "$T/recovery.json" >/dev/null || {
  echo "    the unbalanced ledger produced no finding; a count nobody is told about is not a check:"
  jq '.findings' "$T/recovery.json"; exit 1; }
jq -e '.pointer.layout == "pointer" and .pointer.resolves == true' "$T/recovery.json" >/dev/null || {
  echo "    the pointer's layout was misread:"; jq '.pointer' "$T/recovery.json"; exit 1; }

# ------------------------------------------------- 3. provider support is declared, not observed
# The fixture has no provider hook directory and no provider configuration of any kind. A
# reader that answered by looking at the filesystem would report that Claude Code has no
# lifecycle at all — which is a statement about whether somebody ran the installer, not
# about the provider.
[ ! -d "$R/.claude" ] || { echo "    the fixture unexpectedly carries a provider hook directory"; exit 1; }
api lifecycle/providers providers.json
jq -e '[.providers[] | select(.id == "claude-code") | .lifecycle[]] | sort == ["PreCompact","SessionEnd","SessionStart"]' \
  "$T/providers.json" >/dev/null || {
  echo "    the declared lifecycle events are not what share/providers.yaml declares:"; jq '.providers[] | select(.id=="claude-code")' "$T/providers.json"; exit 1; }
jq -e '.providers[] | select(.id == "claude-code") | .prompt_capture == true' "$T/providers.json" >/dev/null || {
  echo "    prompt capture support was not reported from the declaration"; exit 1; }
jq -e '[.providers[] | select(.lifecycle == null or (.lifecycle | length) == 0)] | length >= 1' "$T/providers.json" >/dev/null || {
  echo "    every provider claims a lifecycle adapter, which the distribution does not ship"; exit 1; }

# ------------------------------------------------- 4. the process says which commit it answers about
api lifecycle/runtime runtime.json
jq -e '.served_head != "" and .repository_head != "" and (.agree | type) == "boolean" and .note != ""' \
  "$T/runtime.json" >/dev/null || { echo "    the runtime comparison is incomplete:"; cat "$T/runtime.json"; exit 1; }
jq -e '.agree == true' "$T/runtime.json" >/dev/null || {
  echo "    a freshly built index does not agree with the repository it was built from:"; cat "$T/runtime.json"; exit 1; }

# ------------------------------------------------- 5. the Cockpit is the projection of exactly these
curl -fsS -H 'Accept: text/html' "$url/cockpit/continuity" > "$T/page.html" || { echo "    the Continuity page did not render"; exit 1; }
for id in s-20260911000001-aaaa s-20260911000002-bbbb s-20260911000003-cccc; do
  grep -qF "$id" "$T/page.html" || { echo "    the page does not show $id"; exit 1; }
done
grep -qF ".tmp.KILLED" "$T/page.html" || { echo "    the page does not show the orphan temporary file"; exit 1; }
grep -qF "stranded" "$T/page.html" || { echo "    the page does not show the stranded standing as a word"; exit 1; }

# ------------------------------------------------- 6. local, and therefore served and never shipped
# These read .ai/local/. They are exposed over the loopback server and over MCP and they
# declare no command line, because a command line is how a value reaches a script, a log and
# eventually a commit. `local-state-ignored` is the claim that nothing under .ai/local/ is
# published; this is the half of it that belongs to these five.
api capabilities capabilities.json
jq -e '[.capabilities[] | select(.id | startswith("lifecycle."))] | length == 5' "$T/capabilities.json" >/dev/null || {
  echo "    the registry does not carry the five lifecycle capabilities:"; jq '[.capabilities[].id] | map(select(startswith("lifecycle.")))' "$T/capabilities.json"; exit 1; }
jq -e '[.capabilities[] | select(.id | startswith("lifecycle.")) | .exposure.cli] | all(. == null)' \
  "$T/capabilities.json" >/dev/null || {
  echo "    a lifecycle capability declares a command line; these read .ai/local/ and are served, never shipped"; exit 1; }

echo "    3 open episodes read from the store where continuity.state reads 1; recovery, providers, runtime and the page agree"
