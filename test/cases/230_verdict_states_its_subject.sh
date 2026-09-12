# majordomus-covers: none
# The gate of project.a-verdict-states-its-subject, driven against fixture trees rather than
# against this checkout. A case that asserted this repository's own six blocking rules and
# 129 unproven claims would pass the day somebody edited the baseline in the same commit as
# the debt — which is the thing the gate exists to make visible. What is asserted here is the
# gate's behaviour: what it refuses, what it deliberately does not refuse, and that each
# verdict it prints carries the denominator the rule is named for.
#
# The non-refusals carry as much weight as the refusals. This gate is about detectors that
# cannot tell their subject from something that merely looks like it, so a case that only
# proved it fires would leave its own false-positive surface untested. Three shapes are
# planted deliberately and each must pass: a rule that names its case by path, a rule that
# names it the way the runner takes it (`bash test/run.sh 98_why_catalogue` — the spelling
# the first draft of this gate reported as "names no proof"), and a rule whose body quotes
# `class: blocking` in its prose while its front matter says otherwise.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/enforcement-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# rule <dir> <file> <id> <class> <verification-body> [extra front matter]
rule() {
  local f="$1/.ai/repo/rules/project/$2.v1.md"
  mkdir -p "$(dirname "$f")"
  { printf -- '---\nid: %s\nversion: 1\nkind: rule\ntitle: t\ndescription: d\nstatement: s\nstatus: active\nclass: %s\ndepends_on: []\ntags: []\n' "$3" "$4"
    if [ -n "${6:-}" ]; then printf '%s\n' "$6"; fi
    printf -- '---\n\n# Rationale\n\nr\n\n# Required behaviour\n\nb\n\n# Verification\n\n%s\n' "$5"
  } > "$f"
}

# claim <dir> <id> <status> <test-path>
# The claims file is created by the first claim, never by the fixture: a tree with no
# docs/CLAIMS.yaml is a tree where the claim half does not apply (an adopter owes none of
# ours), and that must be distinguishable from a claims file with nothing in it.
claim() {
  if [ ! -f "$1/docs/CLAIMS.yaml" ]; then printf 'version: 1\nclaims:\n' > "$1/docs/CLAIMS.yaml"; fi
  printf '  - id: %s\n    claim: c\n    source: docs/X.md\n    implementation: lib/x.sh\n    test: %s\n    status: %s\n' "$2" "$4" "$3" >> "$1/docs/CLAIMS.yaml"
}

fixture() {
  local f="$T/tree$1"
  rm -rf "$f"
  mkdir -p "$f/.ai/repo/rules/project" "$f/test/cases" "$f/scripts/ci" "$f/docs"
  printf '#!/bin/sh\necho ok\n' > "$f/test/cases/98_why_catalogue.sh"
  printf '#!/bin/sh\necho ok\n' > "$f/test/cases/42_named_by_path.sh"
  printf '#!/bin/sh\necho ok\n' > "$f/scripts/ci/some-gate"
  printf '%s' "$f"
}

# ---------------------------------------------------------------- 1. what it accepts
# Four rules that are enforced, each by a different spelling, and one advisory rule with no
# proof at all — advisory is outside the population and must not be counted.
F="$(fixture 0)"
rule "$F" wired            project.wired            blocking 'Review.' 'x-majordomus:
  validator: wired
  enforced_by: [doctor]
  exit_code: 10'
rule "$F" by-path          project.by-path          blocking 'Proven by `test/cases/42_named_by_path.sh`, which mutates the tree.'
rule "$F" by-runner        project.by-runner        blocking 'Run `bash test/run.sh 98_why_catalogue`; it adds one file and finds it everywhere.'
rule "$F" by-gate          project.by-gate          blocking 'The gate is `scripts/ci/some-gate`; review decides the rest.'
rule "$F" advisory-no-proof project.advisory-no-proof advisory 'Review. Nothing runs for this and nothing is meant to.'
claim "$F" proven guaranteed test/cases/42_named_by_path.sh
printf 'echo proven\n' >> "$F/test/cases/42_named_by_path.sh"   # the test names the claim back
expect_exit 0 env MJ_ROOT="$F" "$GATE" --strict
expect_grep 'blocking rules: +0 of 4'
expect_grep 'guaranteed claims: +0 of 1'
expect_grep 'every blocking rule and every guaranteed claim names a proof'
# the advisory rule was never in the denominator: 4 blocking rules, not 5
expect_no_grep 'advisory-no-proof'

