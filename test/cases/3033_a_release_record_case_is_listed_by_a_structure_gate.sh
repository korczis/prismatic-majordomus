# majordomus-covers: none
# A case that names the release record is run by the release-record-cases gate, and a
# structure gate says so when it is not (I2716, gate release-record-listing).
#
# The release-record-cases gate runs its cases by name, for the release record's pull request,
# which plans that gate and not the suite. On 2026-10-14 a branch added a case that named the
# release record and left it off the list; only case 94, in a suite shard, noticed.
# scripts/ci/release-record-cases-check now holds the one definition of such a case and asks
# the question in the structure job. This case plants each shape of the omission in a fixture
# tree and watches it refused, holds this checkout to the gate, and proves a change to a case
# plans the gate.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
GATE="$ROOT/scripts/ci/release-record-cases-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
MODEL="$ROOT/.ai/repo/ci/gates.yaml"

# ---------------------------------------------------------------- this checkout
expect_exit 0 "$GATE"
expect_grep 'case\(s\) name the release record, each run by release-record-cases'
# the gate is declared in the structure job, runs this script, and a changed case plans it
awk '/^  - id: release-record-listing$/{f=1; next} f && /^  - id: /{exit} f' "$MODEL" > decl.txt
grep -qx '    job: structure' decl.txt || { echo "    release-record-listing is not a structure gate"; exit 1; }
grep -qx '    runs: scripts/ci/release-record-cases-check' decl.txt \
  || { echo "    release-record-listing does not run scripts/ci/release-record-cases-check"; exit 1; }
printf 'test/cases/1_a_new_case.sh\n' | "$ROOT/scripts/ci-plan" --files - > plan.json
jq -e '.selected | index("release-record-listing")' plan.json >/dev/null \
  || { echo "    a change to a case does not plan release-record-listing: $(jq -c .selected plan.json)"; exit 1; }
# --wanted is the definition's answer, and the checkout's gate runs every name in it
runs="$(awk '/^  - id: release-record-cases$/{f=1} f && /^    runs: /{print; exit}' "$MODEL")"
"$GATE" --wanted > wanted.txt
[ -s wanted.txt ] || { echo "    --wanted selected no case in this checkout"; exit 1; }
while read -r c; do
  case "$runs " in *" $c "*) ;; *) echo "    --wanted names $c and the gate does not run it"; exit 1 ;; esac
done < wanted.txt

# ---------------------------------------------------------------- a fixture tree
F="$T/tree"
mkdir -p "$F/.ai/repo/ci" "$F/test/cases"
listing() { # <case names...>: the model, with the gate running exactly these
  printf 'version: 1\ngates:\n  - id: shell-suite\n    job: suite\n    runs: bash test/run.sh\n  - id: release-record-cases\n    job: rust\n    runs: MJ_TEST_JOBS=4 bash test/run.sh --no-skips %s\n    summary: the cases\n  - id: other\n    job: structure\n    runs: true\n' \
    "$*" > "$F/.ai/repo/ci/gates.yaml"
}
kase() { # <name> <line>: a case whose one line is given
  printf '# a case\n%s\n' "$2" > "$F/test/cases/$1.sh"
}
gate() { MJ_ROOT="$F" expect_exit "$1" "$GATE"; }

# the exempt cases exist, each naming the paths as fixtures, and are owed nothing
kase 94_ci_plan 'printf "%s\n" .ai/repo/releases/v1.yaml > rec.txt'
kase 3033_a_release_record_case_is_listed_by_a_structure_gate 'echo release-record-cases'
# two readers of the record, both listed; a case about something else is owed nothing
kase 10_schema 'jq . "$ROOT/.ai/repo/releases/v1.yaml"'
kase 11_pipeline 'grep -q tag scripts/release-record'
kase 12_unrelated 'echo docs/DESIGN.md'
listing 10_schema 11_pipeline
gate 0
expect_grep '2 case\(s\) name the release record, each run by release-record-cases'
MJ_ROOT="$F" "$GATE" --wanted > fw.txt
[ "$(tr '\n' ' ' < fw.txt)" = '10_schema 11_pipeline ' ] || { echo "    --wanted over the fixture: $(tr '\n' ' ' < fw.txt)"; exit 1; }

# the day of 2026-10-14: a new case names the release record and is not listed
kase 20_site_offers 'test -d site/static/releases'
gate 10
expect_grep 'UNLISTED  20_site_offers names the release record and release-record-cases does not run it'
expect_no_grep '94_ci_plan|3033_'
listing 10_schema 11_pipeline 20_site_offers
gate 0

# reading this checkout's own installation guide is reading what is offered from the record;
# a fixture's own copy of it, named without $ROOT, is not
kase 21_install_guide 'grep -q curl "$ROOT/docs/INSTALL.md"'
kase 22_fixture_guide 'printf x > docs/INSTALL.md'
gate 10
expect_grep 'UNLISTED  21_install_guide'
expect_no_grep '22_fixture_guide'
listing 10_schema 11_pipeline 20_site_offers 21_install_guide
gate 0

# a listed name that is not a case
listing 10_schema 11_pipeline 20_site_offers 21_install_guide 99_gone
gate 10
expect_grep 'MISSING   release-record-cases names 99_gone, which is not a case'

# a runs line that would let a case skip itself
sed 's/ --no-skips / /' "$F/.ai/repo/ci/gates.yaml" > "$T/m" && mv "$T/m" "$F/.ai/repo/ci/gates.yaml"
gate 10
expect_grep 'NOSKIPS   release-record-cases does not run named cases with --no-skips'

# an exemption whose case is gone is a finding, not a silent widening
listing 10_schema 11_pipeline 20_site_offers 21_install_guide
rm "$F/test/cases/94_ci_plan.sh"
gate 10
expect_grep 'EXEMPT    94_ci_plan is exempt from the definition and is not a case'
kase 94_ci_plan 'echo .ai/repo/releases'
gate 0

# no model, or no such gate in it, is not a clean answer
sed '/release-record-cases/,$d' "$F/.ai/repo/ci/gates.yaml" > "$T/m" && mv "$T/m" "$F/.ai/repo/ci/gates.yaml"
gate 12
expect_grep 'declares no gate release-record-cases'
rm "$F/.ai/repo/ci/gates.yaml"
gate 12
expect_grep 'gates.yaml is missing'
MJ_ROOT="$F" expect_exit 2 "$GATE" --bogus
expect_grep 'unknown option --bogus'
