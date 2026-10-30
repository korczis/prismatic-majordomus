# majordomus-covers: none
# The cluster page of the Dashboard Suite (ADR 0159) through the built executable: one
# reading, drawn once as console text, and the numbers on it are its sources' own.
#
#   1. `majordomus dashboard cluster --format json` answers the page's shape — an origin,
#      machines, milestones, the three queue lines, the notes and the lines of console text —
#      and exits 0: a page that could not read a source says so in `notes` and is still a page;
#   2. the milestone tally is `plan.status`'s, counted a second time here with jq (an oracle
#      that called the implementation would agree with any mistake in it): the page's
#      `milestones_total` is the plan's milestones and `milestones_done` the ones it calls DONE;
#   3. the width is the caller's: no line is wider than the width asked, a width below the
#      floor is the floor, and one above the ceiling is the ceiling;
#   4. the text format prints the page's own lines, byte for byte, so that the terminal and
#      the /ui panel (which draws `lines` as they are) are one text;
#   5. a subcommand the suite does not declare is a usage error (2), not an empty page.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

R="$T/repo"; mkdir -p "$R"
cp -R "$ROOT/share/skeleton/ai" "$R/.ai"
( cd "$R" && git init -q . && git add -A >/dev/null 2>&1 \
  && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null 2>&1 ) \
  || { echo "    could not make a fixture repository"; exit 1; }

# --- 1. the shape, and the exit code
( cd "$R" && "$RB" dashboard cluster --format json ) > page.json 2> err.txt && code=0 || code=$?
[ "$code" = 0 ] || { echo "    dashboard cluster exited $code"; cat err.txt; exit 1; }
jq -e '
  (.at | type == "string") and (.origin | type == "string")
  and (.machines | type == "array") and (.milestones | type == "array")
  and (.notes | type == "array")
  and (.lines | type == "array" and length > 0 and all(type == "string"))
  and has("integration") and has("derive") and has("mesh")
  and (.milestones_total | type == "number") and (.milestones_done | type == "number")
' page.json >/dev/null || { echo "    the page has not the declared shape:"; jq -c 'keys' page.json; exit 1; }

# --- 2. the tally is the plan's own
( cd "$R" && "$RB" run plan.status --input '{}' --format json --quiet ) > plan.json 2>/dev/null \
  || { echo "    could not ask plan.status for the oracle"; exit 1; }
# a source omits an empty list (skip_serializing_if): absence is its way of saying none
want_total="$(jq '(.milestones // []) | length' plan.json)"
want_done="$(jq '[(.milestones // [])[] | select(.status == "DONE")] | length' plan.json)"
got_total="$(jq .milestones_total page.json)"; got_done="$(jq .milestones_done page.json)"
[ "$got_total" = "$want_total" ] && [ "$got_done" = "$want_done" ] \
  || { echo "    the page says $got_done of $got_total milestones done; plan.status says $want_done of $want_total"; exit 1; }

# --- 3. the width is the caller's, within a floor and a ceiling
# characters, not bytes: the page draws box characters, which awk on some systems counts as three
widest() { jq '[.lines[] | length] | max // 0' "$1"; }
( cd "$R" && "$RB" dashboard cluster --width 60 --format json ) > w60.json 2>/dev/null
[ "$(widest w60.json)" -le 60 ] || { echo "    a line is wider than the 60 asked ($(widest w60.json))"; exit 1; }
( cd "$R" && "$RB" dashboard cluster --width 10 --format json ) > w10.json 2>/dev/null
[ "$(widest w10.json)" -le 40 ] || { echo "    a width below the floor was not raised to it ($(widest w10.json))"; exit 1; }
( cd "$R" && "$RB" dashboard cluster --width 9999 --format json ) > w9999.json 2>/dev/null
[ "$(widest w9999.json)" -le 200 ] || { echo "    a width above the ceiling was not lowered to it ($(widest w9999.json))"; exit 1; }

# --- 4. the text format is the page's own lines
( cd "$R" && "$RB" dashboard cluster --width 70 ) > text.txt 2>/dev/null
( cd "$R" && "$RB" dashboard cluster --width 70 --format json ) 2>/dev/null | jq -r '.lines[]' > lines.txt
# the page is composed twice, a second apart at most; the timestamp is the one line that may differ
diff <(grep -v " CLUSTER " text.txt) <(grep -v " CLUSTER " lines.txt) >/dev/null \
  || { echo "    the text format is not the lines of the page:"; diff <(grep -v " CLUSTER " text.txt) <(grep -v " CLUSTER " lines.txt) | head -6; exit 1; }

# --- 5. an undeclared page is a usage error
( cd "$R" && "$RB" dashboard nonsense ) >/dev/null 2>&1 && code=0 || code=$?
[ "$code" = 2 ] || { echo "    an undeclared page exited $code, not 2"; exit 1; }
echo "    cluster page: shape, plan tally, width bounds, text = lines, usage error"
