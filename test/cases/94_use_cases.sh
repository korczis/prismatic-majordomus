# majordomus-covers: usecase
# majordomus-negative: usecase
# claims: use-case-coverage, use-case-evidence, use-case-impact
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

# --- one use case: a file with a scenario section, nothing else to register
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
---

# Situation

Which tool is this?

# Scenario

```yaml
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
```

# Outcome

The version, from anywhere.
MD
# the repository has no claims file or fixtures of its own: the claim resolves against the
# distribution's? No — a use case names what this repository declares; give it a claims
# file and let the distribution's fixtures (setup scripts) serve the scenario
mkdir -p docs
printf 'version: 1\nstatuses:\n  - id: guaranteed\n    meaning: Deterministic.\nclaims:\n  - id: exit-code-contract\n    claim: Exit codes mean one thing\n    source: docs/CLI.md\n    implementation: bin/majordomus\n    test: test/cases/00.sh\n    status: guaranteed\n    responsibility: none\n' > docs/CLAIMS.yaml
# a guaranteed claim of one of the tool's responsibilities, and an MCP tool the executable's
# registry projects: coverage counts both, and neither is named by any use case yet
printf '  - id: health-is-reported\n    claim: Doctor reports the health\n    source: docs/CLI.md\n    implementation: lib/doctor.sh\n    test: test/cases/00.sh\n    status: guaranteed\n    responsibility: doctor\n' >> docs/CLAIMS.yaml
mkdir -p docs/generated
printf '{"capabilities":[{"id":"status.read","tool": "majordomus_status"}]}\n' > docs/generated/registry.json
expect_exit 0 "$MJ" usecase coverage
expect_grep '^claim +health-is-reported +0 +0 +0 +gap +advisory$'
expect_grep '^mcp_tool +majordomus_status +0 +0 +0 +gap +advisory$'
# the claim with responsibility none is not a coverage target at all
expect_no_grep 'exit-code-contract'
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
sed -i.bak "s/^      exit: 2$/      exit: 0/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
expect_exit 10 "$MJ" usecase run see-the-version
expect_grep 'expected exit 0, got 2'
git checkout -q -- "$UC/see-the-version.md"

# --- and the same failure through --json, which is the form every generator reads.
# The count of failures is a fact about the run, not about how it is printed. This lived
# only in the text branch once, so `--json` reported "failed":0 and exited 0 however the
# scenarios went — and scripts/generate-site-data relies on exactly that exit code to
# refuse to publish a demonstration that does not hold. A green exit there did not stop
# the site; it silently lowered the use case's maturity instead, which is the difference
# between knowing something is broken and not knowing anything about it.
sed -i.bak "s/stdout_contains: \['unknown option'\]/stdout_contains: ['everything is fine']/" "$UC/see-the-version.md"; rm -f "$UC/see-the-version.md.bak"
expect_exit 10 "$MJ" usecase run --json see-the-version
"$MJ" usecase run --json see-the-version > run.json 2>/dev/null || true
jq -e '.failed == 1 and .ran == 1' run.json >/dev/null \
  || { echo "    the JSON run does not report its own failure"; cat run.json; exit 1; }
jq -e '[.results[] | select(.result != "pass")] | length == 1' run.json >/dev/null \
  || { echo "    the failing scenario is not in the JSON results"; cat run.json; exit 1; }
