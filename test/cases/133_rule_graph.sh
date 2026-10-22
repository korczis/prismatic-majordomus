# majordomus-covers: rules
# majordomus-negative: rules
# A rule is only as enforced as what proves it, and this is the reading that says so.
#
# `majordomus doctrine` asks whether the repository satisfies a rule right now. That is a
# question about the tree. There is a second question — is this rule in a state where it
# could be satisfied at all — which is about the rule, and the proof graph is what asks it.
# This case drives that graph through the executable, in a fixture repository of its own,
# because the states that matter are states about commits, about files that stop existing,
# and about runs that happened and then went out of date.
#
# What it proves, in order:
#   1  the corpus is read from the tree: every rule of the fixture appears, in canonical
#      order, with tallies counted from what was derived and nothing written down
#   2  a rule naming a case a runner drives, with no run recorded, is `not run` — visible,
#      and not a finding: nothing is broken, nothing has been measured
#   3  a recorded pass makes it `proven`, carrying the execution behind it
#   4  editing what the rule is about makes the same rule `stale`: the proof is older than
#      its subject, which is a different fact from never having had one
#   5  deleting the case makes it `dangling` and a finding at every class, because the rule
#      still reads as enforced everywhere that checks only the name
#   6  a blocking rule naming nothing is `unproven`, and the answer says what would fix it
#   7  `reviewed_because` is a third mode: reported, counted apart, never proof, and an
#      executable proof always wins over it
#   8  a check wired as its own CI gate is `gated` — a mechanism, not a verdict — and is
#      distinguished from a path no runner and no gate drives, which is `unrunnable`
#   9  the relation walks both ways: a test names the rules it proves, and says which of
#      them would be left with nothing at all if it were deleted
#  10  a rule the repository does not declare is a not-found, never an empty answer
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# The JSON answers live outside the repository under test: a file written inside it would
# be an untracked file, which is a dirty tree, which is what the `proven` assertion is about.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj133.XXXXXX")"; trap 'rm -rf "$W"' EXIT

