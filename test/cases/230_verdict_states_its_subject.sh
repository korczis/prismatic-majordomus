# majordomus-covers: none
# The gate of project.a-verdict-states-its-subject, driven against fixture trees rather than
# against this checkout. A case that asserted this repository's own blocking-rule count and
# its unproven claims would pass the day somebody edited a baseline in the same commit as the
# debt — which is the thing the gates exist to make visible. What is asserted here is their
# behaviour: what they refuse, what they deliberately do not refuse, that each verdict carries
# the denominator the rule is named for, and that each names the population it is about.
#
# TWO GATES, TWO POPULATIONS. `scripts/ci/rule-proof-check` measures rules;
# `scripts/ci/claim-proof-check` measures claims. They were one script — `enforcement-check`
# — carrying both halves, and its rule half was a weaker duplicate of rule-proof-check: it
# accepted any `x-majordomus:` block as proof, including the `reviewed_because:` of ADR 0048,
# which declares that a *person* enforces the rule. It printed `FIXED project.clean-room` — "a
# promise now has a proof" — over a rule that had gained an exemption and no proof. The half
# was removed rather than repaired, because a second inventory of a property another gate owns
# is the defect this repository exists to remove.
#
# What is left is a boundary, and section 3 below is the case that pins it: the same tree,
# holding one unproven rule and one unproven claim, must be reported by each gate as its own
# finding and by neither as the other's. Before the split, a reader met one list of ids from
# two populations under a remedy paragraph that led with blocking rules, and read a list of
# unproven claims as a list of unproven rules.
#
# The non-refusals carry as much weight as the refusals. These gates are about detectors that
# cannot tell their subject from something that merely looks like it, so a case that only
# proved they fire would leave their own false-positive surface untested.
. "$ROOT/test/lib.sh"
RGATE="$ROOT/scripts/ci/rule-proof-check"
CGATE="$ROOT/scripts/ci/claim-proof-check"
[ -x "$RGATE" ] || { echo "    $RGATE is not executable"; exit 1; }
[ -x "$CGATE" ] || { echo "    $CGATE is not executable"; exit 1; }

