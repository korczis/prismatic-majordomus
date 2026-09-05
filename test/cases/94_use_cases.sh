# majordomus-covers: usecase
# majordomus-negative: usecase
# The executable use-case system, in a repository `init` wrote: the sections exist with the
# taxonomy; a use case is one file, discovered, validated against everything it names, run
# against the real tool with the evidence recorded and normalised; coverage is computed
# from the registry and gated by the policy; a draft scaffolded for a gap runs but never
# counts; impact traces a changed file to the use cases and cases it affects; and every
# broken reference, failed step and stale expectation is refused with the entity named.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null && git commit -qm install
UC=.ai/repo/use-cases

# --- the sections are seeded and empty; coverage is advisory in a fresh policy
[ -f "$UC/taxonomy.yaml" ] || { echo "    init seeded no taxonomy"; exit 1; }
[ -f .ai/repo/applications/README.md ] || { echo "    init seeded no applications section"; exit 1; }
expect_exit 0 "$MJ" usecase list
expect_grep '^use cases: 0 in \.ai/repo/use-cases$'
expect_exit 0 "$MJ" usecase coverage
expect_grep '^command +doctor +0 +0 +0 +gap +advisory$'
expect_grep 'required gap\(s\)'
expect_exit 0 "$MJ" usecase coverage --check
expect_exit 2 "$MJ" usecase bogus
expect_grep "unknown subcommand 'bogus'"
expect_exit 12 "$MJ" usecase show nosuch
expect_grep "no use case 'nosuch'"

# --- one use case: a file with a scenario, nothing else to register
cat > "$UC/see-the-version.md" <<'MD'
---
id: see-the-version
kind: use-case
title: 'See which version runs'
summary: 'Print the version and refuse an unknown option.'
category: adoption
status: active
target: guaranteed
actors: [operator]
difficulty: basic
commands: [version]
doctrines: []
claims: [exit-code-contract]
responsibilities: []
applications: []
scenario:
  setup: bare
  given:
    - 'nothing installed'
  steps:
    - id: print
      run: ['version']
      note: 'the version string'
      expect:
        exit: 0
        stdout_contains: ['^majordomus [0-9]+\.[0-9]+\.[0-9]+$']
    - id: refuse
      run: ['version', '--no-such-option']
      note: 'a usage error'
      expect:
        exit: 2
        stdout_contains: ['unknown option']
  then:
    - 'the version needs no repository'
---

# Situation

Which tool is this?

# Outcome

The version, from anywhere.
MD
# the repository has no claims file or fixtures of its own: the claim resolves against the
# distribution's? No — a use case names what this repository declares; give it a claims
# file and let the distribution's fixtures (setup scripts) serve the scenario
mkdir -p docs
printf 'version: 1\nstatuses:\n  - id: guaranteed\n    meaning: Deterministic.\nclaims:\n  - id: exit-code-contract\n    claim: Exit codes mean one thing\n    source: docs/CLI.md\n    implementation: bin/majordomus\n    test: test/cases/00.sh\n    status: guaranteed\n    responsibility: none\n' > docs/CLAIMS.yaml
git add -A >/dev/null && git commit -qm "one use case"
expect_exit 0 "$MJ" usecase validate
expect_grep 'usecase validate: 0 failure'
expect_exit 0 "$MJ" usecase list
expect_grep '^see-the-version +adoption +active +scenario +version'
expect_exit 0 "$MJ" usecase run see-the-version
expect_grep '^see-the-version +pass +2 step'
EV=.ai/local/state/../evidence/use-cases/see-the-version.json
[ -f "$EV" ] || { echo "    no evidence written"; exit 1; }
grep -q '"result":"pass"' "$EV" || { echo "    evidence does not record the pass"; cat "$EV"; exit 1; }
grep -q '"command":"majordomus version --no-such-option"' "$EV" || { echo "    evidence does not carry the command"; exit 1; }
grep -q '"expected_exit":2' "$EV" || { echo "    evidence does not carry the expected exit"; exit 1; }
# the evidence is normalised: no absolute path of the scenario repository, no timestamp
grep -q "$T" "$EV" && { echo "    evidence leaks the scenario path"; exit 1; }
expect_exit 0 "$MJ" usecase show see-the-version
expect_grep '^--- evidence: .*\(pass\)'
# the tracked tree is untouched by a run
[ -z "$(git status --porcelain)" ] || { echo "    usecase run changed the tree"; git status --porcelain; exit 1; }
# coverage moved
expect_exit 0 "$MJ" usecase coverage
expect_grep '^command +version +1 +1 +1 +covered'
expect_exit 0 "$MJ" usecase coverage --json
expect_grep '"id":"version","use_cases":1,"executable":1,"evidence":1,"status":"covered"'

