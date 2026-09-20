# majordomus-covers: capture history
# majordomus-negative: capture
# No event the ledger declares is named after one provider's lifecycle event.
#
# ADR 0052 asked for three receipts — provider.session_start.received,
# provider.pre_compact.received, provider.session_end.received. Those three names are
# `claude-code.lifecycle` in share/providers.yaml, and they are declared there precisely so
# that nothing in the tool enumerates them (project.providers-are-data). Built into event
# ids they would have compiled one provider's vocabulary into the ledger every provider
# writes to: the next adapter would have had either a receipt whose id names an event it
# does not have, or a fourth id nobody could derive from the data.
#
# So the id says what happened to this repository — a provider lifecycle event arrived —
# and which event, from whom, is what the line carries. That was a convention no check held,
# which is why the shape the ADR asked for could have been built at any time. Both halves
# are asserted here, because neither alone is the property: an id that names no provider
# event, and a receipt that still distinguishes the events in a field.
. "$ROOT/test/lib.sh"

EVENTS="$ROOT/share/events.yaml"
PROVIDERS="$ROOT/share/providers.yaml"

# ---------------------------------------------------------------- the declared set
# Read from the data, never a list written here: a provider added tomorrow is covered by
# this case on the day it is declared, which is the whole point of the rule being tested.
lifecycle_names() {
  awk '
    /^  [a-z0-9-]+:$/ { p = $1; sub(":", "", p) }
    /^    lifecycle:$/ { f = 1; next }
    f && /^      - / { print $2; next }
    f && !/^      - / { f = 0 }
  ' "$PROVIDERS"
}

names="$(lifecycle_names)"
[ -n "$names" ] || { echo "    share/providers.yaml declares no lifecycle event for any provider; this case would assert nothing"; exit 1; }

event_ids() { sed -n 's/^  - id: \([a-z0-9._-]*\)$/\1/p' "$EVENTS"; }

ids="$(event_ids)"
[ -n "$ids" ] || { echo "    share/events.yaml declares no event id"; exit 1; }

# ---------------------------------------------------------------- no id names one of them
# Compared in the form an id would take: PreCompact -> pre_compact, SessionStart ->
# session_start. A hand-written `provider.session_start.received` is exactly the shape ADR
# 0052 asked for and the shape this refuses.
fail=0
for n in $names; do
  snake="$(printf '%s' "$n" | sed 's/\([a-z0-9]\)\([A-Z]\)/\1_\2/g' | tr '[:upper:]' '[:lower:]')"
  for id in $ids; do
    case "$id" in
      *"$snake"*)
        echo "    share/events.yaml declares $id, built from the provider lifecycle event $n"
        echo "    a provider's own event names are data (share/providers.yaml); the receipt carries them in provider_event"
        fail=1 ;;
    esac
  done
done
[ "$fail" = 0 ] || exit 1

# ---------------------------------------------------------------- and the pair still distinguishes
# The neutrality above is only right because the distinction survives in the payload. If
# this half ever goes, the receipts stop being a vocabulary and the first half becomes a
# licence to record nothing.
expect_grep '^  - id: provider\.event\.received$' "$EVENTS"
expect_grep '^  - id: provider\.event\.failed$' "$EVENTS"
expect_grep 'requires: \[provider, provider_event\]' "$EVENTS"

# ---------------------------------------------------------------- against the real writer
# Through the shim a provider runs, so the assertion is about what the tool produces rather
# than about a line written here.
"$MJ" init >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
LEDGER=.ai/local/state/ledger.jsonl

for e in start compact end; do
  "$MJ" capture session --provider claude-code --event "$e" </dev/null >/dev/null 2>&1 || true
done

expect_grep '"event":"provider.event.received"' "$LEDGER"
for e in start compact end; do
  expect_grep "\"provider_event\":\"$e\"" "$LEDGER"
done

# every receipt names the provider it came from, so a ledger written by two adapters can be
# read apart without the id having to carry the provider
lines="$(grep -c '"event":"provider.event.received"' "$LEDGER" || true)"; : "${lines:=0}"
named="$(grep -c '"event":"provider.event.received".*"provider":"[^"]\+"' "$LEDGER" || true)"; : "${named:=0}"
[ "$lines" = "$named" ] || {
  echo "    $lines receipt(s) written, $named name their provider"
  exit 1; }

echo "    $lines receipt(s), $(printf '%s' "$ids" | wc -l | tr -d ' ') declared event id(s), none named after a provider's own event"