git checkout -q -- "$UC/see-the-version.md"
# a passing run still exits 0 and says so, so the fix did not make --json fail always
expect_exit 0 "$MJ" usecase run --json see-the-version
"$MJ" usecase run --json see-the-version > run.json 2>/dev/null
jq -e '.failed == 0 and .ran == 1 and (.results[0].result == "pass")' run.json >/dev/null \
  || { echo "    a passing scenario is not reported as passing in JSON"; cat run.json; exit 1; }

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
probe "unknown setup"     "s/^setup: bare/setup: nosuch/"                                "names setup 'nosuch'"
probe "step not listed"   "s/run: \['version', '--no-such-option'\]/run: ['doctor']/"        "step 'refuse' runs 'doctor', which the use case does not list under commands"
probe "unknown key"       "s/^difficulty: basic/difficulty: basic\ncolour: red/"             "unknown key.s.: colour"
probe "no situation"      "s/^# Situation/# Setting/"                                        "body has no '# Situation' heading"
probe "unknown application" "s/^applications: \[\]/applications: [nosuch-app]/"             "names application 'nosuch-app', which does not exist"
# a guaranteed target with no scenario is refused: a guarantee needs executable evidence
awk '/^# Scenario$/{skip=1} /^# Outcome$/{skip=0} !skip' "$UC/see-the-version.md" > "$UC/x.md" && mv "$UC/x.md" "$UC/see-the-version.md"
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
# check applies the same gate as doctor: a required gap fails it with the scaffold named
expect_exit 10 "$MJ" check
expect_grep 'FAIL use-case +command doctor — gap: 0 use case\(s\) name it, 0 run it; the policy requires an executable use case'
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
# the claim and the MCP tool are still named gaps: nothing active names them
expect_grep '^claim +health-is-reported +0 +0 +0 +gap +advisory$'
expect_grep '^mcp_tool +majordomus_status +0 +0 +0 +gap +advisory$'
# naming them from the executable use case closes both, and the tally says so
sed -i.bak 's/^claims: \[\]$/claims: [health-is-reported]\nmcp_tools: [majordomus_status]/' "$UC/doctor-draft.md"; rm -f "$UC/doctor-draft.md.bak"
expect_exit 0 "$MJ" usecase validate
expect_exit 0 "$MJ" usecase coverage
expect_grep '^claim +health-is-reported +1 +1 +1 +covered +advisory$'
expect_grep '^mcp_tool +majordomus_status +1 +1 +1 +covered +advisory$'
# the policy gates the other classes too: a claim gap under claims: required fails --check
sed -i.bak 's/^claims: \[health-is-reported\]$/claims: []/' "$UC/doctor-draft.md"; rm -f "$UC/doctor-draft.md.bak"
cp .ai/repo/policy.yaml "$T/policy.required"
sed -i.bak 's/^    commands: required$/    commands: advisory/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 0 "$MJ" usecase coverage --check
expect_grep '^claim +health-is-reported +0 +0 +0 +gap +advisory$'
sed -i.bak 's/^    claims: advisory$/    claims: required/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$MJ" usecase coverage --check
expect_grep '^claim +health-is-reported +0 +0 +0 +gap +required$'
expect_grep ', 1 required gap\(s\)'
cp "$T/policy.required" .ai/repo/policy.yaml
sed -i.bak 's/^claims: \[\]$/claims: [health-is-reported]/' "$UC/doctor-draft.md"; rm -f "$UC/doctor-draft.md.bak"

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
# an implementation file names its command, the behavioural cases that declare they cover
# it, and through the command the use cases and scenarios that run it
mkdir -p lib test/cases
printf '# the doctor implementation of this fixture\n' > lib/doctor.sh
printf '# majordomus-covers: doctor\n' > test/cases/01_doctor.sh
printf '# majordomus-covers: version\n' > test/cases/02_version.sh
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "an implementation and its cases"
printf '# touched\n' >> lib/doctor.sh
expect_exit 0 "$MJ" usecase impact --base HEAD
expect_grep '^  commands   doctor$'
expect_grep '^  cases      test/cases/01_doctor\.sh$'
expect_grep '^  use cases  doctor-draft$'
expect_grep '^  scenarios  doctor-draft$'
expect_exit 0 "$MJ" usecase impact --base HEAD --json
expect_grep '"commands":\["doctor"\],"rules":\[\],"use_cases":\["doctor-draft"\],"scenarios":\["doctor-draft"\],"cases":\["test/cases/01_doctor\.sh"\]'
git checkout -q -- lib/doctor.sh
# a rule file names the rule by its identity, read from its front matter, not its path
RULE=.ai/repo/rules/vendor/majordomus/rules/use-case-coverage.v1.md
[ -f "$RULE" ] || { echo "    the fixture has no vendored use-case-coverage rule"; exit 1; }
printf '\n' >> "$RULE"
expect_exit 0 "$MJ" usecase impact --base HEAD
expect_grep '^  rules      majordomus\.use-case-coverage$'
expect_grep '^  commands   none$'
# ...and the behavioural case the rule names as its proof is affected by changing the rule
expect_grep '^  cases      test/cases/94_use_cases\.sh$'
git checkout -q -- "$RULE"

