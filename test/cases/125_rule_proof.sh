# majordomus-covers: rules
# majordomus-negative: rules
# A rule names what proves it, and what it names is in the tree.
#
# Two halves, and they fail differently. The loader half is about the rule object: an
# enforcement block is valid in one of two modes, and a block that is neither is refused
# before anything reads it. The gate half is about the relation the block creates: a rule
# that names a case which no longer exists reads as proven and is not, and that is the
# one-way reference this repository keeps rediscovering.
#
# Every guarantee below is proved by a mutation: the tree is green, one fact changes, the
# thing that should refuse it goes red naming that fact, the change is undone, the tree is
# green again. A mutation nothing survives is a guarantee nothing gives.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
P=.ai/repo/rules/project
CHECK="$ROOT/scripts/ci/rule-proof-check"
BASE=.ai/repo/rule-proof-baseline.txt
[ -x "$CHECK" ] || { echo "    scripts/ci/rule-proof-check is not executable"; exit 1; }
# the gate measures whatever tree MJ_ROOT names; the fixture is this one
export MJ_ROOT="$PWD"
# The fixture stands in for a tree whose cases exist: every path the vendored rules name is
# materialised here, so that the only dangling reference below is the one this case makes.
# Without this the fixture would fail for the vendored baseline's sake and prove nothing
# about the mutation.
mkdir -p test/cases && : > test/cases/01_fixture.sh
grep -rhoE 'test/cases/[0-9]+_[a-z0-9_]+\.sh' .ai/repo/rules | LC_ALL=C sort -u | while read -r c; do : > "$c"; done

