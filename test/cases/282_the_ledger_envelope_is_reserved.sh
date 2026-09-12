# majordomus-covers: capture history
# majordomus-negative: history capture
# A payload may not use one of the ledger envelope's own key names.
#
# mj_ledger_append writes ts, event, head, branch, by and session itself, then appends the
# caller's extra fields verbatim. Two callers passed a field literally named `event` — the
# provider's own event name in the lifecycle receipt — so every receipt this repository has
# written since d5d541850 carried the envelope key twice:
#
#   {"ts":...,"event":"provider.event.received",...,"provider":"claude-code","event":"start"}
#
# JSON permits that and says nothing about which of the two wins, so the line meant one
# thing to `history` (which matches, taking the first) and another to `history --validate`
# (which used a greedy sed, taking the last). Validate read the event name as `start`, which
# share/events.yaml does not declare, and refused a healthy ledger with exit 10 — on every
# developer checkout, and on no CI runner, because no runner's ledger holds a receipt.
#
# Two halves, and each is asserted on its own, because neither is sufficient. The writer now
# refuses the collision, which is what makes it impossible to reintroduce in any caller. The
# readers take the first occurrence through one shared extraction, which is what keeps the
# lines already written readable — a reader that takes the last of a duplicated key stays
# wrong about them however careful the writer becomes.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
LEDGER=.ai/local/state/ledger.jsonl

# ---------------------------------------------------------------- the real writer
# Through the shim the provider runs, not through a hand-written line: the defect was in
# what this command produces, and a fixture line would only prove the assertion.
for e in start compact end; do
  "$MJ" capture session --provider claude-code --event "$e" </dev/null >/dev/null 2>&1 || true
done
expect_grep '"event":"provider.event.received"' "$LEDGER"
# no line carries the same key twice
dup="$(grep -c '"event":.*"event":' "$LEDGER" || true)"; : "${dup:=0}"
[ "$dup" = 0 ] || {
  echo "    $dup ledger line(s) carry \"event\" twice:"
  grep -n '"event":.*"event":' "$LEDGER" | sed 's/^/    | /'
  exit 1; }
# the provider's own event name is still recorded, under a name of its own
expect_grep '"provider_event":"start"' "$LEDGER"
expect_grep '"provider_event":"compact"' "$LEDGER"
expect_grep '"provider_event":"end"' "$LEDGER"
# and the registry asks for it by that name, so a caller cannot satisfy the contract with
# the colliding one
expect_grep 'requires: \[provider, provider_event\]' "$ROOT/share/events.yaml"

expect_exit 0 "$MJ" history --validate
expect_grep 'history --validate: ok'

# ---------------------------------------------------------------- the writer refuses it
# Directly, because no command offers a way to pass an arbitrary payload — which is the
# point: the guard is what makes the collision unreachable from any future caller.
probe="$T/probe.sh"
write_probe() {                                   # write_probe EVENT EXTRA
  cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append $1 '$2'
PROBE
}
# refuses KEY VALUE — the payload field named KEY must be refused as an internal error,
# and the message must name the field, or a reader of the failure cannot tell this from
# any other refusal.
refuses() {
  local key="$1" val="$2" got=0 out
  write_probe question.opened "\"task_id\":\"x\",\"question\":\"q\",\"$key\":$val"
  out="$(bash "$probe" 2>&1)" || got=$?
  if [ "$got" != 13 ]; then
    printf '    a payload field named %s was accepted (exit %s, expected 13)\n' "$key" "$got"
    printf '    the line would carry "%s" twice; every reader would then be free to disagree\n' "$key"
    printf '    about which of the two it means, which is the defect this guard ends\n'
    [ -n "$out" ] && printf '%s\n' "$out" | sed 's/^/    | /'
    grep -n "\"$key\":$val" "$LEDGER" 2>/dev/null | sed 's/^/    written: /' || true
    return 1
  fi
  printf '%s\n' "$out" | grep -qE "carries the payload field '$key'" || {
    printf '    the refusal of %s does not name the field:\n' "$key"
    printf '%s\n' "$out" | sed 's/^/    | /'
    return 1; }
  printf '%s\n' "$out" | grep -qE 'envelope key' || {
    printf '    the refusal of %s does not say why it is one:\n' "$key"
    printf '%s\n' "$out" | sed 's/^/    | /'
    return 1; }
}
# every envelope key, not only the one that was actually collided with
refuses event '"start"'
refuses ts '"2026-01-01T00:00:00Z"'
refuses session '"s-forged"'
refuses head '"deadbeef"'
refuses branch '"other"'
refuses by '"somebody"'

# A key that merely ends in an envelope name is not a collision, and neither is an envelope
# name inside a value: the guard matches with the opening quote, and mj_json_esc turns a
# quote inside a value into \". Refusing these would break session.recovered, which carries
# both session_id and session_path — so this half is asserted too, or the guard could be
# "correct" by refusing everything.
write_probe session.recovered '"session_id":"s-1","session_path":".ai/repo/sessions/x.md","reason":"said \"event\": in prose"'
expect_exit 0 bash "$probe"
expect_grep '"event":"session.recovered"' "$LEDGER"
expect_grep '"session_id":"s-1"' "$LEDGER"
expect_exit 0 "$MJ" history --validate

# ---------------------------------------------------------------- the readers, on old lines
# The line the old writer produced. Every reader must resolve it to its envelope's event
# name: this is what a developer's ledger is full of, and rewriting an append-only file is
# not an option.
printf '{"ts":"2026-09-12T10:00:00Z","event":"provider.event.received","head":"abc0000","branch":"master","by":"majordomus/0.6.0","session":"s-legacy","provider":"claude-code","event":"start"}\n' >> "$LEDGER"
expect_exit 0 "$MJ" history --validate
expect_grep 'history --validate: ok'
expect_no_grep "not a registered event"
# it filters as the event it is, and not as the payload's value
expect_exit 0 "$MJ" history --event provider.event.received --all
expect_grep '2026-09-12T10:00:00Z +provider.event.received'
expect_exit 2 "$MJ" history --event start
expect_grep "unknown event 'start'"
# and doctor, which reads the same file, does not call it corrupt
expect_exit 0 "$MJ" history --validate

# ---------------------------------------------------------------- one extraction, not four
# The greedy `sub(/^.*"key":"/, ...)` is what made the readers disagree: awk's .* is greedy,
# so it returns the last occurrence. mj_history_render had it right with `match`; two others
# did not. There is now one declaration, and the broken form must not reappear.
# Comment lines are excluded the way test/cases/08 excludes them: the prose that records
# why this is forbidden has to be able to quote it.
if grep -rnE 'sub\(/\^\.\*"(event|ts|session|head|branch|by)":"/' "$ROOT/lib" \
   | grep -vE '^[^:]+:[0-9]+:[[:space:]]*#'; then
  echo "    a reader extracts a ledger key with a greedy sub(), which takes the LAST"
  echo "    occurrence of a duplicated key instead of the envelope's own; use mjfield from"
  echo "    MJ_LEDGER_FIELD_AWK"
  exit 1
fi
grep -q 'MJ_LEDGER_ENVELOPE_KEYS=' "$ROOT/lib/common.sh" \
  || { echo "    the envelope's key list is no longer declared in lib/common.sh"; exit 1; }
grep -q 'MJ_LEDGER_FIELD_AWK=' "$ROOT/lib/common.sh" \
  || { echo "    the shared first-occurrence extraction is no longer declared"; exit 1; }

printf '    the envelope is reserved at the writer, and read as the envelope by every reader\n'
