# A claim's `test: <path>` is a promise that a proof exists somewhere. The evidence fabric
# is the other half: the ledger of what actually ran, against which commit, with what
# result — and the join that decides, per claim, whether the repository may still call it
# proven. This case drives that join through the executable, in a fixture repository of its
# own, because the behaviours that matter are behaviours about commits and about time.
#
# What it proves, in order:
#   1  a repository that recorded nothing answers `not_run` for every claim, and succeeds
#   2  a recorded pass carries its provenance: commit, outcome, duration, origin, digest
#   3  a SECOND, partial run updates only the test it names — a one-case run must never
#      erase the repository's proof of everything else
#   4  a test edited after its run makes the claim `stale`, and the changed path is named
#   5  `proven` and `inputs_unchanged` are distinguished: recorded at HEAD with a clean
#      tree is the first, one unrelated commit later it is the second and never the first
#   6  a failing result is `failing`, and `show --check` exits 10 only when a *guaranteed*
#      claim is unsupported
#   7  the relation walks both ways: a test lists the claims it proves, and every claim it
#      lists names it back
#   8  a malformed report is refused and writes no ledger; a result for a test that does
#      not exist is named as unknown and recorded for nobody
#
# Why the fixture tracks its ledger, as this repository does: `proven` is not "recorded at
# HEAD with a clean tree" — that rule is unsatisfiable anywhere the evidence is tracked,
# because recording writes the ledger and committing the record moves HEAD past the commit
# the record names. It is "the diff against the execution's own commit is empty", with the
# ledger's own row excluded, because the evidence is about the tree rather than part of what
# the tests measure. So the fixture keeps its ledger tracked and asserts both halves: the
# tree IS dirty after a recording, and every claim is `proven` anyway.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# The run reports and the JSON answers live outside the repository under test: a file
# written inside it would be an untracked file, which is a dirty tree, which is the very
# thing the `proven` assertion is about.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj124.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}

# ---------------------------------------------------------------- the fixture
# A repository with a layer, a claims matrix of its own and cases of its own. Not this
# repository's matrix: the states asserted below are reached by editing files and making
# commits, and a case that did that to the checkout it runs in would be rewriting the thing
# it measures.
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
  - id: alpha-also
    claim: Alpha holds for a second reader
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/01_alpha.sh
    status: advisory
  - id: beta-holds
    claim: Beta holds
    source: docs/BETA.md
    implementation: lib/beta.sh
    test: test/cases/02_beta.sh
    status: guaranteed
  - id: gamma-planned
    claim: Gamma is not built yet
    source: docs/ALPHA.md
    implementation: '-'
    test: '-'
    status: planned
  - id: delta-unrunnable
    claim: Delta is proved by something no runner drives
    source: docs/BETA.md
    implementation: lib/beta.sh
    test: lib/delta.sh
    status: advisory
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '# Beta\n'  > docs/BETA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '#!/usr/bin/env bash\n# beta\n'  > lib/beta.sh
printf '#!/usr/bin/env bash\n# delta\n' > lib/delta.sh
printf '# the alpha case\n' > test/cases/01_alpha.sh
printf '# the beta case\n'  > test/cases/02_beta.sh
git add -A >/dev/null && git commit -qm fixture
C1="$(git rev-parse HEAD)"
LEDGER=".ai/repo/evidence/ledger.json"

# ---------------------------------------------------------------- 1. nothing recorded
# A fresh clone has no ledger. That is not an error, and the answer is `not run` rather
# than a refusal to answer: a repository that cannot say "nothing was recorded" would have
# to say nothing at all.
expect_exit 0 "$MJB" evidence --repo "$T" show
expect_grep 'nothing has been recorded'
ev empty show
jqe empty '.ledger.present == false and .ledger.executions == 0' \
  "a repository that recorded nothing does not report an empty ledger"
