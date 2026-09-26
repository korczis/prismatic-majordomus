# majordomus-covers: none
# claims: evidence-judged-at-the-presented-revision, evidence-a-skip-is-not-run, evidence-a-failure-outranks-an-absence
# A verdict is a verdict AT something, and this case proves that the evidence join says what,
# through the Rust executable's `evidence show` and `rules`, in a fixture repository of its
# own — the states below are reached by making commits, switching branches and editing the
# ledger, which a case must never do to the checkout it runs in.
#
# What it proves, in order:
#   1  a test that declined to run is `not_run` with its reason, never `failing`, and the
#      guarantee it proves is named by a finding that says it declined
#   2  a result word nobody can classify is an error, and an error is `failing`
#   3  a pass recorded on a commit the checked-out branch does not contain is `stale`, and
#      the detail names that commit — however empty a diff between the two trees is
#   4  `--presented HEAD` judges the checked-out commit as committed: from the ledger that
#      commit holds, against its own tree measured without the ledger's working copy. A
#      clean failure the checkout holds uncommitted caps it at `stale` and never decides it;
#      an uncommitted pass never strengthens it; a dirty tree, measured or declared, caps it
#      at `inputs_unchanged`; and a revision other than the checked-out commit, a revision
#      that names nothing, a tree state with no revision and an unreadable working ledger
#      are each refused
#   5  a ledger row naming a commit no object has is `stale`: git could not compare
#   6  a failure outranks an absence: a blocking rule whose tests are one failing and one
#      that declined to run is `failing`, and a finding
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# The JSON answers live outside the repository under test: a file written inside it would be
# an untracked file, which is a dirty tree, which is what several assertions below are about.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj502.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
rr() {           # rr <name> <args...> — the JSON answer of `rules` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" rules --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}
record() {       # record <tsv line...> — one runner report, recorded into the working ledger
  printf '%s\n' "$@" > "$W/run.tsv"
  expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/run.tsv"
}
TAB="$(printf '\t')"
ALPHA='.claims[] | select(.id == "alpha-holds")'
BETA='.claims[] | select(.id == "beta-holds")'
LEDGER=".ai/repo/evidence/ledger.json"

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
mkdir -p docs lib test/cases
cat > docs/CLAIMS.yaml <<'YAML'
version: 1
claims:
  - id: alpha-holds
    claim: Alpha holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/01_alpha.sh
    status: guaranteed
  - id: beta-holds
    claim: Beta holds
    source: docs/BETA.md
    implementation: lib/beta.sh
    test: test/cases/02_beta.sh
    status: guaranteed
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '# Beta\n'  > docs/BETA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '#!/usr/bin/env bash\n# beta\n'  > lib/beta.sh
printf '# the alpha case\n' > test/cases/01_alpha.sh
printf '# the beta case\n'  > test/cases/02_beta.sh
git add -A >/dev/null && git commit -qm fixture
TRUNK="$(git symbolic-ref --short HEAD)"
[ -z "$(git status --porcelain)" ] || { echo "    the fixture did not start on a clean tree"; git status --porcelain; exit 1; }

# ---------------------------------------------------------------- 1. a skip is not run
# Proves `evidence-a-skip-is-not-run`: a test that declined to run proved nothing and failed
# nothing, and it says why.
record "01_alpha${TAB}skip${TAB}0${TAB}parallel"
ev skip show
jqe skip "$ALPHA | .execution.outcome == \"skip\"" "the fixture did not record a skip"
jqe skip "$ALPHA | .state == \"not_run\"" "a recorded skip is not reported as not_run"
jqe skip "$ALPHA | .detail | test(\"declined\")" "a skip does not say the test declined to run"
jqe skip '[.findings[] | select(.claim == "alpha-holds") | .reason] | length == 1 and (.[0] | test("declined to run"))' \
  "the guarantee whose test declined to run is not a finding that says so"
jqe skip '[.findings[] | select(.claim == "alpha-holds") | .reason | test("no run of that test has ever been recorded")] == [false]' \
  "a skip is reported as a test that was never recorded"

# ---------------------------------------------------------------- 2. an error is failing
# A result nobody can classify is not a pass, and not an absence either.
record "01_alpha${TAB}exploded${TAB}1${TAB}parallel"
ev error show
jqe error "$ALPHA | .execution.outcome == \"error\"" "an unknown result word was not recorded as an error"
jqe error "$ALPHA | .state == \"failing\"" "an error is not reported as failing"
jqe error "$ALPHA | .detail == \"the harness could not run it\"" "an error does not say the harness could not run it"

# ---------------------------------------------------------------- 3. containment
# Proves `evidence-judged-at-the-presented-revision`, its containment half: a pass recorded
# on a side branch is about another history. The ledger is untracked here, so it stays in
# the working tree when the branch is switched back.
git checkout -q -b side
printf 'a side note\n' > side.txt
git add side.txt >/dev/null && git commit -qm side
SIDE="$(git rev-parse HEAD)"
record "01_alpha${TAB}ok${TAB}1${TAB}parallel"
git checkout -q "$TRUNK"
[ -f "$LEDGER" ] || { echo "    the ledger did not survive the switch back to $TRUNK"; exit 1; }
git merge-base --is-ancestor "$SIDE" HEAD && { echo "    the side commit is an ancestor of $TRUNK"; exit 1; }
ev elsewhere show
jqe elsewhere "$ALPHA | .execution.commit == \"$SIDE\" and .execution.outcome == \"pass\"" \
  "the pass on the side branch is not the execution behind the claim"
jqe elsewhere "$ALPHA | .state == \"stale\"" \
  "a pass on a commit the checked-out branch does not contain is not stale"
jqe elsewhere "$ALPHA | .detail | contains(\"${SIDE:0:12}\") and contains(\"does not contain\")" \
  "the detail does not name the commit the presented revision does not contain"

# ---------------------------------------------------------------- 4. the presented commit
# The checked-out commit, judged as committed: the ledger that commit holds, the tree
# measured without the ledger's working copy.
C1="$(git rev-parse HEAD)"
record "01_alpha${TAB}ok${TAB}2${TAB}parallel"
git add "$LEDGER" >/dev/null && git commit -qm ledger
C2="$(git rev-parse HEAD)"
[ -z "$(git status --porcelain)" ] || { echo "    committing the ledger left the tree dirty"; git status --porcelain; exit 1; }
ev p_clean show --presented HEAD
jqe p_clean "$ALPHA | .execution.commit == \"$C1\" and .state == \"proven\"" \
  "a pass recorded on the commit before, with only the ledger committed since, is not proven at HEAD"
jqe p_clean ".presented.revision == \"$C2\" and .presented.tree == \"clean\"" \
  "the report does not say it was judged at the checked-out commit, clean"
jqe p_clean '.presented | has("uncommitted") | not' "a clean checkout reports uncommitted runs"
expect_exit 0 "$MJB" evidence --repo "$T" show --presented HEAD
expect_grep "judged at ${C2:0:12} as committed \(clean\)"
expect_exit 0 "$MJB" evidence --repo "$T" show
expect_grep "judged at the working tree \(HEAD ${C2:0:12}, clean\)"

# The verdict follows the commit's ledger, and a clean failure the checkout holds withholds
# `proven` there. The recorder ignores the ledger when it measures the tree, so this run is
# stamped with C2 and a clean tree: a clean failing run of exactly what is presented.
record "01_alpha${TAB}FAIL${TAB}3${TAB}parallel"
ev w_fail show
jqe w_fail "$ALPHA | .state == \"failing\"" "the working tree does not read its own failing run"
jqe w_fail "$ALPHA | .execution.commit == \"$C2\" and .execution.working_tree == \"clean\"" \
  "the failing run was not stamped with the presented commit and a clean tree"
ev p_fail show --presented HEAD
jqe p_fail "$ALPHA | .state != \"proven\"" \
  "a named revision reads proven over a clean failing run the checkout holds"
jqe p_fail "$ALPHA | .state == \"stale\"" \
  "a clean failure the checkout holds does not cap the presented commit at stale"
jqe p_fail "$ALPHA | .execution.outcome == \"pass\"" \
  "the presented commit was not judged from the ledger it holds"
jqe p_fail "$ALPHA | .detail | contains(\"uncommitted\") and contains(\"${C2:0:12}\")" \
  "the cap does not name the uncommitted run and its commit"
jqe p_fail '.presented.uncommitted == ["suite:01_alpha"]' \
  "the report does not list the test whose run the checkout holds uncommitted"
jqe p_fail '.presented.tree == "clean"' "the ledger's working copy was counted as a dirty tree"
expect_exit 0 "$MJB" evidence --repo "$T" show --presented HEAD
expect_grep 'the working ledger holds executions this commit does not \(suite:01_alpha\)'
git checkout -q -- "$LEDGER"
ev p_restored show --presented HEAD
jqe p_restored "$ALPHA | .state == \"proven\"" "restoring the ledger did not restore the verdict"
jqe p_restored '.presented | has("uncommitted") | not' "a restored ledger still reports uncommitted runs"

# An uncommitted pass never strengthens: the commit's ledger holds no row for beta.
record "02_beta${TAB}ok${TAB}1${TAB}parallel"
ev w_beta show
jqe w_beta "$BETA | .state == \"proven\"" "the working tree does not read its own passing run"
ev p_beta show --presented HEAD
jqe p_beta "$BETA | .state == \"not_run\"" "an uncommitted pass strengthened the presented commit"
jqe p_beta '.presented.uncommitted == ["suite:02_beta"]' "the uncommitted pass is not listed"
git checkout -q -- "$LEDGER"

# The presented tree is measured, and a caller can only weaken it.
printf 'an unrelated note\n' > note.txt
ev p_dirty show --presented HEAD
jqe p_dirty '.presented.tree == "dirty"' "an untracked file did not make the presented tree dirty"
jqe p_dirty "$ALPHA | .state == \"inputs_unchanged\"" "a dirty presented tree read as proven"
jqe p_dirty "$ALPHA | .detail | test(\"presented revision was built from a tree that was not its commit\")" \
  "the cap does not say the presented tree was not its commit"
ev p_dirty_told show --presented HEAD --presented-tree clean
jqe p_dirty_told "$ALPHA | .state == \"inputs_unchanged\"" "a given clean tree made a dirty checkout proven"
jqe p_dirty_told '.presented.tree == "dirty"' "a given clean tree overrode a measured dirty one"
rm -f note.txt
ev p_told show --presented HEAD --presented-tree dirty
jqe p_told '.presented.tree == "dirty"' "a given dirty tree did not weaken a clean measurement"
jqe p_told "$ALPHA | .state == \"inputs_unchanged\"" "a declared dirty tree read as proven"

# One unrelated commit later the run is no longer of this commit.
printf 'committed later\n' > later.txt
git add later.txt >/dev/null && git commit -qm later
C3="$(git rev-parse HEAD)"
ev p_later show --presented HEAD
jqe p_later ".presented.revision == \"$C3\"" "the report does not follow the checked-out commit"
jqe p_later "$ALPHA | .state == \"inputs_unchanged\"" \
  "a pass whose inputs are unchanged one commit later is not inputs_unchanged"

# Refusals: each is a non-zero exit that names what was wrong.
expect_exit 13 "$MJB" evidence --repo "$T" show --presented "$C2"
expect_grep "not the checked-out commit ${C3:0:12}"
ZEROS="0000000000000000000000000000000000000000"
expect_exit 13 "$MJB" evidence --repo "$T" show --presented "$ZEROS"
expect_grep "$ZEROS"
expect_exit 13 "$MJB" evidence --repo "$T" show --presented-tree dirty
expect_grep 'presented'
printf 'this is not a ledger\n' > "$LEDGER"
expect_exit 13 "$MJB" evidence --repo "$T" show --presented HEAD
expect_grep '\.ai/repo/evidence/ledger\.json'
expect_grep 'cannot be ruled out'
git checkout -q -- "$LEDGER"

# ---------------------------------------------------------------- 5. an unknown commit
# A row naming a commit no object has cannot be compared, and not knowing is not proof.
GHOST="0123456789abcdef0123456789abcdef01234567"
jq --arg c "$GHOST" '.executions |= map(if .test == "suite:01_alpha" then .commit = $c else . end)' \
  "$LEDGER" > "$W/ghost-ledger.json"
cp "$W/ghost-ledger.json" "$LEDGER"
ev ghost show
jqe ghost "$ALPHA | .execution.commit == \"$GHOST\"" "the fixture did not write the unknown commit"
jqe ghost "$ALPHA | .state == \"stale\"" "a row naming a commit no object has is not stale"
jqe ghost "$ALPHA | .detail | test(\"git could not compare\")" "the detail does not say git could not compare"
git checkout -q -- "$LEDGER"

# ---------------------------------------------------------------- 6. a failure outranks an absence
# Proves `evidence-a-failure-outranks-an-absence`: a blocking rule naming two cases, one
# failing and one that declined to run. The declared order ranks `failing` above `not_run`,
# so a plain weakest-of would read `not_run` — not a finding — and hide the failure.
P=.ai/repo/rules/project
mkdir -p "$P"
# the vendored rules name cases of their own; materialise them, so that the rule this
# section adds is the only one whose state it reasons about
grep -rhoE 'test/cases/[0-9]+_[a-z0-9_]+\.sh' .ai/repo/rules | LC_ALL=C sort -u \
  | while read -r c; do [ -f "$c" ] || : > "$c"; done
cat > "$P/fixture-both.v1.md" <<'MD'
---
id: project.fixture-both
version: 1
kind: rule
title: Rule fixture-both
description: What project.fixture-both requires, in one sentence.
statement: The normative sentence project.fixture-both asks a worker to follow.
status: active
class: blocking
depends_on: []
tags: [fixture]

x-majordomus:
  tests: [test/cases/01_alpha.sh, test/cases/02_beta.sh]
---

# Rationale

A fixture.

# Required behaviour

The fixture holds.

# Failure behaviour

The fixture is reported.

# Verification

This case.
MD
# discovery reads the files git knows
git add -A >/dev/null
record "01_alpha${TAB}FAIL${TAB}1${TAB}parallel" "02_beta${TAB}skip${TAB}0${TAB}parallel"
rr both show project.fixture-both
jqe both '[.proof.tests[].state] == ["failing", "not_run"]' \
  "the rule's two tests are not one failing and one not run"
jqe both '.proof.state == "failing"' "a failing test beside one that declined to run did not make the rule failing"
jqe both '.proof.satisfied == false' "a blocking rule with a failing test is satisfied"
rr all report
jqe all '[.findings[] | select(.rule == "project.fixture-both")] | length == 1' \
  "a blocking rule with a failing test and a skipped one is not a finding"
jqe all '[.rules[] | select(.rule.id == "project.fixture-both") | .state] == ["failing"]' \
  "the corpus report disagrees with the single-rule answer"
jqe all '.verdict == "failing"' "a corpus with a failing blocking rule is not failing"
exit 0