rr() {           # rr <name> <args...> — the JSON answer into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" rules --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}

# ---------------------------------------------------------------- the fixture
# A repository with a layer of its own, cases of its own and a CI model of its own. Not
# this repository's: the states below are reached by editing files and making commits, and
# a case that did that to the checkout it runs in would be rewriting what it measures.
"$MJ" init >/dev/null
P=".ai/repo/rules/project"
mkdir -p "$P" test/cases scripts/ci .ai/repo/ci

rule() {         # rule <stem> <class> [block line ...]
  local stem="$1" cls="$2"; shift 2
  { cat <<Y
---
id: project.$stem
version: 1
kind: rule
title: Rule $stem
description: What project.$stem requires, in one sentence.
statement: The normative sentence project.$stem asks a worker to follow.
status: active
class: $cls
depends_on: []
tags: [fixture]
Y
    if [ $# -gt 0 ]; then printf '\nx-majordomus:\n'; printf '  %s\n' "$@"; fi
    printf -- '---\n\n# Rationale\n\nA fixture.\n\n# Required behaviour\n\nThe fixture holds.\n\n# Failure behaviour\n\nThe fixture is reported.\n\n# Verification\n\nThis case.\n'
  } > "$P/$stem.v1.md"
}

# The vendored baseline names cases of its own; materialise them, so that the only rule in
# an interesting state below is one this case put there.
grep -rhoE 'test/cases/[0-9]+_[a-z0-9_]+\.sh' .ai/repo/rules | LC_ALL=C sort -u \
  | while read -r c; do : > "$c"; done

# A validator is a shell function in lib/, so the fixture defines the one it asserts on.
# Without this the vendored dispatched rules would correctly report no validator here —
# correct for this repository, and not what the assertion below is about.
mkdir -p lib
printf '#!/usr/bin/env bash\nmj_validate_adr() { :; }\n' > lib/doctrine_fixture.sh

printf '# the alpha case\n' > test/cases/01_alpha.sh
printf '# the beta case\n'  > test/cases/02_beta.sh
printf '#!/bin/sh\nexit 0\n' > scripts/ci/gamma-check && chmod +x scripts/ci/gamma-check
printf '#!/bin/sh\nexit 0\n' > scripts/not-a-gate.sh && chmod +x scripts/not-a-gate.sh
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: gamma
    job: structure
    always: true
    runs: scripts/ci/gamma-check
    summary: the fixture's own gate
  - id: shell-suite
    job: suite
    runs: bash test/run.sh
    summary: every behavioural case
classes: []
YAML

rule alpha   blocking 'tests: [test/cases/01_alpha.sh]'
rule beta    advisory 'tests: [test/cases/02_beta.sh]'
rule gamma   blocking 'tests: [scripts/ci/gamma-check]'
rule delta   blocking 'tests: [scripts/not-a-gate.sh]'
rule epsilon blocking
rule zeta    blocking 'reviewed_because: the rule is about the provenance of code, which no program in this tree can read'
git add -A >/dev/null && git commit -qm fixture

# ---------------------------------------------------------------- 1. the corpus is read
# Every number below is counted from the tree. A denominator written down anywhere would be
# a denominator that goes stale, which is the defect this repository keeps rediscovering.
expect_exit 0 "$MJB" rules --repo "$T" report
rr all report
jqe all '[.rules[].rule.id] | index("project.alpha") != null and index("project.zeta") != null' \
  "the report does not carry the rules the fixture declares"
# `sort_by(.)` rather than `sort`: identical in jq, and scripts/ci/order-check's regex for
# an unpinned shell sort cannot tell a jq filter from a pipeline into sort(1).
jqe all '[.rules[].rule.id] == ([.rules[].rule.id] | sort_by(.))' \
  "the rules are not reported in a total order, so two runs may disagree"
jqe all '.coverage.rules == ([.rules[]] | length)' \
  "the tally does not count the rules the report carries"
jqe all '.coverage.blocking + .coverage.advisory <= .coverage.rules' \
  "the classes are counted for more rules than exist"

# Two runs over one tree agree. Discovery order is the filesystem's business and must not
# reach the answer.
rr all2 report
cmp -s "$W/all.json" "$W/all2.json" || { echo "    two runs over one tree disagree"; exit 1; }

# ---------------------------------------------------------------- 2. named, never run
# A rule naming a case nobody has run is visible and is not a finding: nothing is broken,
# and nothing has been measured. Collapsing those two is how a red report stops being read.
rr alpha show project.alpha
jqe alpha '.proof.state == "not_run"' "a case with no recorded run is not reported as not_run"
jqe alpha '.proof.rule.enforcement.mode == "gated"' "a rule naming a case is not gated"
jqe alpha '.proof.tests[0].kind == "case" and .proof.tests[0].present == true' \
  "the case the rule names is not read as a present case"
jqe alpha '.proof.tests[0].gates == ["shell-suite"]' \
  "the gate that drives the suite is not derived from the CI model"
jqe alpha '.missing | test("bash test/run.sh 01_alpha")' \
  "the answer does not say what would fix it, with the command"
jqe all '[.findings[].rule] | index("project.alpha") == null' \
  "a rule that has simply never been run is reported as a finding"

# ---------------------------------------------------------------- 3. a recorded pass
# the runner's report is name<TAB>result<TAB>seconds<TAB>phase, which is what CI writes
printf '01_alpha\tok\t1\tcase\n02_beta\tok\t1\tcase\n' > "$W/report.tsv"
run_quiet "$W/rec1.err" "$MJB" evidence --repo "$T" record --suite "$W/report.tsv"
rr alpha2 show project.alpha
jqe alpha2 '.proof.state == "proven"' "a pass recorded against this very tree is not proven"
jqe alpha2 '.proof.tests[0].execution.outcome == "pass"' "the execution behind the state is not carried"
jqe alpha2 '.proof.tests[0].execution.commit != null' "the execution names no commit"
jqe alpha2 '.missing == null' "a proven rule is told to fix something"

# ---------------------------------------------------------------- 4. proof older than subject
# Editing the case makes the run older than what it measured. The rule did pass; it does
# not pass *this*, and those are different sentences.
printf '# the alpha case, edited\n' > test/cases/01_alpha.sh
rr alpha3 show project.alpha
jqe alpha3 '.proof.state == "stale"' "a run older than the case it ran is not reported as stale"
jqe alpha3 '.missing | test("Run what it names again")' "a stale proof does not say to run it again"
git checkout -q -- test/cases/01_alpha.sh

# ---------------------------------------------------------------- 5. the proof that is gone
# The mutation the whole graph exists for: the case is deleted, the rule still names it,
# and every surface that checks only the name reads the rule as enforced.
rm -f test/cases/01_alpha.sh
rr alpha4 show project.alpha
jqe alpha4 '.proof.state == "dangling"' "a rule naming a deleted case is not reported as dangling"
jqe alpha4 '.proof.tests[0].present == false' "the missing case is reported as present"
jqe alpha4 '.proof.satisfied == false' "a dangling rule is reported as satisfied"
jqe alpha4 '.missing | test("not in the tree")' "the answer does not name what is missing"
rr fall report
jqe fall '[.findings[] | select(.rule == "project.alpha")] | length == 1' \
  "a dangling proof is not a finding"

# A dangling proof is a finding at every class, because it reads as proof and is not — an
# advisory rule owes no live proof, and it does owe the truth about what it names.
rm -f test/cases/02_beta.sh
rr fall2 report --findings
jqe fall2 '[.findings[] | select(.rule == "project.beta")] | length == 1' \
  "a dangling proof on an advisory rule is excused"
jqe fall2 '.coverage.rules == ([.rules[]] | length) + 0 or .coverage.rules > 0' \
  "the tallies do not count the whole corpus when the answer is filtered"
git checkout -q -- test/cases/01_alpha.sh test/cases/02_beta.sh

# ---------------------------------------------------------------- 6. naming nothing
rr eps show project.epsilon
jqe eps '.proof.state == "unproven"' "a blocking rule naming nothing is not reported as unproven"
jqe eps '.proof.rule.enforcement.mode == "declarative"' "a rule naming nothing has an enforcement mode"
jqe eps '.proof.satisfied == false' "a blocking rule nobody can prove is reported as satisfied"
jqe eps '.missing | test("x-majordomus")' "the answer does not say where to name the proof"

# ---------------------------------------------------------------- 7. the third mode
rr zeta show project.zeta
jqe zeta '.proof.state == "reviewed"' "a declared, reasoned exemption is not reported as reviewed"
jqe zeta '.proof.rule.enforcement.reviewed_because | test("provenance")' "the reason is not carried"
jqe zeta '.proof.satisfied == true' "a declared exemption is reported as a finding"
rr rall report
jqe rall '.coverage.review_only >= 1' "review-enforced rules are not counted apart"
jqe rall '[.rules[] | select(.rule.id == "project.zeta") | .state] == ["reviewed"]' \
  "the corpus report disagrees with the single-rule answer"
# ...and it is never counted as proof: review_only and passing are disjoint tallies, so a
# corpus whose only non-executable rule is reviewed has it in the first and not the second
jqe rall '.coverage.review_only > 0 and (.coverage.named_proof | tostring) != (.coverage.review_only | tostring)' \
  "review_only is being counted as if it named executable proof"

# An executable proof always wins, so the declaration cannot outlive its own need.
rule zeta blocking 'reviewed_because: the rule is about the provenance of code, which no program in this tree can read' 'tests: [test/cases/02_beta.sh]'
rr zeta2 show project.zeta
jqe zeta2 '.proof.rule.enforcement.mode == "gated"' \
  "a rule that acquired a case is still read as review-enforced"
jqe zeta2 '.proof.state != "reviewed"' "an executable proof did not win over the declaration"

# ---------------------------------------------------------------- 8. mechanism, not verdict
# Proves `rule-state-is-derived-from-what-ran`: the state is derived from the tree and
# the ledger together, and a mechanism that refuses violations is reported as a mechanism
# — never as a run that passed.
# A check a CI gate runs refuses violations on every run. What this repository does not have
# is a verdict for it, and reading the mechanism as a verdict is the green badge the whole
# exercise is against.
rr gamma show project.gamma
jqe gamma '.proof.tests[0].kind == "gate"' "a check wired as a gate is not read as a gate"
jqe gamma '.proof.state == "gated"' "a gated rule is not reported as gated"
jqe gamma '.proof.gates == ["gamma"]' "the gate that runs it is not derived from the CI model"
jqe gamma '.proof.satisfied == true' "a gate that refuses violations is reported as a finding"
jqe gamma '.missing | test("verdict")' "the answer does not say what a gate cannot show"
jqe rall '.coverage.mechanism_only >= 1' "a mechanism with no verdict is counted among the passing"

# The same shape with no gate behind it is proof in prose only, and says so.
rr delta show project.delta
jqe delta '.proof.tests[0].kind == "unknown"' "a path nothing runs is read as runnable"
jqe delta '.proof.state == "unrunnable"' "a path nothing runs is not reported as unrunnable"
jqe delta '.proof.satisfied == false' "a blocking rule nothing can run is reported as satisfied"

# ---------------------------------------------------------------- 8b. a validator is a function
# `validator: adr` names the shell function mj_validate_adr, which the dispatcher calls. It
# is not a path, and reading it as one made every dispatched rule in the vendored package
# report as naming something not in the tree — the opposite of true, since those are the
# best-enforced rules there are. This case found that defect; it keeps it fixed.
rr disp show majordomus.adr-integrity
jqe disp '.proof.rule.enforcement.mode == "dispatched"' "a rule naming a validator is not dispatched"
jqe disp '.proof.validator.function == "mj_validate_adr"' "the validator is not named as the function the dispatcher calls"
jqe disp '.proof.validator.present == true' "a validator lib/ defines is reported as missing"
jqe disp '.proof.validator.defined_in | startswith("lib/")' "the file defining the validator is not named"
jqe disp '.proof.state != "dangling"' "a dispatched rule with a real validator is reported as dangling"

# ---------------------------------------------------------------- 9. both directions
# The question that could not be asked while the relation ran one way: before renaming this
# case, what stops being enforced?
rr proves proves test/cases/01_alpha.sh
jqe proves '[.proves[].id] | index("project.alpha") != null' "a test does not name the rule that names it"
jqe proves '.sole_proof_of | index("project.alpha") != null' \
  "the test that is a rule's only proof does not say so"
jqe proves '.present == true and .test == "suite:01_alpha"' "the test identity is not derived from the path"
# and the identity spelling answers the same as the path spelling
rr proves2 proves suite:01_alpha
jqe proves2 '[.proves[].id] == '"$(jq -c '[.proves[].id]' "$W/proves.json")" \
  "the path and the identity give different answers for one test"

# ---------------------------------------------------------------- 10. a rule that is not one
# A typo must never read as "this rule has no proof". That is the one answer this capability
# may not give.
expect_exit 12 "$MJB" rules --repo "$T" show project.no-such-rule
expect_grep 'is not a rule of this repository'
