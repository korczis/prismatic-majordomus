# majordomus-covers: scope check finish
# majordomus-negative: scope
# claims: scope-enforcement
# A task's scope can be widened while it runs (I2114, the second half). Work finds what a
# scope did not foresee — a gate that wants a roadmap line, an advisory that wants a lock
# bumped — and until `scope widen` the only route was to close the task and start another.
#
# What it must be:
#
#   in place. The task keeps its id; the paths join its scope; the record says who widened
#   it, when and why, and the ledger keeps every widening.
#
#   explained. A widening without --because is refused: a scope that grows silently is the
#   creep the scope exists to show.
#
#   the same admission as start. A path start --scope would refuse, widen refuses.
#
# What it must not become: a way to widen nothing, or someone else's task.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib docs other
echo a > lib/a; echo d > docs/d.md; echo o > other/o; echo r > README.md
git add -A >/dev/null; git commit -qm base

# ---------------------------------------------------------------- no task: nothing to widen
expect_exit 12 "$MJ" scope widen docs --because "x"
expect_grep 'no task here to widen'

"$MJ" start "t" --scope lib >/dev/null
CUR=".ai/local/state/current.yaml"
ID="$(sed -n 's/^id: //p' "$CUR")"

# ---------------------------------------------------------------- a file outside the scope is refused
echo b > lib/a; echo r2 > README.md
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +README.md — outside claimed scope \(lib\)'

# ---------------------------------------------------------------- refused: no reason, no path, a bad path
expect_exit 2 "$MJ" scope widen README.md
expect_grep '--because is required'
expect_exit 2 "$MJ" scope widen README.md --because "   "
expect_grep '--because is required'
expect_exit 2 "$MJ" scope widen --because "why"
expect_grep 'name the paths to add'
expect_exit 2 "$MJ" scope widen "a b" --because "why"
expect_grep "scope entry 'a b' holds whitespace"
expect_exit 2 "$MJ" scope widen ../outside --because "why"
expect_grep 'escapes the repository'
grep -q 'scope_widened' "$CUR" && { echo "    a refused widening wrote the record"; exit 1; }

# ---------------------------------------------------------------- widened in place
expect_exit 0 "$MJ" scope widen README.md,docs --because "the roadmap gate wants a line" --by alice
expect_grep "^widened $ID  added=README.md,docs  scope=lib,README.md,docs$"
[ "$(sed -n 's/^id: //p' "$CUR")" = "$ID" ] || { echo "    the task id changed"; exit 1; }
[ "$(sed -n 's/^outcome: //p' "$CUR")" = active ] || { echo "    the task is no longer active"; exit 1; }
expect_grep '^  - lib$' "$CUR"
expect_grep '^  - README.md$' "$CUR"
expect_grep '^  - docs$' "$CUR"
expect_grep '^scope_widened_by: "alice"$' "$CUR"
expect_grep '^scope_widened_because: "the roadmap gate wants a line"$' "$CUR"
expect_grep '^scope_widened_at: [0-9]{4}-' "$CUR"
expect_grep "\"event\":\"task.scope_widened\".*\"task_id\":\"$ID\".*\"scope\":\"README.md docs\".*\"widened_by\":\"alice\".*\"because\":\"the roadmap gate wants a line\"" .ai/local/state/ledger.jsonl
expect_exit 0 "$MJ" check
expect_no_grep 'outside claimed scope'
expect_no_grep 'unknown key'

# ---------------------------------------------------------------- what the scope holds is left out; nothing new writes nothing
cp "$CUR" "$CUR.before"
expect_exit 0 "$MJ" scope widen docs/d.md lib --because "again"
expect_grep "^unchanged $ID  nothing to add"
expect_grep 'already within the scope'
cmp -s "$CUR" "$CUR.before" || { echo "    a widening that added nothing rewrote the record"; exit 1; }
rm -f "$CUR.before"

# ---------------------------------------------------------------- a second widening: provenance replaced, ledger keeps both
expect_exit 0 "$MJ" scope widen other --because "an advisory wants the lock bumped"
[ "$(grep -c '^scope_widened_because: ' "$CUR")" = 1 ] || { echo "    the record carries two widenings"; exit 1; }
expect_grep '^scope_widened_because: "an advisory wants the lock bumped"$' "$CUR"
[ "$(grep -c '"event":"task.scope_widened"' .ai/local/state/ledger.jsonl)" = 2 ] \
  || { echo "    the ledger does not keep both widenings"; exit 1; }

# ---------------------------------------------------------------- finish accepts the widened scope
echo o2 > other/o
git add -A >/dev/null; git commit -qm "the work, wider than first said" >/dev/null
expect_exit 0 "$MJ" check
expect_no_grep 'outside claimed scope'

# ---------------------------------------------------------------- only an active task is widened
"$MJ" handover --close <<'N' >/dev/null 2>&1 || true
# Objective
o
# Current State
s
# Next Action
n
N
if [ "$(sed -n 's/^outcome: //p' "$CUR" 2>/dev/null)" != active ]; then
  expect_exit 12 "$MJ" scope widen more --because "late"
fi