# the matrix the rest of this case reasons about, named once: every "no claim is in state
# X" below is only an assertion while there are claims to be in one
jqe empty '[.claims[].id] | sort == ["alpha-also", "alpha-holds", "beta-holds", "delta-unrunnable", "gamma-planned"]' \
  "the report does not carry the five claims the fixture matrix declares"
jqe empty '[.claims[] | select(.test != null) | select(.state != "not_run")] | length == 0' \
  "a claim naming a runnable test reports something other than not_run with no ledger"
jqe empty '(.claims[] | select(.id == "gamma-planned") | .state) == "no_test"' \
  "a claim naming no test is not reported as no_test"
jqe empty '(.claims[] | select(.id == "delta-unrunnable") | .state) == "unrunnable"' \
  "a claim naming a path no runner drives is not reported as unrunnable"
# and the two guarantees are the only findings: a planned claim and an advisory one are not
# held to a proof they never claimed
jqe empty '[.findings[].claim] | sort == ["alpha-holds", "beta-holds"]' \
  "the findings hold a claim to a proof its status does not claim, or miss a guarantee"

# ---------------------------------------------------------------- 8a. a malformed report
# Before anything is recorded, so that "wrote no ledger" is a fact about the file and not
# about its contents. A dropped line would leave a claim reading `not run` after a run that
# ran it, which is a lie in the safe direction and still a lie.
printf '01_alpha\tok\t12\n' > "$W/bad.tsv"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/bad.tsv"
expect_grep 'name<TAB>result<TAB>seconds<TAB>phase'
[ ! -f "$LEDGER" ] || { echo "    a refused recording wrote $LEDGER"; exit 1; }

# ---------------------------------------------------------------- 2. a run, recorded
# Proves `evidence-proof-is-an-execution`: what makes the claim proven below is this
# recorded execution and the commit it ran against, not the fact that its `test:` path
# resolves — section 1 already showed the path resolving and the answer was `not_run`.
printf '01_alpha\tok\t12\tparallel\n02_beta\tok\t7\texclusive\n' > "$W/all.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --origin ci
expect_grep 'recorded +2 execution\(s\), 2 passing'
expect_file "$LEDGER"
# Recording writes the ledger, so the tree is never clean afterwards — and the ledger is
# tracked, so it cannot be wished away. The invariant is that the ledger is the ONLY thing
# recording touched, and that its own row does not age the proof it records: without that
# exclusion `proven` would be unreachable by construction, because recording dirties the
# tree and committing the record moves HEAD past the commit the record names.
DIRT="$(git status --porcelain | awk '{print $NF}' | grep -v '^\.ai/repo/evidence/' || true)"
[ -z "$DIRT" ] || { echo "    recording touched something other than the ledger:"; printf '      %s\n' $DIRT; exit 1; }

ev proven show
# The tree IS dirty here — recording just wrote the ledger — and every claim is still
# `proven`. That is the point: the ledger is evidence about the tree, not part of what the
# tests measure, so its own row does not age the proof it records. Assert both halves, or a
# regression that made `proven` depend on a clean tree again would pass unnoticed.
jqe proven "(.head == \"$C1\") and .working_tree == \"dirty\"" \
  "the report does not read HEAD and the tree state it derives from"
jqe proven '(.claims | length) == 5 and ([.claims[] | select(.test != null) | select(.state != "proven")] | length) == 0' \
  "a passing run is not reported as proven, though nothing but the ledger has changed since it"
ev alpha claim alpha-holds
jqe alpha '.state == "proven"' "the claim of a passing run at HEAD is not proven"
jqe alpha ".execution.commit == \"$C1\"" "the recorded execution does not carry the commit it ran against"
jqe alpha '.execution.outcome == "pass"' "the recorded execution does not carry the outcome"
jqe alpha '.execution.seconds == 12' "the recorded execution does not carry the duration the runner measured"
jqe alpha '.execution.origin == "ci"' "the recorded execution does not carry the origin it was recorded with"
jqe alpha '.execution.working_tree == "clean"' "the recorded execution does not carry the state of the tree it measured"
jqe alpha '(.execution.digest | startswith("sha256:")) and (.execution.digest | length) == 71' \
  "the recorded execution does not carry a digest of the test's own source"
