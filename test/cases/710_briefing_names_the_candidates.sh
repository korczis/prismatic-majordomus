# majordomus-covers: knowledge context session capture
# claim: briefing-names-candidates
# The next worker is told what awaits review on this branch, bounded, and told when nothing does.
#
# A candidate nobody is told about is a candidate nobody reviews. The start briefing is the
# one route by which a record reaches a worker without the worker remembering to ask (ADR
# 0017), so it gains one section: the candidates whose episode was on this branch, the count
# on one line and the ids beneath it, bounded the way the open-questions block is and inside
# the briefing budget. Absence is printed rather than omitted, and a candidate whose episode
# this checkout cannot place is named as such rather than hidden — a queue that quietly
# shrinks on a clone is the same defect as one that quietly grows.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
CAND=.ai/repo/knowledge/candidates
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
BUDGET="$(awk '/^  briefing_budget_lines:/ { print $2 }' .ai/repo/policy.yaml)"
must "the policy declares no briefing budget" [ -n "$BUDGET" ]
BRANCH="$(git branch --show-current)"
ABSENT='No knowledge candidates await review on this branch'
PRESENT='Knowledge candidates awaiting review on this branch'

# the briefing of one start event, on stdout, with the budget asserted every time
briefing() {   # provider-session -> $S/brief.txt
  start_event "$1" > "$S/brief.txt" 2> "$S/brief.err"
  local lines; lines="$(wc -l < "$S/brief.txt" | tr -d ' ')"
  [ "$lines" -le "$((BUDGET + 1))" ] || { printf '    the briefing is %s lines, budget %s\n' "$lines" "$BUDGET"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
  [ -s "$S/brief.txt" ] || { echo "    the start hook wrote no briefing"; sed 's/^/    | /' "$S/brief.err"; exit 1; }
}
# an episode that records N decisions and closes, on the current branch
episode_with() {   # provider-session count
  local i=1
  start_event "$1" >/dev/null 2>&1
  while [ "$i" -le "$2" ]; do
    "$MJ" decision add "Decision $i of episode $1" --why "to fill the queue" >/dev/null
    i=$((i + 1))
  done
  end_event "$1" >/dev/null 2>&1
  must "episode $1 did not close" [ ! -f "$STATE/sessions-open/$1.yaml" ]
}

# ---------------------------------------------------------------- nothing yet: absence is printed
briefing e0
grep -qF "$ABSENT" "$S/brief.txt" || { echo "    the briefing did not say that no candidate awaits review:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
end_event e0 >/dev/null 2>&1

# ---------------------------------------------------------------- one candidate, named
episode_with e1 1
ID1="$(find "$CAND" -maxdepth 1 -name 's-*.md' | LC_ALL=C sort | head -n 1)"
[ -n "$ID1" ] || { echo "    the close derived no candidate"; exit 1; }
ID1="$(basename "$ID1" .md)"
briefing e2
printf '    briefing with one candidate:\n'; grep -A3 -F "$PRESENT" "$S/brief.txt" | sed 's/^/    | /'
grep -qF "$PRESENT: 1" "$S/brief.txt" || { echo "    the briefing did not count the one candidate:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
grep -qE "^- $ID1  " "$S/brief.txt" || { echo "    the briefing did not name the candidate $ID1 with its title"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
grep -qF "$ABSENT" "$S/brief.txt" && { echo "    the briefing printed both the candidate and its absence"; exit 1; }
end_event e2 >/dev/null 2>&1

# The full briefing a worker asks for by hand carries the same section: the start event is
# where a worker is told without asking, `majordomus context` is where one asks, and the
# start briefing's last line sends the worker there for the whole of it. The same two lines,
# the count and the id, are asserted; the section heading the context builder gives them is
# its own.
expect_exit 0 "$MJ" context
printf '%s\n' "$LAST_OUT" | grep -qF "$PRESENT: 1" || {
  echo "    majordomus context does not carry the knowledge section the start briefing carries (the count line is absent)"
  printf '%s\n' "$LAST_OUT" | grep -n '^## ' | sed 's/^/    | /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -qE "^- $ID1  " || {
  echo "    majordomus context counts the candidate but does not name it: $ID1"; exit 1; }

# ---------------------------------------------------------------- many candidates, bounded
episode_with e3 7
briefing e4
printf '    briefing with eight candidates:\n'; grep -A7 -F "$PRESENT" "$S/brief.txt" | sed 's/^/    | /'
grep -qF "$PRESENT: 8" "$S/brief.txt" || { echo "    the briefing did not count eight candidates:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
grep -qE '^- \.\.\. and 3 more candidate\(s\)$' "$S/brief.txt" || { echo "    the list is not bounded to five ids with the rest counted:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
[ "$(grep -cE '^- s-[0-9]+-[0-9a-f]+-[0-9a-f]+  ' "$S/brief.txt")" = 5 ] || {
  printf '    %s ids listed, expected five\n' "$(grep -cE '^- s-[0-9]+-[0-9a-f]+-[0-9a-f]+  ' "$S/brief.txt")"; exit 1; }
end_event e4 >/dev/null 2>&1

# ---------------------------------------------------------------- another branch: absence
# The candidates' episodes were on $BRANCH. A worker on another branch is told nothing awaits
# review there, because a briefing quietly about somebody else's work is worse than none.
git checkout -qb other
briefing e5
grep -qF "$ABSENT" "$S/brief.txt" || { echo "    on branch other the briefing still offers $BRANCH's candidates:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
grep -qF "$PRESENT" "$S/brief.txt" && { echo "    on branch other the briefing counts $BRANCH's candidates"; exit 1; }
end_event e5 >/dev/null 2>&1
git checkout -q "$BRANCH"

# ---------------------------------------------------------------- an episode this checkout cannot place
# A candidate pulled from another checkout names an episode whose record has not arrived
# here yet. It is not on this branch and it is not absent; it is named apart, so that a
# reviewer knows the queue is longer than the branch shows.
cat > "$CAND/hand-unknown-episode.md" <<Y
---
schema: knowledge/v1
id: hand-unknown-episode
kind: knowledge
class: convention
title: "A candidate from an episode this checkout has not seen"
description: "Hand-made to stand for a record pulled from elsewhere."
status: candidate
epistemics: decided
date: 2026-09-12
tags:
  - derived
  - decision
provenance:
  origin: extracted
  derived_from:
    - session:s-20200101000000-dead
---

# A candidate from an episode this checkout has not seen

Hand-made to stand for a record pulled from elsewhere.
Y
briefing e6
printf '    briefing with an unplaceable candidate:\n'; grep -B1 -A2 'not known in this checkout' "$S/brief.txt" | sed 's/^/    | /'
grep -qE '^1 candidate\(s\) whose episode is not known in this checkout' "$S/brief.txt" || {
  echo "    the briefing did not name the candidate whose episode it cannot place:"; sed 's/^/    | /' "$S/brief.txt"; exit 1; }
grep -qE '^- hand-unknown-episode  ' "$S/brief.txt" || { echo "    the unplaceable candidate's id is not listed"; exit 1; }
grep -qF "$PRESENT: 8" "$S/brief.txt" || { echo "    the unplaceable candidate was counted as this branch's"; exit 1; }
end_event e6 >/dev/null 2>&1

expect_exit 0 "$MJ" knowledge candidates --json
row="$(printf '%s\n' "$LAST_OUT" | grep -o '{[^{}]*"id":"hand-unknown-episode"[^{}]*}')"
[ -n "$row" ] || { echo "    knowledge candidates --json does not list the unplaceable candidate"; exit 1; }
printf '%s' "$row" | grep -q '"branch":null' || { printf '    the unplaceable candidate does not carry branch: null:\n    | %s\n' "$row"; exit 1; }
row="$(printf '%s\n' "$LAST_OUT" | grep -o "{[^{}]*\"id\":\"$ID1\"[^{}]*}")"
printf '%s' "$row" | grep -q "\"branch\":\"$BRANCH\"" || { printf '    the placed candidate does not carry its branch:\n    | %s\n' "$row"; exit 1; }
