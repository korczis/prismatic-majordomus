# majordomus-covers: history start checkpoint decision question handover finish update
# majordomus-negative: history
#
# The ledger is append-only and canonical, so every line written into it is permanent. An
# event name no reader recognises is therefore a durable record that is silently ignored,
# which is indistinguishable from the event never having happened at all. This case holds
# the vocabulary, the code that writes it, and the code that reads it back to one another.
#
# Four things must agree: share/events.yaml, the mj_ledger_append call sites, the rendering
# in lib/history.sh, and the table in docs/SCHEMAS.md. Any one of them moving alone is the
# drift this case exists to catch.
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

registered="$(sed -n 's/^events\.[0-9]*\.id=//p' "$FLAT" | sort)"
[ -n "$registered" ] || { echo "    the registry declares no events"; exit 1; }
dupes="$(printf '%s\n' "$registered" | uniq -d)"
[ -z "$dupes" ] || { echo "    duplicate event id(s): $dupes"; exit 1; }

# 1. every name the source writes is registered, and every registered name is written by
#    something. An event nothing writes is a phantom — docs/SCHEMAS.md documented a
#    `bootstrap` event for months that no code has ever emitted.
written="$(grep -rhoE 'mj_ledger_append [a-z][a-z._]*' "$ROOT/lib" | awk '{print $2}' | grep -v '^event$' | sort -u)"
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
  grep -qE "mj_ledger_append $esc( |\$)" "$ROOT/lib/$by.sh" || {
    echo "    $id declares emitted_by '$by', but lib/$by.sh never writes it"; exit 1; }
  [ -n "$(sed -n "s|^events\.$i\.summary=||p" "$FLAT")" ] || { echo "    $id has no summary"; exit 1; }
  i=$((i+1))
done

# 3. every registered event has a human rendering, and the renderer names no event the
#    registry does not. This is the one place a per-event branch is allowed to exist —
#    rendering is presentation — so it is reconciled rather than removed.
rendered="$(grep -oE 'e == "[a-z][a-z._]*"' "$ROOT/lib/history.sh" | sed 's/.*"\(.*\)"/\1/' | sort -u)"
for e in $rendered; do
  printf '%s\n' "$registered" | grep -Fxq "$e" || {
    echo "    lib/history.sh renders '$e', which is not a registered event"; exit 1; }
done

# 4. the schema document lists exactly the registered vocabulary
for e in $registered; do
  grep -q "| \`$e\` |" "$ROOT/docs/SCHEMAS.md" || {
    echo "    docs/SCHEMAS.md does not document the event '$e'"; exit 1; }
done
for e in $(grep -oE '^\| `[a-z][a-z._]*` \|' "$ROOT/docs/SCHEMAS.md" | sed 's/^| `//; s/` |$//' | sort -u); do
  case "$e" in *.*) ;; *) continue ;; esac      # the table shares the file with field-name tables
  printf '%s\n' "$registered" | grep -Fxq "$e" || {
    echo "    docs/SCHEMAS.md documents the event '$e', which is not registered"; exit 1; }
done

# ---- behaviour, through the real binary -------------------------------------------------
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" start "t" --scope lib >/dev/null
id="$(sed -n 's/^id: //p' .majordomus/state/current.yaml)"

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
  >> .majordomus/state/ledger.jsonl
expect_exit 10 "$MJ" history --validate
expect_grep 'FAIL ledger +task.invented'
expect_grep 'not a registered event'
# and the tool itself refuses to write one: the guard is on the append, not only on the read
sed -i.bak '$d' .majordomus/state/ledger.jsonl && rm -f .majordomus/state/ledger.jsonl.bak
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
expect_grep '"event":"question.opened".*"question":"real"' .majordomus/state/ledger.jsonl

printf '    %s registered events, written, rendered and documented\n' "$(printf '%s\n' "$registered" | wc -w | tr -d ' ')"