jqe alpha '.reproduce == "bash test/run.sh 01_alpha"' "the claim does not say how to produce the proof again"

# 6b. every guaranteed claim is supported, so the gate is silent
expect_exit 0 "$MJB" evidence --repo "$T" show --check

# ---------------------------------------------------------------- 7. both directions
# Proves `evidence-navigates-both-ways`: one derivation, read from either end — a test
# lists the claims it proves and each of those claims names the test back.
# A relation that can only be walked one way is half a relation.
ev t1 proves suite:01_alpha
jqe t1 '.test == "suite:01_alpha" and .present == true and .digest_matches == true' \
  "the test does not resolve to its own identity, source and digest"
jqe t1 '[.proves[].id] | sort == ["alpha-also", "alpha-holds"]' \
  "the test does not list every claim that names it"
jqe t1 '[.proves[] | select(.test != "suite:01_alpha")] | length == 0' \
  "a claim the test lists does not name that test back"
# the path a claim writes and the identity the ledger keeps are the same test
ev t1p proves test/cases/01_alpha.sh
jqe t1p '.test == "suite:01_alpha"' "the path a claim names does not resolve to the test's identity"
jqe alpha '.test == "suite:01_alpha" and (.also_proves == ["alpha-also"])' \
  "the claim does not name its test, or does not name the other claims the same test proves"
ev t2 proves suite:02_beta
jqe t2 '[.proves[].id] == ["beta-holds"]' "the second test claims the wrong claims"

# ---------------------------------------------------------------- 3. a partial second run
# Proves `evidence-partial-run-preserves-the-rest`: the second report names one test, and
# every other claim's evidence must survive it unchanged.
# The regression that matters most: recording one case must not delete the evidence for
# every case the run did not include.
printf '01_alpha\tok\t3\tserial\n' > "$W/one.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/one.tsv" --origin local
expect_grep 'recorded +1 execution\(s\), 1 passing'
ev partial show
jqe partial '.ledger.executions == 2' \
  "a partial run changed how many executions the ledger holds: it erased evidence it never measured"
jqe partial '(.claims[] | select(.id == "alpha-holds") | .execution | .seconds == 3 and .origin == "local")' \
  "the partial run did not replace the evidence for the test it did name"
jqe partial '(.claims[] | select(.id == "beta-holds") | .execution | .seconds == 7 and .origin == "ci" and .outcome == "pass")' \
  "recording one case destroyed another case's evidence"
jqe partial '[.claims[] | select(.test != null) | select(.state != "proven")] | length == 0' \
  "a partial run left the tests it did not name unproven"

# ---------------------------------------------------------------- 5. proven vs unchanged
# Proves `evidence-currency-is-not-collapsed`: `proven` and `inputs_unchanged` are two
# states here and never one, because a run recorded against this commit says more than a
# run whose inputs merely have not changed since.
# One commit that changes nothing any claim names. The run still stands — but it was not
# made against this commit, and saying `proven` here would be the green badge whose
# derivation cannot be inspected.
printf 'an unrelated note\n' > unrelated.txt
git add -A >/dev/null && git commit -qm unrelated
C2="$(git rev-parse HEAD)"
[ "$C1" != "$C2" ] || { echo "    the unrelated commit did not move HEAD"; exit 1; }
ev moved show
jqe moved "(.head == \"$C2\")" "the report did not follow HEAD"
jqe moved '[.claims[] | select(.state == "proven")] | length == 0' \
  "a run recorded against an earlier commit is still reported as proven at HEAD"
