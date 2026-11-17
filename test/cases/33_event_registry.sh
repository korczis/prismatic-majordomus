# majordomus-covers: history start checkpoint decision question handover finish update
# majordomus-negative: history
# claims: event-vocabulary
#
# The ledger is append-only and canonical, so every line written into it is permanent. An
# event name no reader recognises is therefore a durable record that is silently ignored,
# which is indistinguishable from the event never having happened at all. This case holds
# the vocabulary, the code that writes it, and the code that reads it back to one another.
#
# Four things must agree: share/events.yaml, the mj_ledger_append call sites, the rendering
# in lib/history.sh, and the table in docs/SCHEMAS.md. Any one of them moving alone is the
# drift this case exists to catch. The foreign adoption workflow, which times its steps from
# the ledger, is held to the same vocabulary and to the time field the ledger stamps.
. "$ROOT/test/lib.sh"
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
# shellcheck source=../../lib/common.sh
. "$ROOT/lib/common.sh"

REG="$ROOT/share/events.yaml"
[ -f "$REG" ] || { echo "    no event registry at share/events.yaml"; exit 1; }
FLAT="$T/events.flat"
mj_yaml_flatten "$REG" > "$FLAT" 2>"$T/err" || { echo "    share/events.yaml does not parse: $(cat "$T/err")"; exit 1; }
grep -qx 'version=1' "$FLAT" || { echo "    event registry version must be 1"; exit 1; }

unk="$(mj_yaml_unknown_keys "$FLAT" "$ROOT/share/allow/events.txt" || true)"
[ -z "$unk" ] || { echo "    unknown key(s) in share/events.yaml: $(printf '%s' "$unk" | tr '\n' ' ')"; exit 1; }

registered="$(sed -n 's/^events\.[0-9]*\.id=//p' "$FLAT" | LC_ALL=C sort)"
[ -n "$registered" ] || { echo "    the registry declares no events"; exit 1; }
dupes="$(printf '%s\n' "$registered" | uniq -d)"
[ -z "$dupes" ] || { echo "    duplicate event id(s): $dupes"; exit 1; }

# 1. every name the source writes is registered, and every registered name is written by
#    something. An event nothing writes is a phantom — docs/SCHEMAS.md documented a
#    `bootstrap` event for months that no code has ever emitted.
#    A helper that appends the event its caller names writes a variable, and a scan that
#    reads source cannot follow it. Such a helper declares what reaches the ledger through it
#    on a `# mj_ledger_append-writes: <id>...` line, and the declaration is read here — the
#    names still have to be registered below, so a declaration cannot invent an event.
written="$( { grep -rhoE 'mj_ledger_append [a-z][a-z._]*' "$ROOT/lib" | awk '{print $2}'
              grep -rhoE '# mj_ledger_append-writes:[a-z._ ]*' "$ROOT/lib" | sed 's/.*://' | tr ' ' '\n'
            } | grep -E '^[a-z][a-z._]*$' | grep -v '^event$' | LC_ALL=C sort -u)"
[ -n "$written" ] || { echo "    could not find any mj_ledger_append call site"; exit 1; }
for e in $written; do
  printf '%s\n' "$registered" | grep -Fxq "$e" || {
    echo "    lib/ writes '$e', which share/events.yaml does not declare"; exit 1; }
done
for e in $registered; do
  printf '%s\n' "$written" | grep -Fxq "$e" || {
    echo "    share/events.yaml declares '$e', which nothing in lib/ writes"; exit 1; }
done

# 2. each entry's emitted_by names a real command whose module actually writes it, so the
#    registry cannot drift into describing the wrong source.
i=0
while [ -n "$(sed -n "s|^events\.$i\.id=||p" "$FLAT")" ]; do
  id="$(sed -n "s|^events\.$i\.id=||p" "$FLAT")"
  by="$(sed -n "s|^events\.$i\.emitted_by=||p" "$FLAT")"
  [ -f "$ROOT/lib/$by.sh" ] || { echo "    $id declares emitted_by '$by', but lib/$by.sh does not exist"; exit 1; }
  # the module the registry names must be one that writes it. Another module may write it
  # too — check --checkpoint records a task.checkpoint — but the declared owner may not be
  # a module that never mentions it.
  esc="$(printf '%s' "$id" | sed 's/\./\\./g')"
  grep -qE "mj_ledger_append $esc( |\$)|# mj_ledger_append-writes:[a-z._ ]*( |:)$esc( |\$)" \
    "$ROOT/lib/$by.sh" || {
    echo "    $id declares emitted_by '$by', but lib/$by.sh never writes it"; exit 1; }
  [ -n "$(sed -n "s|^events\.$i\.summary=||p" "$FLAT")" ] || { echo "    $id has no summary"; exit 1; }
  i=$((i+1))