# --- the evidence the site shows is the evidence the tool recorded. The site generator runs
#     in a scratch copy of this tool's tree (never the real site/), with the catalogue pruned
#     to the use cases whose scenarios walk the lifecycle and refuse something, which is the
#     least the generator accepts; it executes them itself and embeds what they recorded.
command -v jq >/dev/null || { echo "    jq absent; the site half is not asserted"; exit 1; }
SF="$T/site-fixture"
fixture_repo "$SF" AGENTS.md docs site/data/marketing.toml site/content-src
KEEP='know-which-tool-is-running hand-work-between-sessions run-several-workers-at-once carry-a-blocker-across-a-handover'
for f in "$SF"/.ai/repo/use-cases/*.md "$SF"/.ai/repo/applications/*.md; do
  grep -q '^kind: context$' "$f" && continue
  case " $KEEP " in *" $(basename "$f" .md) "*) sed -i.bak 's/^applications: \[.*\]$/applications: []/' "$f"; rm -f "$f.bak" ;; *) rm -f "$f" ;; esac
done
( cd "$SF" && git init -q && git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture ) || exit 1
expect_exit 0 "$SF/scripts/generate-site-data" --out "$T/site-data" \
  || { echo "    the site generator refused the pruned catalogue"; exit 1; }
CAT="$T/site-data/catalogue.json"
expect_file "$CAT"
for u in $KEEP; do
  EVF="$SF/.ai/local/evidence/use-cases/$u.json"
  expect_file "$EVF" || { echo "    the generator did not execute the scenario of $u"; exit 1; }
  jq -S --arg u "$u" '.use_cases[] | select(.id == $u) | .evidence' "$CAT" > "$T/shown.json"
  jq -S 'del(.steps[].timing)' "$EVF" > "$T/recorded.json"
  cmp -s "$T/shown.json" "$T/recorded.json" \
    || { echo "    the site does not show the evidence the tool recorded for $u"; diff "$T/recorded.json" "$T/shown.json" | head -20; exit 1; }
done
# and that evidence is the executed scenario, step by step: exit and output as observed
jq -e '.use_cases[] | select(.id == "know-which-tool-is-running") | .evidence
       | .result == "pass" and (.steps | length) == 2
         and .steps[0].exit == 0 and (.steps[0].output | test("^majordomus [0-9]+\\.[0-9]+\\.[0-9]+$"))
         and .steps[1].exit == 2 and .steps[1].expected_exit == 2 and (.steps[1].output | test("unknown option"))
         and all(.steps[]; .result == "pass" and (.assertions | length) > 0)' "$CAT" >/dev/null \
  || { echo "    the site's evidence is not the executed scenario"; jq '.use_cases[] | select(.id == "know-which-tool-is-running") | .evidence' "$CAT"; exit 1; }
grep -q "$T" "$CAT" && { echo "    the site data leaks the case directory path"; exit 1; }
# the page carries the scenario it demonstrates, and maturity is observed from it: the scenario
# lives in the body's `# Scenario` section, and a generator that read only the front matter
# showed every use case of this repository as `described`, with no steps, while
# `usecase run` executed the same scenarios and passed them
for u in $KEEP; do
  jq -e --arg u "$u" '.use_cases[] | select(.id == $u)
         | (.scenario.steps | length) > 0 and .maturity != "described"' "$CAT" >/dev/null \
    || { echo "    the site shows $u without the scenario it runs"; jq --arg u "$u" '.use_cases[] | select(.id == $u) | {maturity, scenario}' "$CAT"; exit 1; }
done

# --- every reference the migrated catalogue of this tree makes resolves, and its scenarios run:
#     the repository's own use cases against its own tool (dogfooding), one of them at least
( cd "$ROOT" && expect_exit 0 "$MJ" usecase validate && expect_grep 'usecase validate: 0 failure' ) || exit 1
( cd "$ROOT" && expect_exit 0 "$MJ" usecase run know-which-tool-is-running && expect_grep '^know-which-tool-is-running +pass' ) || exit 1
( cd "$ROOT" && expect_exit 0 "$MJ" usecase coverage --check ) || exit 1