jqe moved '(.claims | length) == 5 and ([.claims[] | select(.test != null) | select(.state != "inputs_unchanged")] | length) == 0' \
  "a passing run whose claim names nothing that changed is not reported as inputs_unchanged"
jqe moved '(.claims[] | select(.id == "alpha-holds") | .changed // []) == []' \
  "inputs_unchanged named a changed file, which would make it stale"
expect_exit 0 "$MJB" evidence --repo "$T" show --state inputs_unchanged
# and it says what it means: the absence of a known invalidation, not a proof at HEAD
ev meaning claim alpha-holds
jqe meaning '.meaning | test("absence of a known invalidation")' \
  "inputs_unchanged does not explain itself as the absence of a known invalidation"
# the state is not shown as the strongest one anywhere a person reads it
expect_exit 0 "$MJB" evidence --repo "$T" claim alpha-holds
expect_grep '^state +inputs_unchanged$'

# ---------------------------------------------------------------- 4. the test moves on
# The proof is older than its subject, and the report says which file made it so.
printf '# the alpha case, rewritten\n' > test/cases/01_alpha.sh
ev stale show
jqe stale '(.claims[] | select(.id == "alpha-holds") | .state) == "stale"' \
  "a claim whose test changed after the run is not reported as stale"
jqe stale '(.claims[] | select(.id == "alpha-holds") | .changed) == ["test/cases/01_alpha.sh"]' \
  "the stale claim does not name the file that changed under it"
jqe stale '(.claims[] | select(.id == "alpha-also") | .state) == "stale"' \
  "the second claim of the same test did not go stale with it"
jqe stale '(.claims[] | select(.id == "beta-holds") | .state) == "inputs_unchanged"' \
  "an edit to one test made an unrelated claim stale"
expect_exit 0 "$MJB" evidence --repo "$T" proves suite:01_alpha
expect_grep 'DOES NOT match: the test has changed since this run'
git checkout -q -- test/cases/01_alpha.sh

# a change to the implementation the claim names does the same: the derivation is over what
# the claim names, not over the test alone
printf '# changed\n' >> lib/beta.sh
ev stale2 claim beta-holds
jqe stale2 '.state == "stale" and (.changed == ["lib/beta.sh"])' \
  "a claim whose implementation changed after the run is not stale, or does not name it"
git checkout -q -- lib/beta.sh

# ---------------------------------------------------------------- 6. a failing result
# Proves `evidence-unsupported-guarantee-is-reported`: a guarantee no recorded run
# supports is named by the gate — `show --check` exits 10 — rather than still displayed
# as guaranteed.
# Nothing here re-judges a run: a case that failed is a case the runner said failed.
printf '02_beta\tFAIL\t9\tserial\n' > "$W/fail.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/fail.tsv"
expect_grep 'recorded +1 execution\(s\), 0 passing'
ev failing show
jqe failing '(.claims[] | select(.id == "beta-holds") | .state) == "failing"' \
  "the latest run of a claim's test failed and the claim is not reported as failing"
jqe failing '(.claims[] | select(.id == "beta-holds") | .execution.outcome) == "fail"' \
  "the failing execution is not carried with the claim"
jqe failing '[.findings[].claim] == ["beta-holds"]' \
  "a guarantee whose test most recently failed is not a finding, or something else became one"
jqe failing '(.findings[0].reproduce) == "bash test/run.sh 02_beta"' \
  "the finding does not say what to run to settle it"
expect_exit 10 "$MJB" evidence --repo "$T" show --check
expect_grep 'claim\(s\) declare a guarantee the evidence does not support'
# the filters narrow the claims and never the tally: an answer that showed one claim and
# counted one claim would misreport how much of the matrix was examined
ev only_failing show --state failing
jqe only_failing '[.claims[].id] == ["beta-holds"] and (.claims | length) == 1' \
  "--state did not narrow the answer to that state"
jqe only_failing '([.totals[]] | add) == 5' \
  "a filtered answer tallies only what it returned, not the whole matrix"