done

# 3. every registered event has a human rendering, and the renderer names no event the
#    registry does not. This is the one place a per-event branch is allowed to exist —
#    rendering is presentation — so it is reconciled rather than removed.
rendered="$(grep -oE 'e == "[a-z][a-z._]*"' "$ROOT/lib/history.sh" | sed 's/.*"\(.*\)"/\1/' | LC_ALL=C sort -u)"
for e in $rendered; do
  printf '%s\n' "$registered" | grep -Fxq "$e" || {
    echo "    lib/history.sh renders '$e', which is not a registered event"; exit 1; }
done

# 4. the schema document lists exactly the registered vocabulary
for e in $registered; do
  grep -q "| \`$e\` |" "$ROOT/docs/SCHEMAS.md" || {
    echo "    docs/SCHEMAS.md does not document the event '$e'"; exit 1; }
done
# The events table is one section of a file full of field tables, and a field name may carry
# a dot too (children.require_contract), so the rows are read from that section alone rather
# than from every row in the file that happens to look like one.
events_table() { awk '/^\| event \| extra fields \|$/ { t = 1; next } t && !/^\|/ { exit } t' "$ROOT/docs/SCHEMAS.md"; }
for e in $(events_table | grep -oE '^\| `[a-z][a-z._]*` \|' | sed 's/^| `//; s/` |$//' | LC_ALL=C sort -u); do
  printf '%s\n' "$registered" | grep -Fxq "$e" || {
    echo "    docs/SCHEMAS.md documents the event '$e', which is not registered"; exit 1; }
done

# ---- behaviour, through the real binary -------------------------------------------------
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" start "t" --scope lib >/dev/null
id="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"

# a registered name filters; an unregistered one is refused rather than answered with the
# empty result a registered-but-absent name would produce
expect_exit 0 "$MJ" history --event task.started
expect_grep 'task.started'
expect_exit 0 "$MJ" history --event question.opened
expect_grep 'no matching events'
expect_exit 2 "$MJ" history --event task.startd
expect_grep "unknown event 'task.startd'"
expect_grep 'one of: .*task.started'

# --validate rejects a stored line whose name no reader knows. This is the durable-record
# failure the registry exists to prevent, so it is proved on a real ledger.
expect_exit 0 "$MJ" history --validate
printf '{"ts":"2026-09-04T00:00:00Z","event":"task.invented","head":"x","branch":"b","by":"majordomus/0.1.0","task_id":"%s"}\n' "$id" \
  >> .ai/local/state/ledger.jsonl
expect_exit 10 "$MJ" history --validate
expect_grep 'FAIL ledger +task.invented'
expect_grep 'not a registered event'
# and the tool itself refuses to write one: the guard is on the append, not only on the read
sed -i.bak '$d' .ai/local/state/ledger.jsonl && rm -f .ai/local/state/ledger.jsonl.bak
expect_exit 0 "$MJ" history --validate

probe="$T/probe.sh"
cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append task.invented '"task_id":"x"'
PROBE
expect_exit 13 bash "$probe"
expect_grep "unregistered event 'task.invented'"
# a registered name missing a field the registry requires is refused for that reason
cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append question.opened '"task_id":"x"'
PROBE
expect_exit 13 bash "$probe"
expect_grep "missing the required field 'question'"
# ... and the same call with the field present is written, so the check above is not
# rejecting everything
cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append question.opened '"task_id":"x","question":"real"'
PROBE
expect_exit 0 bash "$probe"
expect_grep '"event":"question.opened".*"question":"real"' .ai/local/state/ledger.jsonl

# 5. the foreign adoption workflow times its steps from this ledger, so what it says about
#    the ledger is held to what the commands write. It once read a ledger time from `at`, a
#    field no ledger line has carried, and said a refused check writes no line as if a
#    passing one wrote some.
LEDGER=.ai/local/state/ledger.jsonl
lines() { wc -l < "$LEDGER" | tr -d ' '; }
clock() { date -u +%Y-%m-%dT%H:%M:%SZ; }
# a plain check writes nothing, passing or refused; check --checkpoint writes one line
rm -f "$probe"   # outside the task's scope, so a check would refuse over it
n="$(lines)"
expect_exit 0 "$MJ" check
[ "$(lines)" = "$n" ] || { echo "    a passing check wrote: $(tail -n 1 "$LEDGER")"; exit 1; }
echo x > outside && git add outside
expect_exit 10 "$MJ" check
[ "$(lines)" = "$n" ] || { echo "    a refused check wrote: $(tail -n 1 "$LEDGER")"; exit 1; }
git rm -qf --cached outside && rm -f outside
"$MJ" check --checkpoint >/dev/null
[ "$(lines)" = $((n + 1)) ] && tail -n 1 "$LEDGER" | grep -q '"event":"task.checkpoint"' \
  || { echo "    check --checkpoint did not write one task.checkpoint line"; exit 1; }