# rule <dir> <file> <id> <class> [x-majordomus line ...] — a rule object whose enforcement
# block is written verbatim from the remaining arguments, and whose body deliberately quotes
# the rule's own subject.
rule() {
  local d="$1" f="$2" id="$3" cls="$4"; shift 4
  local p="$d/.ai/repo/rules/project/$f.v1.md"
  mkdir -p "$(dirname "$p")"
  { printf -- '---\nid: %s\nversion: 1\nkind: rule\ntitle: t\ndescription: d\nstatement: s\nstatus: active\nclass: %s\ndepends_on: []\ntags: []\n' "$id" "$cls"
    if [ $# -gt 0 ]; then printf '\nx-majordomus:\n'; printf '  %s\n' "$@"; fi
    printf -- '---\n\n# Rationale\n\nr\n\n# Required behaviour\n\nb\n\n# Verification\n\nThis case.\n'
  } > "$p"
}

# claim <dir> <id> <status> <test-path>
# The claims file is created by the first claim, never by the fixture: a tree with no
# docs/CLAIMS.yaml is a tree where the claim gate does not apply (an adopter owes none of
# ours), and that must be distinguishable from a claims file with nothing in it.
claim() {
  if [ ! -f "$1/docs/CLAIMS.yaml" ]; then printf 'version: 1\nclaims:\n' > "$1/docs/CLAIMS.yaml"; fi
  printf '  - id: %s\n    claim: c\n    source: docs/X.md\n    implementation: lib/x.sh\n    test: %s\n    status: %s\n' "$2" "$4" "$3" >> "$1/docs/CLAIMS.yaml"
}

fixture() {
  local f="$T/tree$1"
  rm -rf "$f"
  mkdir -p "$f/.ai/repo/rules/project" "$f/test/cases" "$f/scripts/ci" "$f/docs"
  printf '#!/bin/sh\necho ok\n' > "$f/test/cases/42_named_by_path.sh"
  printf '%s' "$f"
}

# ---------------------------------------------------------------- 1. the rule gate
# Two rules that are proven, each by a different mode, and one advisory rule with no proof at
# all — advisory is outside the population and must not be counted.
F="$(fixture 0)"
rule "$F" dispatched project.dispatched blocking 'validator: d' 'category: c' 'exit_code: 10' \
  'enforced_by: [doctor]' 'tests: [test/cases/42_named_by_path.sh]'
rule "$F" gated      project.gated      blocking 'tests: [test/cases/42_named_by_path.sh]'
rule "$F" advisory-no-proof project.advisory-no-proof advisory
expect_exit 0 env MJ_ROOT="$F" "$RGATE" --strict
expect_grep 'blocking rules: +2 of 3 rule\(s\) in'
expect_grep 'every one of 2 blocking rule\(s\) names what proves it'
# the advisory rule was never in the denominator: 2 blocking rules, not 3
expect_no_grep 'advisory-no-proof'
# and the gate says, on the same run, which population it is not a verdict about
expect_grep '^guaranteed claims: +not measured here'

# --- a body that quotes the rule's own subject does not make the rule its own subject.
# This is lease-reader-check's defect in miniature: the gate reads front matter, never the
# body, so a rule whose prose contains "class: blocking" is classed by its front matter.
p="$F/.ai/repo/rules/project/advisory-no-proof.v1.md"
printf 'A rule that says `class: blocking` in its body is still advisory.\n' >> "$p"
expect_exit 0 env MJ_ROOT="$F" "$RGATE" --strict
expect_grep 'blocking rules: +2 of 3 rule\(s\) in'

# --- an exemption is not a proof. A block carrying only `reviewed_because:` says a person
# enforces the rule, and the gate reports it apart on every run and never as proof. This is
# the regression the removed half had: it read any x-majordomus block as machine-followable
# enforcement and announced the exemption as a promise that had gained one.
rule "$F" reviewed project.reviewed blocking 'reviewed_because: the rule is about a judgement no program here can make'
expect_exit 0 env MJ_ROOT="$F" "$RGATE" --strict
expect_grep 'NOTE rule-proof reviewed +project\.reviewed'
expect_grep 'blocking rules: +3 of 4 rule\(s\) in .*; 1 declare'
expect_no_grep 'FAIL rule-proof'

# --- and a blocking rule with nothing at all is refused
rule "$F" bare project.bare blocking
expect_exit 10 env MJ_ROOT="$F" "$RGATE" --strict
expect_grep 'FAIL rule-proof unproven +project\.bare'

# ---------------------------------------------------------------- 2. the claim gate
# A guaranteed claim whose test does not name it back. The path exists, so the check that
# only verifies existence passes it; the link is one-way and a rewrite breaks it silently.
F="$(fixture 1)"
rule "$F" gated project.gated blocking 'tests: [test/cases/42_named_by_path.sh]'
claim "$F" unproven-claim guaranteed test/cases/42_named_by_path.sh
claim "$F" planned-one    planned     test/cases/42_named_by_path.sh
expect_exit 10 env MJ_ROOT="$F" "$CGATE" --strict
expect_grep 'guaranteed claims: +1 of 1'
expect_grep 'unproven-claim'
# a claim that is not guaranteed is outside the population
expect_no_grep 'planned-one'
# and the claim gate says which population it is not a verdict about
expect_grep '^blocking rules: +not measured here'

# --- the test names the claim back, and the claim is proven enough for this gate to pass
printf '# proves unproven-claim\n' >> "$F/test/cases/42_named_by_path.sh"
expect_exit 0 env MJ_ROOT="$F" "$CGATE" --strict
expect_grep 'guaranteed claims: +0 of 1'
expect_grep 'every guaranteed claim names a test that names it back'

# --- a tree with no claims file is a tree the gate does not apply to, which is not the same
# as a tree it found nothing wrong in
rm -f "$F/docs/CLAIMS.yaml"
expect_exit 0 env MJ_ROOT="$F" "$CGATE" --strict
expect_grep 'not applicable — this tree has no docs/CLAIMS\.yaml'

# ---------------------------------------------------------------- 3. neither is a verdict on the other
# One tree, one unproven rule, one unproven claim. Each gate reports its own finding and
# neither reports the other's, and each says so in words rather than by omission. This is the
# case that would have failed before the split: one script reported both populations as one
# list of ids, so its findings could not be attributed by reading them.
F="$(fixture 2)"
rule "$F" gated   project.gated   blocking 'tests: [test/cases/42_named_by_path.sh]'
rule "$F" no-proof project.no-proof blocking
claim "$F" orphan-claim guaranteed test/cases/42_named_by_path.sh

expect_exit 10 env MJ_ROOT="$F" "$RGATE" --strict
expect_grep 'project\.no-proof'
expect_no_grep 'orphan-claim'
expect_grep '^guaranteed claims: +not measured here'

expect_exit 10 env MJ_ROOT="$F" "$CGATE" --strict
expect_grep 'orphan-claim'
expect_no_grep 'project\.no-proof'
expect_grep '^blocking rules: +not measured here'

# --- and the claim gate's finding says what kind of thing it names, so a list of ids read on
# its own cannot be mistaken for rules. The ratchet: recorded debt blocks nobody, and a claim
# added afterwards is reported as new.
mkdir -p "$F/.ai/repo"
expect_exit 0 env MJ_ROOT="$F" "$CGATE" --write-baseline
expect_grep 'baseline written'
expect_exit 0 env MJ_ROOT="$F" "$CGATE"
expect_grep 'no new guaranteed claim without a proof'
claim "$F" fresh-claim guaranteed test/cases/42_named_by_path.sh
expect_exit 10 env MJ_ROOT="$F" "$CGATE"
expect_grep 'NEW claim fresh-claim'
expect_grep 'a claim id, not a rule id'
expect_no_grep 'NEW claim orphan-claim'
# ...and a claim that gains its back-link is reported as such, so the list cannot rot
printf '# proves orphan-claim\n' >> "$F/test/cases/42_named_by_path.sh"
expect_exit 10 env MJ_ROOT="$F" "$CGATE"
expect_grep 'FIXED claim orphan-claim'

# ---------------------------------------------------------------- 4. zero is not a pass
# A tree with no blocking rule is a tree the rule gate could not measure, not a tree with
# nothing wrong; and a claims file that exists carrying no guaranteed claim is a population
# that vanished rather than one that came back empty. Reporting `clean` over either is the
# failure this whole rule is named for, so each gate exits unusable rather than passing.
F="$(fixture 3)"
rule "$F" only-advisory project.only-advisory advisory
expect_exit 12 env MJ_ROOT="$F" "$RGATE"
expect_grep 'nothing was measured, which is not the same as nothing being wrong'
printf 'version: 1\nclaims:\n' > "$F/docs/CLAIMS.yaml"
expect_exit 12 env MJ_ROOT="$F" "$CGATE"
expect_grep 'the population vanished rather than came back empty'

# ---------------------------------------------------------------- 5. every verdict has a denominator
# The rule the gates enforce, applied to the gates: no line either prints may carry a count
# without what the count is out of. Asserted against the real repository's output, because the
# wording is what a reader meets.
# MJ_ROOT points at this checkout so the wording a reader actually meets is what is asserted,
# and no mode that writes is used: a case that wrote into ROOT would violate
# project.tests-run-in-disposable-repos while proving a rule about honest measurement.
out="$(env MJ_ROOT="$ROOT" "$RGATE" 2>&1 || true)"
printf '%s\n' "$out" | grep -qE '^blocking rules: +[0-9]+ of [0-9]+ rule\(s\) in ' \
  || { printf '    the rule verdict states no denominator:\n%s\n' "$out"; exit 1; }
printf '%s\n' "$out" | grep -qE '^guaranteed claims: +not measured here' \
  || { printf '    the rule gate does not disclaim the claim population:\n%s\n' "$out"; exit 1; }

out="$(env MJ_ROOT="$ROOT" "$CGATE" 2>&1 || true)"
printf '%s\n' "$out" | grep -qE '^guaranteed claims: +[0-9]+ of [0-9]+' \
  || { printf '    the claim verdict states no denominator:\n%s\n' "$out"; exit 1; }
printf '%s\n' "$out" | grep -qE '^blocking rules: +not measured here' \
  || { printf '    the claim gate does not disclaim the rule population:\n%s\n' "$out"; exit 1; }