# a claim the matrix does not declare is a not-found, never an empty answer that would read
# as "this claim has no evidence"
expect_exit 13 "$MJB" evidence --repo "$T" claim no-such-claim
expect_grep 'is not a claim of docs/CLAIMS\.yaml'

# ---------------------------------------------------------------- 8b. a result for nothing
# A report naming a case this repository does not have is named, not recorded: an execution
# of a test that does not exist is evidence for nothing, and dropping it silently would
# hide a runner and a matrix that have diverged.
printf '02_beta\tok\t4\tparallel\n99_ghost\tok\t1\tparallel\n' > "$W/ghost.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/ghost.tsv"
expect_grep '99_ghost'
expect_grep 'no such test here'
expect_grep 'recorded +1 execution\(s\)'
expect_no_grep '99_ghost' "$LEDGER"
ev ghost proves suite:99_ghost
jqe ghost '.present == false and (has("execution") | not) and (.proves == [])' \
  "a test nothing recorded and nothing claims reports an execution or a claim"
ev after_ghost show
jqe after_ghost '.ledger.executions == 2' "the ghost result reached the ledger after all"
jqe after_ghost '(.claims[] | select(.id == "beta-holds") | .state) == "proven"' \
  "the run that followed a failure did not replace it"

# ---------------------------------------------------------------- 9. the ratchet
# `scripts/evidence-check` renders the answer above and ratchets it against
# .ai/repo/evidence-baseline.txt. What the ratchet refuses is the whole question: its first
# predicate was membership — an unsupported claim absent from the baseline list is a
# regression — and membership cannot tell a claim that LOST its proof from a claim written
# after the list was, because both are absent from a snapshot of the past. So every newly
# merged guarantee reddened the trunk for evidence it had never had. The baseline now
# records the state per claim, both halves, and three behaviours are asserted here:
#   a  a claim that arrives already supported does not fire, and neither does one that
#      arrives unsupported — that is new debt, named as such, and not a regression
#   b  a claim the baseline recorded as supported, and that no longer is, DOES fire
#   c  a supported guarantee leaving the matrix DOES fire, because nothing became unproven
#      and yet the repository proves less: the denominator shrank instead of the debt
EC="$ROOT/scripts/evidence-check"
BL="$T/.ai/repo/evidence-baseline.txt"
export MAJORDOMUS_BIN="$MJB"
claims_matrix() {  # claims_matrix <extra yaml on stdin> — the fixture matrix, rewritten
  { printf 'version: 1\nclaims:\n'; cat; } > docs/CLAIMS.yaml
  git add -A >/dev/null && git commit -qm matrix
}
BASE_CLAIMS='  - id: alpha-holds
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
'

# 9a. the baseline, over a matrix whose two guarantees both carry a passing run
printf '01_alpha\tok\t2\tparallel\n02_beta\tok\t3\tparallel\n' > "$W/ratchet.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/ratchet.tsv"
printf '%s' "$BASE_CLAIMS" | claims_matrix
expect_exit 0 bash "$EC" --repo "$T" --baseline
expect_grep 'baseline written, 2 supported and 0 unsupported'
expect_grep '^\+alpha-holds$' "$BL"
expect_grep '^\+beta-holds$' "$BL"
expect_exit 0 bash "$EC" --repo "$T"
expect_grep '2 supported, 0 unsupported'