# each ledger step's line is stamped inside the run clock read around its command, under the
# key the workflow must name; the key is read off the line, not assumed
timed() {   # <event> <command...>
  local ev="$1" before after line stamp was; shift
  was="$(lines)"; before="$(clock)"; "$@" >/dev/null; after="$(clock)"
  [ "$(lines)" -gt "$was" ] || { echo "    $* wrote no ledger line"; exit 1; }
  line="$(grep "\"event\":\"$ev\"" "$LEDGER" | tail -n 1)"
  key="$(printf '%s' "$line" | sed -n 's/^{"\([a-z_]*\)":"[0-9-]\{10\}T[0-9:]\{8\}Z".*/\1/p')"
  stamp="$(printf '%s' "$line" | sed -n 's/^{"[a-z_]*":"\([^"]*\)".*/\1/p')"
  [ -n "$key" ] || { echo "    the $ev line starts with no UTC time: $line"; exit 1; }
  [[ "$stamp" < "$before" || "$stamp" > "$after" ]] && {
    echo "    $ev is stamped $stamp, outside the run clock $before..$after"; exit 1; }
  return 0
}
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n'; }
note | "$MJ" handover >/dev/null
timed task.finished "$MJ" finish --outcome completed --verify-command "test -d .ai"
timed task.started "$MJ" start "t2" --scope lib
timed projections.updated "$MJ" update
expect_no_grep '"at":' "$LEDGER"

# the workflow names that key, and each event its ledger rows name is registered and written
# by the command of that row
adoption_claims() {   # <workflow file>
  local f="$1" field row cmd ev esc i by
  field="$(tr '\n' ' ' < "$f" \
    | sed -nE "s/.*A ledger time is the (line|event)'s own \`([a-z_]+)\`.*/\2/p")"
  [ "$field" = "$key" ] || {
    echo "    $f reads a ledger time from '${field:-nothing}'; the ledger stamps it as '$key'"
    return 1; }
  grep -E '^\| [^|]+ \| `majordomus [a-z]+[^|]*\| ledger: ' "$f" > "$T/ledger-rows" \
    || { echo "    $f names no ledger step"; return 1; }
  while IFS= read -r row; do
    cmd="$(printf '%s' "$row" | sed -n 's/^| [^|]* | `majordomus \([a-z]*\).*/\1/p')"
    for ev in $(printf '%s' "$row" | awk -F'|' '{print $4}' | grep -oE '`[a-z][a-z._]*`' \
                | tr -d '`'); do
      esc="$(printf '%s' "$ev" | sed 's/\./\\./g')"
      i="$(sed -n "s/^events\.\([0-9]*\)\.id=$esc\$/\1/p" "$FLAT")"
      [ -n "$i" ] || { echo "    $f names the event '$ev', which is not registered"; return 1; }
      by="$(sed -n "s/^events\.$i\.emitted_by=//p" "$FLAT")"
      [ "$by" = "$cmd" ] || {
        echo "    $f times '$cmd' by '$ev', which '$by' writes"; return 1; }
    done
  done < "$T/ledger-rows"
}
ADOPT="$ROOT/.ai/repo/workflows/foreign-repository-adoption.md"
adoption_claims "$ADOPT"
# ... and the check tells a wrong field and a misattributed event from the real text
sed 's/the line'"'"'s own `ts`/the line'"'"'s own `at`/' "$ADOPT" > "$T/adopt-at.md"
expect_exit 1 adoption_claims "$T/adopt-at.md"
expect_grep "reads a ledger time from 'at'"
sed 's/| ledger: `task.started` |/| ledger: `task.finished` |/' "$ADOPT" > "$T/adopt-by.md"
expect_exit 1 adoption_claims "$T/adopt-by.md"
expect_grep "times 'start' by 'task.finished', which 'finish' writes"

printf '    %s registered events, written, rendered and documented\n' "$(printf '%s\n' "$registered" | wc -w | tr -d ' ')"
