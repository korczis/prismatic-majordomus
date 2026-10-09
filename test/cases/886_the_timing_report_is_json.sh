# majordomus-covers: doctor context
# The timing report is one line of JSON under --json, and it carries what the text form does.
#
# MJ_TIMING=1 makes a command report its phases (time and calls) and its work counters on
# stderr. The text form is for a person; a check that compares one run's breakdown with
# another's needs the same data in a form it can parse. Under --json the report is one
# JSON object, {"timing":{"clock","total_ms","phases":[{name,ms,calls}],"counters":[{name,
# count}]}}, documented in docs/SCHEMAS.md ("The timing report").
#
# Held here:
#   1. the JSON form of a real command parses, and names exactly the phases and counters the
#      text form of the same command names, with the same counts (times differ run to run).
#      The command is doctor, which records some forty phases in a fresh repository: two empty
#      lists would agree and prove nothing;
#   2. the command's own stdout is the same with and without the report (context, whose
#      output does not carry timings of its own the way doctor's budget lines do), both
#      runs given one clock;
#   3. a name that needs escaping (a quote, a backslash) still yields JSON that parses.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
git config user.email a@b.c; git config user.name a
"$MJ" init >/dev/null

# 1. the same command, both forms
# doctor may report findings in a fresh repository; its exit is not what this case is about
MJ_TIMING=1 "$MJ" doctor > "$T/out.text" 2> "$T/err.text" || :
MJ_TIMING=1 "$MJ" --json doctor > "$T/out.json" 2> "$T/err.json" || :
line="$(grep '^{"timing":' "$T/err.json" | tail -n 1)"
[ -n "$line" ] || { echo "    --json printed no timing object on stderr:"; sed 's/^/      /' "$T/err.json" | tail -5; exit 1; }
printf '%s\n' "$line" | jq -e '.timing | (.clock | type == "string") and (.total_ms | type == "number")
  and (.phases | type == "array" and length > 0) and (.counters | type == "array")' >/dev/null \
  || { echo "    the timing object does not have the documented shape: $line"; exit 1; }
grep -q '^TIMING clock=' "$T/err.text" || { echo "    the text form lost its header"; exit 1; }
text_phases="$(awk '$1 == "phase" { print $NF }' "$T/err.text" | LC_ALL=C sort)"
json_phases="$(printf '%s\n' "$line" | jq -r '.timing.phases[].name' | LC_ALL=C sort)"
[ "$text_phases" = "$json_phases" ] \
  || { echo "    the two forms name different phases:"; diff <(printf '%s\n' "$text_phases") <(printf '%s\n' "$json_phases") | sed 's/^/      /'; exit 1; }
text_counts="$(awk '$1 == "count" { print $3 "\t" $2 }' "$T/err.text" | LC_ALL=C sort)"
json_counts="$(printf '%s\n' "$line" | jq -r '.timing.counters[] | "\(.name)\t\(.count)"' | LC_ALL=C sort)"
[ "$text_counts" = "$json_counts" ] \
  || { echo "    the two forms carry different counters:"; diff <(printf '%s\n' "$text_counts") <(printf '%s\n' "$json_counts") | sed 's/^/      /'; exit 1; }

# 2. stdout is the command's alone
# The briefing's first line carries the second it was written in, and two runs do not share a
# second on a machine slow enough: the runner's shard took the two for different outputs. Both
# are given one clock (MAJORDOMUS_NOW, which mj_now reads), so the comparison stays byte for
# byte; that the line carries that clock is checked, so that it is the clock that was held.
NOW=2026-01-02T03:04:05Z
MAJORDOMUS_NOW="$NOW" MJ_TIMING=1 "$MJ" context > "$T/ctx.timed" 2>/dev/null
MAJORDOMUS_NOW="$NOW" "$MJ" context > "$T/out.plain" 2>/dev/null
sed -n 1p "$T/out.plain" | grep -q "$NOW" \
  || { echo "    the first line of context does not carry the clock it was given: $(sed -n 1p "$T/out.plain")"; exit 1; }
cmp -s "$T/out.plain" "$T/ctx.timed" \
  || { echo "    the timing report changed what the command printed on stdout:"; diff "$T/out.plain" "$T/ctx.timed" | sed 's/^/      /' | head -10; exit 1; }

# 3. a name that must be escaped
# MJ_JSON is set after the library loads, the way the argument parser sets it for --json
out="$(cd "$T" && MJ_TIMING=1 bash -c '
  . "$ROOT/lib/common.sh"
  MJ_JSON=1
  mj_count "a \"quoted\" name"
  mj_count "a back\\slash"
  mj_timing_report' 2>&1 >/dev/null)"
printf '%s\n' "$out" | tail -n 1 | jq -e '[.timing.counters[].name] | sort == ["a \"quoted\" name", "a back\\slash"]' >/dev/null \
  || { echo "    an escaped name did not round-trip through the JSON form: $out"; exit 1; }

echo "    --json gives the report as one JSON line with every phase and counter of the text form"