# prule FILE ID CLASS [BLOCK-LINE ...] — a valid project rule, with an optional
# x-majordomus block written verbatim from the remaining arguments
prule() {
  local f="$1" id="$2" cls="$3"; shift 3
  { cat <<Y
---
id: $id
version: 1
kind: rule
title: Rule $id
description: What $id requires, in one sentence.
statement: The normative sentence $id asks a worker to follow.
status: active
class: $cls
depends_on: []
tags: [fixture]
Y
    if [ $# -gt 0 ]; then printf '\nx-majordomus:\n'; printf '  %s\n' "$@"; fi
    printf -- '---\n\n# Rationale\n\nA fixture.\n\n# Required behaviour\n\nThe fixture holds.\n\n# Failure behaviour\n\nThe fixture is reported.\n\n# Verification\n\nThis case.\n'
  } > "$P/$f"
}
clean() { rm -f "$P"/fixture-*.md; }

# ---------------------------------------------------------------- the loader: two modes
# A gated rule — tests, no validator — is a rule the loader accepts.
prule fixture-gated.v1.md project.fixture-gated blocking 'tests: [test/cases/01_fixture.sh]'
expect_exit 0 "$MJ" rules list
expect_grep 'project\.fixture-gated .* proven by test/cases/01_fixture\.sh'

# ...and one the dispatcher never calls. It names no validator, so a registry that took it
# would report a validator it cannot find; the count is the evidence that it did not.
expect_exit 0 "$MJ" doctrine status
expect_grep 'missing validators: +0'
before="$(printf '%s' "$LAST_OUT" | awk '/declared doctrines:/ { print $NF }')"
[ -n "$before" ] || { echo "    doctrine status printed no count"; exit 1; }

# The two modes are the only two. A block that names where it runs but not what runs is a
# half-declaration, and the loader says which half is missing.
prule fixture-half.v1.md project.fixture-half blocking 'enforced_by: [check]' 'tests: [test/cases/01_fixture.sh]'
expect_exit 10 "$MJ" rules list
expect_grep 'names an enforcing command but no validator'
rm -f "$P/fixture-half.v1.md"

# A validator's category without the validator is the same half-declaration.
prule fixture-cat.v1.md project.fixture-cat blocking 'category: fixture' 'tests: [test/cases/01_fixture.sh]'
expect_exit 10 "$MJ" rules list
expect_grep 'names a category or exit code but no validator'
rm -f "$P/fixture-cat.v1.md"

# Either executable mode names a test. A block that names none, and gives no reason why it
# cannot, is enforcement nobody can reproduce.
prule fixture-untested.v1.md project.fixture-untested blocking 'claims: [fixture]'
expect_exit 10 "$MJ" rules list
expect_grep 'x-majordomus names no test and gives no reviewed_because'
rm -f "$P/fixture-untested.v1.md"

# ---------------------------------------------------------------- the third mode: review
# Some rules have no machine expression — the provenance of what was written, whether a
# mechanism earns its cost. Those may say so, and the reason is the whole declaration:
# there is no flag beside it, because a flag can be set and a reason cannot be set without
# writing one. What that buys is that an exemption is a sentence somebody had to defend.
prule fixture-rev.v1.md project.fixture-rev blocking \
  'reviewed_because: the rule is about the provenance of code, which no program in this tree can read'
expect_exit 0 "$MJ" rules list
expect_grep 'project\.fixture-rev .* review-enforced: the rule is about the provenance'
expect_exit 0 "$MJ" rules list --json
expect_grep '"id":"project\.fixture-rev".*"mode":"reviewed"'

# It is a third mode, not a way out of the first two: a rule cannot be dispatched and
# review-enforced at once, and the loader says which claim contradicts which.
prule fixture-revbad.v1.md project.fixture-revbad blocking 'validator: fixture' 'category: f' \
  'exit_code: 12' 'enforced_by: [check]' 'reviewed_because: a reason it has no business having'
expect_exit 10 "$MJ" rules list
expect_grep 'a dispatched rule is not review-enforced'
rm -f "$P/fixture-revbad.v1.md"

# The gate reports it on every run, as a note and never as a failure, and counts it apart:
# this number going up is governance getting weaker, and a summary that folded it into a
# total would hide exactly that.
expect_exit 0 "$CHECK" --strict
expect_grep 'NOTE rule-proof reviewed .*project\.fixture-rev'
expect_grep 'declares that a reader enforces it, with its reason'

# And an executable proof always wins, so a rule that acquires a case stops being
# review-enforced without anyone remembering to delete the declaration.
prule fixture-rev.v1.md project.fixture-rev blocking \
  'reviewed_because: the rule is about the provenance of code, which no program in this tree can read' \
  'tests: [test/cases/01_fixture.sh]'
expect_exit 0 "$MJ" rules list
expect_grep 'project\.fixture-rev .* proven by test/cases/01_fixture\.sh'
expect_exit 0 "$CHECK" --strict
expect_no_grep 'reviewed .*project\.fixture-rev'
rm -f "$P/fixture-rev.v1.md"
expect_exit 0 "$MJ" rules list

# A dispatched rule still needs all four; the second mode did not loosen the first.
prule fixture-disp.v1.md project.fixture-disp blocking 'validator: fixture' 'tests: [test/cases/01_fixture.sh]'
expect_exit 10 "$MJ" rules list
expect_grep 'x-majordomus lacks category'
rm -f "$P/fixture-disp.v1.md"
expect_exit 0 "$MJ" rules list

# ---------------------------------------------------------------- the gate: the relation
# The fixture tree's own debt is whatever it is; record it, so that the assertions below
# are about what this case changes and not about what it inherited.
expect_exit 0 "$CHECK" --write-baseline
expect_exit 0 "$CHECK"

# A proof that is not in the tree is a failure, and it is not ratcheted: the rule reads as
# proven. This is the mutation the whole gate exists for.
prule fixture-dangling.v1.md project.fixture-dangling blocking 'tests: [test/cases/99_no_such_case.sh]'
expect_exit 10 "$CHECK"
expect_grep 'FAIL rule-proof dangling .*project\.fixture-dangling'
expect_grep 'names test/cases/99_no_such_case\.sh, which is not in the tree'
# ...and recording it in the baseline does not silence it, because the baseline is about
# rules with no proof, not about rules whose proof is a lie
printf 'project.fixture-dangling\n' >> "$BASE"
expect_exit 10 "$CHECK"
expect_grep 'FAIL rule-proof dangling'
sed '/fixture-dangling/d' "$BASE" > "$T/b" && mv "$T/b" "$BASE"
rm -f "$P/fixture-dangling.v1.md"
expect_exit 0 "$CHECK"

# A new blocking rule with no proof at all is new debt, and the gate names it and says how
# to discharge it.
prule fixture-unproven.v1.md project.fixture-unproven blocking
expect_exit 10 "$CHECK"
expect_grep 'FAIL rule-proof unproven .*project\.fixture-unproven'
expect_grep 'new debt, not in'
expect_grep 'tests: \[test/cases/NN_name\.sh\]'

# The ratchet: debt that is recorded blocks nobody...
expect_exit 0 "$CHECK" --write-baseline
expect_exit 0 "$CHECK"
expect_grep 'no new debt'
# ...but --strict ignores the baseline, which is how the debt is measured rather than kept.
expect_exit 10 "$CHECK" --strict
expect_grep 'blocking rule\(s\) name no proof \(--strict\)'

# Discharging debt is reported, so a baseline cannot quietly outlive what it excused.
prule fixture-unproven.v1.md project.fixture-unproven blocking 'tests: [test/cases/01_fixture.sh]'
expect_exit 0 "$CHECK"
expect_grep 'debt cleared, run --write-baseline'
expect_grep 'project\.fixture-unproven'

# An advisory rule is not held to it: the class is what makes proof required.
prule fixture-advisory.v1.md project.fixture-advisory advisory
expect_exit 0 "$CHECK"
expect_no_grep 'project\.fixture-advisory'

# The healthy line states two counts and nothing else. Under `set -o pipefail` grep -c
# both prints its zero and exits non-zero, so the guard that used to follow it printed a
# second zero and the clean case read "no new debt (0\n0 rule(s) known".
# The middle count is the review-enforced tally, stated on the healthy line so that a
# repository leaning harder on review shows that number rather than absorbing it in a total.
clean
expect_exit 0 "$CHECK"
expect_grep 'no new debt \([0-9]+ rule\(s\) known, [0-9]+ declared review-enforced, [0-9]+ rule\(s\) measured\)$'
expect_exit 0 "$CHECK" --write-baseline
expect_exit 0 "$MJ" rules list
