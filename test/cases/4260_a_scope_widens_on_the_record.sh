# majordomus-covers: start check
# A task's scope widens on the record (I4260). An adopter's work grew into a lock file and a
# new directory four times in one session, and each time the only way past check and the
# pre-push finish --check was finish --outcome partial and a second start.
#
#   start --widen adds the paths, and check stops failing on them;
#   the ledger carries task.scope_widened with the paths and the reason;
#   a path already in scope adds nothing; --because is required; no active task is refused;
#   a path start would refuse (absolute, escaping) is refused here too.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p src && echo a > src/a && git add . && git commit -qm base

expect_exit 12 "$MJ" start --widen lib --because "no task yet"
expect_exit 0 "$MJ" start "grow" --scope src
mkdir -p lib && echo b > lib/b && echo c > src/a
expect_exit 10 "$MJ" check
expect_grep 'lib/ — outside claimed scope'

expect_exit 2 "$MJ" start --widen lib
expect_grep 'because'
expect_exit 2 "$MJ" start --widen /etc --because "escape"
expect_exit 0 "$MJ" start --widen lib,src --because "the collector needs a helper module"
expect_grep 'widened t-.*: \+ lib \(because: the collector needs a helper module\)'
expect_no_grep '\+ lib src'
grep -q '"event":"task.scope_widened".*"added":"lib".*"because":"the collector needs a helper module"' .ai/local/state/ledger.jsonl \
  || { echo "    the ledger has no task.scope_widened line"; tail -3 .ai/local/state/ledger.jsonl; exit 1; }
expect_exit 0 "$MJ" check
expect_exit 0 "$MJ" start --widen lib --because "again"
expect_grep 'nothing new'
echo "ok: the scope widens on the record and check judges the widened scope"