# --- a failing expectation fails the run, naming the step and the reason; evidence says fail
sed -i.bak "s/stdout_contains: \['unknown option'\]/stdout_contains: ['everything is fine']/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
expect_exit 10 "$MJ" usecase run see-the-version
expect_grep '^see-the-version +FAIL +expected /everything is fine/'
grep -q '"result":"fail"' "$EV" || { echo "    evidence does not record the failure"; exit 1; }
git checkout -q -- "$UC/see-the-version.md"
sed -i.bak "s/        exit: 2$/        exit: 0/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
expect_exit 10 "$MJ" usecase run see-the-version
expect_grep 'expected exit 0, got 2'
git checkout -q -- "$UC/see-the-version.md"

# --- every broken reference is refused with the entity and the relation named
probe() { # description, sed expression, expected pattern
  sed -i.bak "$2" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
  expect_exit 10 "$MJ" usecase validate || { echo "    probe '$1' was not refused"; git checkout -q -- "$UC/see-the-version.md"; exit 1; }
  expect_grep "$3" || { echo "    probe '$1' failed for another reason"; git checkout -q -- "$UC/see-the-version.md"; exit 1; }
  git checkout -q -- "$UC/see-the-version.md"
}
probe "unknown command"   "s/^commands: \[version\]/commands: [version, nosuchcmd]/"        "names command 'nosuchcmd', which bin\/majordomus does not dispatch"
probe "unknown doctrine"  "s/^doctrines: \[\]/doctrines: [majordomus.nosuch]/"              "names doctrine 'majordomus.nosuch', which no rule declares"
probe "unknown claim"     "s/^claims: \[exit-code-contract\]/claims: [nosuch-claim]/"       "names claim 'nosuch-claim'"
probe "unknown category"  "s/^category: adoption/category: nosuch/"                          "category 'nosuch' is not in taxonomy.yaml"
probe "unknown setup"     "s/^  setup: bare/  setup: nosuch/"                                "names setup 'nosuch'"
probe "step not listed"   "s/run: \['version', '--no-such-option'\]/run: ['doctor']/"        "step 'refuse' runs 'doctor', which the use case does not list under commands"
probe "unknown key"       "s/^difficulty: basic/difficulty: basic\ncolour: red/"             "unknown key.s.: colour"
probe "no situation"      "s/^# Situation/# Setting/"                                        "body has no '# Situation' heading"
probe "unknown application" "s/^applications: \[\]/applications: [nosuch-app]/"             "names application 'nosuch-app', which does not exist"
# a guaranteed target with no scenario is refused: a guarantee needs executable evidence
awk '/^scenario:/{skip=1} /^---$/ && NR>1 {skip=0} !skip' "$UC/see-the-version.md" > "$UC/x.md" && mv "$UC/x.md" "$UC/see-the-version.md"
expect_exit 10 "$MJ" usecase validate
expect_grep 'targets guaranteed and has no scenario'
git checkout -q -- "$UC/see-the-version.md"
# the id is the file name, and a duplicate id is refused
cp "$UC/see-the-version.md" "$UC/copy.md"
expect_exit 10 "$MJ" usecase validate
expect_grep "id 'see-the-version' is not the file name"
expect_grep "declared twice"
rm -f "$UC/copy.md"
# doctor applies the same validation under the catalogue doctrine, and coverage under its own
expect_exit 0 "$MJ" usecase validate
sed -i.bak "s/^category: adoption/category: nosuch/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -q "FAIL use-case    see-the-version — category 'nosuch'" "$T/doctor.out" || { echo "    doctor does not apply the use-case validation"; grep 'use-case\|catalogue' "$T/doctor.out"; exit 1; }
git checkout -q -- "$UC/see-the-version.md"

# --- an application must name its use cases back, and a use case its applications
cat > .ai/repo/applications/solo.md <<'MD'
---
id: solo
kind: application
title: 'Solo'
summary: 'One person.'
status: active
fits_when:
  - 'one person'
does_not_fit_when:
  - 'a team'
use_cases: [see-the-version]
doctrines: []
responsibilities: []
---

# Context

