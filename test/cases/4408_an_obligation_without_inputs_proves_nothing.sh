# majordomus-covers: evidence
# majordomus-negative: evidence check finish
# An empty input set is never a freshness proof (I4434). The `tests` obligation is taken over
# this repository's own directories (lib/**, test/cases/**, ...), so in an adopter's
# repository, whose code lives elsewhere, its pathspecs select no tracked file. Evidence was
# then recorded over the empty hash, the tree still hashed to it, and `check` said
# "OK obligation tests — discharged over inputs " with nothing after the word: any command
# discharged it, and editing the code afterwards never made it stale (F11, measured
# 2026-10-30 with 1.0.0-alpha.8 on a stranger fixture, at origin/master 51b38035a71).
#
# This case builds that stranger: a python package and its unittest, no use cases, nothing
# the vocabulary's pathspecs name. It proves that both judges — mj_obligation_judge, which
# `check` and `finish` run, and obligations.closure, which gates.completion reads — answer
# "not judged" there and that `finish --outcome completed` refuses with a reproduce line, and
# that the ordinary judgement comes back the moment the pathspecs select a file: evidence
# over "" is then stale, evidence over the selection discharges, and an edit makes it stale.
#
# The last section is the same rule for a token that declares no inputs at all (`commit`):
# evidence recorded for it over a dirty tree is owed in obligations.closure, never
# "discharged over inputs " with nothing after the word, while `check` establishes it live
# from git and says the same tree is not committed. Neither judge passes it.
# claims: obligation-closure
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip_case "jq is absent, so obligations.closure was not read"
BIN="$(rust_bin)" || rust_bin_exit $?

"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p calc tests
printf 'def mean(xs):\n    return sum(xs) / len(xs)\n' > calc/__init__.py
printf 'import unittest\nimport calc\n\n\nclass T(unittest.TestCase):\n    def test_mean(self):\n        self.assertEqual(calc.mean([1, 2, 3]), 2)\n' > tests/test_calc.py
git add -A >/dev/null && git commit -qm 'a stranger'

# the stranger has no share/ of its own: the vocabulary is the tool's, as an installed one's is
closure() {
  "$BIN" run obligations.closure --input '{}' --quiet --format json --repo . \
    --share "$ROOT/share" 2>/dev/null \
    | jq -c --arg id "${1:-tests}" '.output.obligations[] | select(.id == $id)'
}

expect_exit 0 "$MJ" start "add median()" --scope calc,tests,lib --requires tests
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'python3 -m unittest -q'
expect_grep 'evidence: tests recorded'

# ---------------------------------------------------------------- the defect, gone
# the selection really is empty here, which is the precondition of everything below
[ -z "$(git ls-files -- 'lib/**' 'bin/majordomus' 'share/**' 'apps/majordomus-cli/src/**' 'test/cases/**')" ] \
  || { echo "    the fixture tracks a file the tests pathspecs select; it is not a stranger"; exit 1; }
expect_exit 0 "$MJ" check
expect_no_grep 'OK +obligation +tests'
expect_grep 'obligation +tests — not judged: its inputs \(lib/\*\* .*\) select no tracked file in this tree.*\(not refused'

printf 'def median(xs):\n    return sorted(xs)[len(xs) // 2]\n' >> calc/__init__.py
git add -A >/dev/null && git commit -qm 'median'
expect_exit 0 "$MJ" check
expect_no_grep 'OK +obligation +tests'
expect_exit 10 "$MJ" finish --outcome completed --note "done" --verify-command 'python3 -m unittest -q'
expect_grep "FAIL obligation +tests — not judged: .*\[reproduce: git ls-files -- 'lib/\*\*' 'bin/majordomus'"
expect_no_grep 'discharged over inputs'

# the executable's judge says the same word over the same selection
c="$(closure)"
[ "$(printf '%s' "$c" | jq -r .state)" = owed ] || { echo "    obligations.closure: $c"; exit 1; }
printf '%s' "$c" | jq -e '(.staleness == null) and (.evidence != null) and (.detail | startswith("not judged: its inputs (lib/**"))
  and (.reproduce | startswith("git ls-files -- '"'"'lib/**'"'"'"))' >/dev/null \
  || { echo "    obligations.closure did not report the empty selection as not judged: $c"; exit 1; }

# ---------------------------------------------------------------- the positive path
# once a pathspec selects a tracked file, the evidence over "" is stale, not owed ...
mkdir -p lib && printf 'mean() { :; }\n' > lib/calc.sh
git add -A >/dev/null && git commit -qm 'a file the obligation names'
expect_exit 0 "$MJ" check
expect_grep 'obligation +tests — the evidence was taken over inputs  ?and this tree hashes to [0-9a-f]{12}'
[ "$(closure | jq -r .state)" = stale ] || { echo "    obligations.closure: $(closure)"; exit 1; }

# ... evidence taken over the selection discharges it, in both judges ...
expect_exit 0 "$MJ" evidence --covers tests --type test --command 'python3 -m unittest -q'
expect_exit 0 "$MJ" check
expect_grep 'OK +obligation +tests — discharged over inputs [0-9a-f]{12}'
[ "$(closure | jq -r .state)" = discharged ] || { echo "    obligations.closure: $(closure)"; exit 1; }

# ... and an edit after it makes it stale, so finish refuses it as stale
echo 'median() { :; }' >> lib/calc.sh
git add -A >/dev/null && git commit -qm 'edit'
expect_exit 10 "$MJ" finish --outcome completed --note "done" --verify-command 'python3 -m unittest -q'
expect_grep 'FAIL obligation +tests — the evidence was taken over inputs [0-9a-f]{12} .* it no longer describes what it proved'
[ "$(closure | jq -r .state)" = stale ] || { echo "    obligations.closure: $(closure)"; exit 1; }

# ---------------------------------------------------------------- no inputs at all
# `commit` declares no pathspecs, so its evidence and the tree hash to nothing whatever the
# tree holds: a record of it is never a freshness proof, in obligations.closure as in check
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4408.XXXXXX")"; trap 'rm -rf "$S"' EXIT
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"
expect_exit 0 "$MJ" finish --outcome blocked --note "$S/note.md"
expect_exit 0 "$MJ" start "commit the median" --scope lib --requires commit
echo 'mode() { :; }' >> lib/calc.sh
expect_exit 0 "$MJ" evidence --covers commit --command git
expect_exit 0 "$MJ" check
expect_no_grep 'OK +obligation +commit'
expect_grep 'obligation +commit'
c="$(closure commit)"
printf '%s' "$c" | jq -e '(.state == "owed") and (.staleness == null) and (.evidence != null)
  and (.detail | startswith("not judged: '"'"'commit'"'"' declares no inputs"))
  and (.detail | contains("discharged") | not) and (.reproduce == "majordomus check")' >/dev/null \
  || { echo "    obligations.closure discharged a token without inputs over nothing: $c"; exit 1; }
expect_exit 10 "$MJ" finish --outcome completed --note "done" --verify-command 'python3 -m unittest -q'
expect_grep 'FAIL obligation +commit'
expect_no_grep 'discharged over inputs'
