# majordomus-covers: knowledge capture
# majordomus-negative: capture session
# claim: knowledge-failure-is-recorded
# A switch that is off says so; a derivation that fails is a ledger line; the hook exits 0.
#
# The deriver runs inside a provider hook, where the exit status belongs to the provider and
# the truth belongs in the ledger. Three things can stop it from writing: the repository
# turned it off, the repository turned something else off, or the write failed. Each must
# be distinguishable afterwards from a deriver that ran and had nothing to say. The first
# is a stderr line naming the switch; the second is nothing at all, because the switches
# are independent; the third is `provider.event.failed`, beside a hook that still exits 0
# and an episode that still closed.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "${TMPDIR:-/tmp}/mj.pol.$$" \
  && cp "${TMPDIR:-/tmp}/mj.pol.$$" .ai/repo/policy.yaml && rm -f "${TMPDIR:-/tmp}/mj.pol.$$"
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
CAND=.ai/repo/knowledge/candidates
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.knowledge.XXXXXX")"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
event_count() { grep -c "\"event\":\"$1\"" "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
compact_event() { printf '{"session_id":"%s","trigger":"auto"}' "$1" | ./.claude/hooks/majordomus-session-compact; }
sid_of() { sed -n 's/^session_id: //p' "$STATE/sessions-open/$1.yaml" | head -n 1; }
candidates() { find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md 2>/dev/null | wc -l | tr -d ' '; }
# Flip one two-space-indented policy key and prove the edit took: sed is silent when its
# pattern matches nothing, and a case that asserts the consequence of a no-op passes for
# free.
pol_set() {
  sed "s/^\(  $1:\) .*/\1 $2/" .ai/repo/policy.yaml > "$S/policy" && cp "$S/policy" .ai/repo/policy.yaml
  grep -q "^  $1: $2" .ai/repo/policy.yaml || { printf '    the policy probe did not take: %s: %s\n' "$1" "$2"; exit 1; }
}

# ---------------------------------------------------------------- (a) the end switch, off
pol_set knowledge_on_end false
start_event e1 >/dev/null 2>&1
SID1="$(sid_of e1)"
must "the start event opened no episode for e1" [ -n "$SID1" ]
"$MJ" decision add "Nothing is derived while the switch is off" --why "the repository said so" >/dev/null
end_event e1 clear > "$S/end1.out" 2> "$S/end1.err"
[ -s "$S/end1.out" ] && { echo "    the end hook wrote to stdout:"; sed 's/^/    | /' "$S/end1.out"; exit 1; }
must "the episode did not close" [ ! -f "$STATE/sessions-open/e1.yaml" ]
[ "$(candidates)" = 0 ] || { echo "    knowledge_on_end: false still wrote a candidate:"; ls -1 "$CAND" | sed 's/^/    | /'; exit 1; }
grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID1\"" "$LEDGER" && { echo "    knowledge_on_end: false still left a knowledge.derived line for $SID1"; exit 1; }
[ "$(event_count knowledge.derived)" = 0 ] || { echo "    knowledge_on_end: false still left a knowledge.derived line"; exit 1; }
grep -q 'knowledge_on_end' "$S/end1.err" || {
  echo "    the hook did not say on stderr that session.knowledge_on_end is off:"; sed 's/^/    | /' "$S/end1.err"; exit 1; }
[ "$(event_count provider.event.failed)" = 0 ] || { echo "    a switch that is off was recorded as a failure"; exit 1; }
pol_set knowledge_on_end true

# ---------------------------------------------------------------- (b) the switches are independent
# The compaction adapter used to return before anything ran when the checkpoint switch was
# off. The knowledge switch is its own, and a repository that wants no compaction checkpoints
# still gets its knowledge derived.
pol_set checkpoint_on_compact false
start_event e2 >/dev/null 2>&1
SID2="$(sid_of e2)"
"$MJ" decision add "The knowledge switch is not the checkpoint switch" --why "two questions, two answers" >/dev/null
# counted around the compaction alone: the end event of (a) wrote its closing checkpoint,
# which is the end adapter's and not this switch's
CP_BEFORE="$(event_count task.checkpoint)"
compact_event e2 > "$S/c2.out" 2> "$S/c2.err"
[ -s "$S/c2.out" ] && { echo "    the compact hook wrote to stdout:"; sed 's/^/    | /' "$S/c2.out"; exit 1; }
[ "$(event_count task.checkpoint)" = "$CP_BEFORE" ] || { echo "    checkpoint_on_compact: false still wrote a checkpoint; the probe is not measuring independence"; exit 1; }
grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID2\"" "$LEDGER" || {
  echo "    checkpoint_on_compact: false suppressed the knowledge derivation; the switches are not independent"
  sed 's/^/    | /' "$S/c2.err"; exit 1; }
[ "$(candidates)" = 1 ] || { printf '    the compaction left %s candidate(s), expected 1\n' "$(candidates)"; exit 1; }
pol_set checkpoint_on_compact true
end_event e2 clear >/dev/null 2>&1
must "episode e2 did not close" [ ! -f "$STATE/sessions-open/e2.yaml" ]

# ---------------------------------------------------------------- (c) the write fails
# The directory the deriver writes into is made unwritable. The hook must still exit 0 and
# write nothing to stdout — the provider is never blocked — and the failure must be a typed
# ledger line rather than a stderr message nobody keeps. The episode still closes: leaving
# it open for ever would be the worse of the two failures.
start_event e3 >/dev/null 2>&1
SID3="$(sid_of e3)"
"$MJ" decision add "A failed write is a ledger line" --why "silence teaches readers to skip the report" >/dev/null
mkdir -p "$CAND"
chmod 0500 "$CAND"
if touch "$CAND/.probe" 2>/dev/null; then
  rm -f "$CAND/.probe"; chmod 0755 "$CAND"
  echo "    skip: the directory stayed writable (running as root); the failure path cannot be provoked here"
  exit 0
fi
rc=0
end_event e3 clear > "$S/end3.out" 2> "$S/end3.err" || rc=$?
chmod 0755 "$CAND"
[ "$rc" = 0 ] || { printf '    the end hook exited %s on a failed derivation; a provider hook exits 0\n' "$rc"; sed 's/^/    | /' "$S/end3.err"; exit 1; }
[ -s "$S/end3.out" ] && { echo "    the end hook wrote to stdout:"; sed 's/^/    | /' "$S/end3.out"; exit 1; }
grep -q '"event":"provider.event.failed".*"event":"end".*"reason":"[^"]*knowledge' "$LEDGER" || {
  echo "    the failed derivation left no provider.event.failed line naming knowledge:"
  grep 'provider.event' "$LEDGER" | sed 's/^/    | /'; sed 's/^/    | /' "$S/end3.err"; exit 1; }
printf '    the failure, as recorded:\n    | %s\n' "$(grep '"event":"provider.event.failed"' "$LEDGER" | tail -n 1)"
grep -q "\"event\":\"session.closed\".*\"session\":\"$SID3\"" "$LEDGER" || {
  echo "    the episode did not close after the derivation failed"; exit 1; }
must "the open record of e3 is still there" [ ! -f "$STATE/sessions-open/e3.yaml" ]
grep -q "\"event\":\"knowledge.derived\".*\"episode\":\"$SID3\"" "$LEDGER" && {
  echo "    a derivation that failed was also recorded as having run"; exit 1; }
[ "$(candidates)" = 1 ] || { printf '    the failed derivation left %s candidate(s), expected the one from (b)\n' "$(candidates)"; exit 1; }