One person and one repository.
MD
expect_exit 10 "$MJ" usecase validate
expect_grep "application names use case 'see-the-version', which does not name it back"
sed -i.bak "s/^applications: \[\]/applications: [solo]/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
expect_exit 0 "$MJ" usecase validate
git add -A >/dev/null && git commit -qm "an application"

# --- the policy gates coverage: required turns a gap into a failure of doctor, check and finish
sed -i.bak 's/^    commands: advisory$/    commands: required/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 10 "$MJ" usecase coverage --check
expect_grep 'required gap\(s\)'
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -q 'FAIL use-case    command doctor — gap: 0 use case(s) name it, 0 run it; the policy requires an executable use case  \[reproduce: majordomus usecase scaffold --for command:doctor\]' "$T/doctor.out" \
  || { echo "    doctor does not fail a required gap with the scaffold named"; tail -5 "$T/doctor.out"; exit 1; }
# the finish contract carries the key once the policy lists it
sed -i.bak 's/^    - note_present$/    - note_present\n    - use_cases_covered/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "require coverage"
"$MJ" start "cover it" --scope lib >/dev/null
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 10 "$MJ" finish --outcome completed --verify-command true
expect_grep 'FAIL use-case'
expect_grep 'refused'

# --- a scaffold closes nothing by itself: it is a draft that runs and does not count
expect_exit 0 "$MJ" usecase scaffold --for command:doctor --dry-run
expect_grep 'would write: \.ai/repo/use-cases/doctor-draft\.md'
[ ! -f "$UC/doctor-draft.md" ] || { echo "    --dry-run wrote"; exit 1; }
expect_exit 0 "$MJ" usecase scaffold --for command:doctor
expect_grep 'wrote: \.ai/repo/use-cases/doctor-draft\.md'
grep -q '^status: draft$' "$UC/doctor-draft.md" || { echo "    the scaffold is not a draft"; exit 1; }
grep -q '^target: advisory$' "$UC/doctor-draft.md" || { echo "    the scaffold targets a guarantee"; exit 1; }
grep -q '^commands: \[doctor\]$' "$UC/doctor-draft.md" || { echo "    the scaffold does not name its command"; exit 1; }
expect_exit 0 "$MJ" usecase validate
expect_exit 0 "$MJ" usecase run doctor-draft
expect_grep '^doctor-draft +pass'
expect_exit 10 "$MJ" usecase coverage --check
expect_grep '^command +doctor +0 +0 +0 +gap +required'
expect_exit 0 "$MJ" usecase scaffold --for command:doctor
expect_grep '^exists: '
expect_exit 0 "$MJ" usecase scaffold --missing --dry-run
expect_grep 'would write: \.ai/repo/use-cases/init-draft\.md'
# made active, with the narrative and its claims, it counts
sed -i.bak 's/^status: draft$/status: active/; s/^title: .*/title: '"'"'Check the health'"'"'/; s/^summary: .*/summary: '"'"'Doctor.'"'"'/' "$UC/doctor-draft.md"; rm -f "$UC/doctor-draft.md.bak"
sed -i.bak 's/^claims: \[.*\]$/claims: []/; s/^responsibilities: \[.*\]$/responsibilities: []/' "$UC/doctor-draft.md"; rm -f "$UC/doctor-draft.md.bak"
expect_exit 0 "$MJ" usecase validate
expect_exit 0 "$MJ" usecase coverage
expect_grep '^command +doctor +1 +1 +1 +covered +required'

# --- impact: a changed file names the use cases, scenarios and cases it reaches
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "doctor use case"
printf '\n# touched\n' >> "$UC/see-the-version.md"
expect_exit 0 "$MJ" usecase impact --base HEAD
expect_grep '^  use cases  see-the-version'
expect_grep '^  scenarios  see-the-version'
expect_grep '^next: majordomus usecase run see-the-version'
git checkout -q -- "$UC/see-the-version.md"
expect_exit 0 "$MJ" usecase impact --base HEAD --json
expect_grep '"use_cases":\[\]'

# --- every reference the migrated catalogue of this tree makes resolves, and its scenarios run:
#     the repository's own use cases against its own tool (dogfooding), one of them at least
( cd "$ROOT" && expect_exit 0 "$MJ" usecase validate && expect_grep 'usecase validate: 0 failure' ) || exit 1
( cd "$ROOT" && expect_exit 0 "$MJ" usecase run know-which-tool-is-running && expect_grep '^know-which-tool-is-running +pass' ) || exit 1
( cd "$ROOT" && expect_exit 0 "$MJ" usecase coverage --check ) || exit 1