# 9b (case a, first half). A claim written after the baseline, naming a test that already
# has a passing run. It is absent from the baseline for the only reason it could be, and it
# is supported. Nothing fires.
{ printf '%s' "$BASE_CLAIMS"; printf '  - id: epsilon-holds
    claim: Epsilon holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/01_alpha.sh
    status: guaranteed
'; } | claims_matrix
expect_exit 0 bash "$EC" --repo "$T"
expect_no_grep 'lost the evidence'
expect_grep '3 supported, 0 unsupported'

# 9c (case a, second half — the defect that reddened the trunk). A claim written after the
# baseline naming a test no run has ever covered. Under membership this was "lost the
# evidence that supported them", of evidence it never had, and exit 10 on every branch that
# merged after it. It is new debt: named, counted, and not a regression.
printf '# the eta case\n' > test/cases/04_eta.sh
{ printf '%s' "$BASE_CLAIMS"; printf '  - id: eta-holds
    claim: Eta holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/04_eta.sh
    status: guaranteed
'; } | claims_matrix
expect_exit 0 bash "$EC" --repo "$T"
expect_grep '1 guaranteed claim\(s\) the baseline does not know'
expect_grep 'New debt, not a regression'
expect_grep 'eta-holds'
expect_no_grep 'lost the evidence'

# 9d (case b). A claim the baseline recorded as supported, whose test most recently failed.
# It had a proof and does not now. This is the regression the gate exists for, and it is
# the one membership could never see: a claim losing its proof joins the unsupported set
# exactly as a new claim does.
printf '02_beta\tFAIL\t9\tserial\n' > "$W/ratchet-fail.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/ratchet-fail.tsv"
expect_exit 10 bash "$EC" --repo "$T"
expect_grep '1 guaranteed claim\(s\) lost the evidence that supported them'
expect_grep 'beta-holds'
# and the remedy the gate names is a run, not an edit to the list
expect_grep 'reproduce: bash test/run\.sh 02_beta'

# 9e (case c). Beta is put back, and then the guarantee is deleted from the matrix while an
# unproven one takes its place. No claim lost anything — beta-holds is not a finding, it is
# not a claim — and the repository nonetheless proves one guarantee fewer than the baseline
# records while carrying one more unproven. A ratchet that watched only names would call
# this clean, which is how a ratchet rots without ever going red.
printf '02_beta\tok\t3\tserial\n' > "$W/ratchet-pass.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/ratchet-pass.tsv"
expect_exit 0 bash "$EC" --repo "$T"
printf '# the zeta case\n' > test/cases/03_zeta.sh
{ printf '  - id: alpha-holds
    claim: Alpha holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/01_alpha.sh
    status: guaranteed
  - id: zeta-holds
    claim: Zeta holds
    source: docs/BETA.md
    implementation: lib/beta.sh
    test: test/cases/03_zeta.sh
    status: guaranteed
'; } | claims_matrix
expect_exit 10 bash "$EC" --repo "$T"
expect_grep '1 supported guarantee\(s\) left the matrix'
expect_grep 'beta-holds'
expect_no_grep 'lost the evidence'

# 9f. A baseline written before the gate learned the difference carries bare lines only. It
# says which guarantees were unsupported and nothing about which were supported, so the only
# honest reading is that nothing is recorded as supported: permissive, and never a crash or
# a false regression against a file that cannot answer the question.
printf '%s' "$BASE_CLAIMS" | claims_matrix
{ printf '# old format\n'; printf 'alpha-holds\nbeta-holds\n'; } > "$BL"
expect_exit 0 bash "$EC" --repo "$T"
expect_no_grep 'lost the evidence'
expect_no_grep 'left the matrix'
# and rewriting it restores both halves
expect_exit 0 bash "$EC" --repo "$T" --baseline
expect_grep '^\+alpha-holds$' "$BL"

# ---------------------------------------------------------------- reading changes nothing
before="$(git status --porcelain)"
"$MJB" evidence --repo "$T" show >/dev/null 2>&1 || true
"$MJB" evidence --repo "$T" claim alpha-holds >/dev/null 2>&1 || true
"$MJB" evidence --repo "$T" proves suite:01_alpha >/dev/null 2>&1 || true
[ "$(git status --porcelain)" = "$before" ] \
  || { echo "    reading the evidence changed the working tree"; git status --porcelain; exit 1; }
exit 0
