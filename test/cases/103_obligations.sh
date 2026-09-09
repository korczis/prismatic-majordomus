# majordomus-covers: evidence
# majordomus-negative: evidence check finish
# A task owes what its `requires` declares, and evidence discharges an obligation only
# while it still describes the tree it was taken over. This case starts a task that owes
# something, watches `finish --outcome completed` refuse it while nothing is recorded,
# records evidence and watches the same command accept it, then changes a file the
# obligation names and watches the evidence go stale rather than stay true. It also proves
# the honest cases: a token nobody declared is refused, narrative is refused, and an
# outcome that is not completed is never refused for owing something.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj103.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base

# the task record carries `requires` beside `scope`; start writes the scope, and the
# obligations are added the way a caller would add them
owes() {
  python3 - "$@" <<'PY'
import io, sys
p = '.ai/local/state/current.yaml'
s = io.open(p, encoding='utf-8').read()
block = 'requires:\n' + ''.join('  - %s\n' % t for t in sys.argv[1:])
s = s.replace('outcome:', block + 'outcome:')
io.open(p, 'w', encoding='utf-8').write(s)
PY
}

# ---------------------------------------------------------------- no task owes nothing
expect_exit 2 "$MJ" evidence
expect_grep 'usage: majordomus evidence'
expect_exit 12 "$MJ" evidence --covers tests --command 'bash test/run.sh'
expect_grep 'no active task'

# ---------------------------------------------------------------- a task that owes tests
expect_exit 0 "$MJ" start "obligations" --scope lib/
owes tests
# a token the vocabulary does not know
expect_exit 2 "$MJ" evidence --covers nonsense --command x
expect_grep "no obligation 'nonsense'"
# a token the vocabulary knows but this task never promised
expect_exit 15 "$MJ" evidence --covers docs --command x
expect_grep "the active task does not require 'docs'"
# narrative is not evidence
expect_exit 2 "$MJ" evidence --covers tests
expect_grep 'narrative is not evidence'

# ---------------------------------------------------------------- owed and unproved
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'FAIL obligation +tests — owed, and no evidence was recorded'
# an honest outcome is never refused for owing something
expect_exit 0 "$MJ" check
expect_grep 'obligation +tests — owed, and no evidence was recorded \(not refused'

# ---------------------------------------------------------------- recorded, then accepted
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'bash test/run.sh'
expect_grep 'evidence: tests recorded'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  tests — discharged over inputs'

# ---------------------------------------------------------------- the point: staleness
echo 'changed' >> lib/a
git add -A >/dev/null && git commit -qm change
expect_exit 10 "$MJ" finish --outcome completed --note "done"
expect_grep 'it no longer describes what it proved'
# recording it again over the new tree discharges it
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'bash test/run.sh'
expect_exit 0 "$MJ" check
expect_grep 'OK   obligation  tests — discharged over inputs'

# ---------------------------------------------------------------- the vocabulary is data
expect_exit 0 "$MJ" doctrine show majordomus.obligation-closure
expect_grep 'validator.*obligations'