# --- a body that quotes the rule's own subject does not make the rule its own subject.
# This is lease-reader-check's defect in miniature: the gate reads front matter, never the
# body, so a rule whose prose contains "class: blocking" is classed by its front matter.
rule "$F" prose project.prose advisory 'Review. A rule that says `class: blocking` in its body is still advisory.'
expect_exit 0 env MJ_ROOT="$F" "$GATE" --strict
expect_grep 'blocking rules: +0 of 4'

# ---------------------------------------------------------------- 2. what it refuses
# A blocking rule whose Verification section names only prose, and one that names a path the
# tree does not have — a stale pointer reads as evidence and is the more dangerous of the two.
F="$(fixture 1)"
rule "$F" reviewed project.reviewed blocking 'Review.'
rule "$F" stale    project.stale    blocking 'Proven by `test/cases/77_this_was_deleted.sh`.'
rule "$F" by-path  project.by-path  blocking 'Proven by `test/cases/42_named_by_path.sh`.'
expect_exit 10 env MJ_ROOT="$F" "$GATE" --strict
expect_grep 'blocking rules: +2 of 3'
expect_grep 'guaranteed claims: +not applicable'
expect_grep 'project.reviewed'
expect_grep 'project.stale'
expect_no_grep 'project.by-path'

# --- a guaranteed claim whose test does not name it back. The path exists, so the check
# that only verifies existence passes it; the link is one-way and a rewrite breaks it silently.
claim "$F" unproven guaranteed test/cases/42_named_by_path.sh
claim "$F" planned-one planned  test/cases/42_named_by_path.sh
expect_exit 10 env MJ_ROOT="$F" "$GATE" --strict
expect_grep 'guaranteed claims: +1 of 1'
expect_grep 'unproven'
expect_no_grep 'planned-one'

# ---------------------------------------------------------------- 3. the ratchet
# The debt is recorded, the gate falls silent about it, and a promise added afterwards is
# reported. A baseline is how known debt is carried, never how a new one is admitted.
expect_exit 0 env MJ_ROOT="$F" "$GATE" --write-baseline
expect_grep 'baseline written'
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'no new promise without a proof'
rule "$F" fresh project.fresh blocking 'Review.'
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'NEW project.fresh'
expect_no_grep 'NEW project.reviewed'
# and a promise that gains a proof is reported as such, so the list cannot rot
rule "$F" fresh project.fresh blocking 'Proven by `test/cases/42_named_by_path.sh`.'
rule "$F" stale project.stale blocking 'Proven by `test/cases/42_named_by_path.sh`.'
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'FIXED project.stale'

# ---------------------------------------------------------------- 4. zero is not a pass
# A tree with no blocking rules and no claims is a tree the gate could not measure, not a
# tree with nothing wrong. Reporting `clean` over an empty population is the failure this
# whole rule is named for, so the gate exits unusable rather than passing.
F="$(fixture 2)"
rule "$F" only-advisory project.only-advisory advisory 'Review.'
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'nothing was measured, which is not the same as nothing being wrong'
# and a claims file that exists and carries no guaranteed claim is a population that
# vanished, not one that came back empty
rule "$F" blocking-one project.blocking-one blocking 'Proven by `test/cases/42_named_by_path.sh`.'
printf 'version: 1\nclaims:\n' > "$F/docs/CLAIMS.yaml"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'the population vanished rather than came back empty'

# ---------------------------------------------------------------- 5. every verdict has a denominator
# The rule the gate enforces, applied to the gate: no line it prints may carry a count
# without what the count is out of. Asserted against the real repository's output, because
# the wording is what a reader meets.
# MJ_ROOT points at this checkout so the wording a reader actually meets is what is
# asserted, and no mode that writes is used: a case that wrote into ROOT would violate
# project.tests-run-in-disposable-repos while proving a rule about honest measurement.
out="$(env MJ_ROOT="$ROOT" "$GATE" 2>&1 || true)"
printf '%s\n' "$out" | grep -qE '^blocking rules: +[0-9]+ of [0-9]+' \
  || { printf '    the blocking verdict states no denominator:\n%s\n' "$out"; exit 1; }
printf '%s\n' "$out" | grep -qE '^guaranteed claims: +[0-9]+ of [0-9]+' \
  || { printf '    the claim verdict states no denominator:\n%s\n' "$out"; exit 1; }
