# majordomus-covers: finish
# majordomus-negative: finish
#
# A verification keeps what it said. `finish --verify-command` sent the command's stdout and
# stderr to /dev/null, so a refusal read `<cmd> — exit 1, 412s` and nothing else: the worker
# who was refused had nothing to read, and the integrator of korczis/algoglyph wrapped every
# verify as `bash -c 'set -o pipefail; npm run verify 2>&1 | tee <log>'` to get it back
# (I3123). The output now goes to a log under .ai/local/state/verify/, one per run:
#   - a refused run prints the log's last lines, under a line naming the log, after the
#     refusal, and its task.refused line carries the verify object with the log;
#   - an accepted run names its log, and its task.finished verify object does too;
#   - the contract lines are unchanged: they are what README.md quotes from a recorded run;
#   - two runs of one task never share a log: the log's name carries the attempt;
#   - the ledger carries the log's path and size, never the output itself: the ledger is the
#     record other readers replay, and a verify's output is whatever the project printed;
#   - writing the log does not move the tree the run is judged over (.ai/local/ is ignored);
#   - the directory keeps the newest 20 logs, so a checkout finishing for months does not
#     grow without bound.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null; }
LEDGER=.ai/local/state/ledger.jsonl
VDIR=.ai/local/state/verify

"$MJ" start "t1" --scope lib >/dev/null
id1="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
echo b >> lib/a
note

# A failing verification: its output is kept, named, and its tail is shown.
# printf builds what it prints, so the output's words are not the command's: the ledger
# records the command, and must not record the output.
cmd="printf 'first-%s\\n' line; printf 'said-on-%s\\n' stdout; printf 'said-on-%s\\n' stderr >&2; exit 3"
expect_exit 10 "$MJ" finish --outcome completed --verify-command "$cmd"
expect_grep "^FAIL verification $id1 — .* — exit 3, [0-9]+s  \\[reproduce: "
expect_grep 'finish: refused'
expect_grep "^verify output, last 3 of 3 line\\(s\\) of $VDIR/[0-9]{8}T[0-9]{6}Z--$id1--1\\.log:\$"
expect_grep '^  said-on-stdout$'
expect_grep '^  said-on-stderr$'
log1="$(ls "$VDIR"/*--"$id1"--*.log)"
expect_file "$log1"
expect_grep '^said-on-stdout$' "$log1"
expect_grep '^said-on-stderr$' "$log1"
expect_grep "\"event\":\"task.refused\".*\"task_id\":\"$id1\".*\"verify\":\\{\"command\":\"[^\"]*\",\"exit\":3,\"seconds\":[0-9]+,\"log\":\"$log1\",\"log_bytes\":[0-9]+,\"log_lines\":3\\}" "$LEDGER"
expect_no_grep 'said-on-stdout|said-on-stderr' "$LEDGER"

# An accepted verification names its log as well, and the log did not move the tree.
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai && echo all-green"
expect_grep "OK +verification $id1 — .* — exit 0, [0-9]+s, tree [0-9a-f]{12}\$"
expect_grep "^verify output: $VDIR/[0-9]{8}T[0-9]{6}Z--$id1--2\\.log, 1 line\\(s\\)\$"
expect_no_grep '^verify output, last'
expect_grep "\"event\":\"task.finished\".*\"task_id\":\"$id1\".*\"verify\":\\{\"command\":\"test -d \\.ai && echo all-green\",\"exit\":0,\"seconds\":[0-9]+,\"tree\":\"[0-9a-f]{40}\",\"log\":\"$VDIR/[^\"]+\",\"log_bytes\":10,\"log_lines\":1\\}" "$LEDGER"
expect_grep '^all-green$' "$(ls "$VDIR"/*--"$id1"--2.log)"
expect_grep '^said-on-stdout$' "$log1"

# A tree that moved under the run is refused as before, and its output is still kept.
git add -A && git commit -qm t1
"$MJ" start "t2" --scope lib >/dev/null
id2="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
echo c >> lib/a
note
expect_exit 10 "$MJ" finish --outcome completed --verify-command "test -d .ai && echo moving && echo x >> lib/a"
expect_grep "FAIL verification .*the tree changed while it ran .*the run describes neither state  \\[reproduce: "
expect_grep "^verify output: $VDIR/[0-9TZ]+--$id2--1\\.log, 1 line\\(s\\)\$"

# Only the newest 20 logs stay: 25 older ones are pruned by the next run, which is kept.
i=0; while [ "$i" -lt 25 ]; do
  printf 'old\n' > "$VDIR/20000101T0000$(printf '%02d' "$i")Z--t-old--1.log"; i=$((i+1))
done
expect_exit 10 "$MJ" finish --outcome completed --verify-command "test -d .ai && echo pruned-run && false"
kept="$(find "$VDIR" -name '*.log' | wc -l | tr -d ' ')"
[ "$kept" = 20 ] || { printf '    %s verify logs kept, not 20\n' "$kept"; exit 1; }
grep -lx 'pruned-run' "$VDIR"/*.log >/dev/null \
  || { printf '    the newest run'"'"'s log was pruned\n'; exit 1; }
[ ! -e "$VDIR/20000101T000000Z--t-old--1.log" ] || { printf '    the oldest log was kept\n'; exit 1; }
