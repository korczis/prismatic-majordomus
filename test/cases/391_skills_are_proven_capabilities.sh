# majordomus-covers: skills
# majordomus-negative: skills doctor
# majordomus-skill: deploy-site implement repo-review assess-before-deleting report-verification-state pack-for-review
# A skill is a capability only when something proves it (project.skills-are-proven-capabilities,
# ADR 0076). This case drives the real commands over a disposable repository: a skill that a
# test names and a workflow invokes stands as partial until a recorded passing run makes its
# evidence current; every way of being an orphan or a binding to nothing is refused by
# `skills verify`, by doctor through the doctrine, and named; a broken contract and an invalid
# provenance marker are refused by the index and reported by the same verdict. Then it reads
# this repository itself: every skill the marker line above names is valid, invoked and counts
# this case among its tests, and no active skill is left out of the line.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || { echo "    skip: jq absent"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
S="$(mktemp -d "${TMPDIR:-/tmp}/mj391.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base

M="majordomus-skill:"
skill() { # id [status] [extra front matter]
  mkdir -p ".ai/repo/skills/$1"
  printf -- '---\nschema: skill/v1\nid: %s\nversion: 1\ntitle: Skill %s\ndescription: The %s procedure.\nstatus: %s\n%b---\n# Purpose\n\nWhy.\n\n# Procedure\n\n1. Do it.\n\n# Output\n\nA report.\n' \
    "$1" "$1" "$1" "${2:-active}" "${3:-}" > ".ai/repo/skills/$1/SKILL.md"
}
verify() { "$RB" skills verify --format json 2>/dev/null > "$S/v.json" || true; }
has() { jq -e --arg c "$1" '[.findings[] | "\(.level):\(.code)"] | index($c) != null' "$S/v.json" >/dev/null \
  || { echo "    expected finding $1:"; cat "$S/v.json"; exit 1; }; }
lacks() { jq -e --arg c "$1" '[.findings[] | "\(.level):\(.code)"] | index($c) == null' "$S/v.json" >/dev/null \
  || { echo "    did not expect finding $1:"; cat "$S/v.json"; exit 1; }; }
commit() { git add -A >/dev/null; git commit -qm "$1" >/dev/null; }

# ---------------------------------------------------------------- a skill, invoked and named
skill alpha
mkdir -p .ai/repo/workflows test/cases
printf '# Uses alpha\n\nFollow majordomus://skill/alpha.\n' > .ai/repo/workflows/uses-alpha.md
printf '# %s alpha\ntrue\n' "$M" > test/cases/01_alpha.sh
commit "alpha, invoked and named"
expect_exit 0 "$RB" skills status
expect_grep '^alpha +active +partial +not_run +no +no +yes$'
expect_exit 0 "$RB" skills verify
expect_grep '1 skill\(s\), 0 failure\(s\), 3 warning\(s\): valid'
expect_exit 0 "$RB" skills explain alpha
expect_grep 'test +test/cases/01_alpha\.sh +not_run +\[reproduce: bash test/run\.sh 01_alpha\]'
expect_grep 'invoked +\.ai/repo/workflows/uses-alpha\.md:3 +majordomus://skill/alpha'

# status is derived, never authored: a key saying so is refused by the contract
skill alpha active 'tested: true\n'
commit "an authored status"
verify; has fail:contract
expect_exit 10 "$RB" skills verify
expect_grep 'unknown_key|tested'
skill alpha; commit "the authored status goes"

# a recorded passing run of the naming test is what makes the evidence current
printf '01_alpha\tok\t1\tparallel\n' > "$S/run.tsv"
expect_exit 0 "$RB" evidence record --suite "$S/run.tsv"
"$RB" skills explain alpha --format json 2>/dev/null > "$S/alpha.json"
jq -e '.tested.state == "proven" or .tested.state == "inputs_unchanged"' >/dev/null \
  "$S/alpha.json" || { echo "    a recorded pass did not make the evidence current:"; cat "$S/alpha.json"; exit 1; }
verify; lacks warn:unevidenced
# and a failing run refuses
printf '01_alpha\tFAIL\t1\tparallel\n' > "$S/run.tsv"
expect_exit 0 "$RB" evidence record --suite "$S/run.tsv"
verify; has fail:failing
expect_exit 10 "$RB" skills verify
git rm -q --cached .ai/repo/evidence/ledger.json >/dev/null 2>&1 || true; rm -f .ai/repo/evidence/ledger.json

# ---------------------------------------------------------------- orphans
rm .ai/repo/workflows/uses-alpha.md; commit "nothing invokes alpha"
verify; has fail:unused
expect_exit 10 "$RB" skills verify
expect_grep "FAIL unused +\.ai/repo/skills/alpha/SKILL\.md: orphan: nothing invokes skill 'alpha'"
expect_exit 0 "$RB" skills status
expect_grep '^alpha +active +orphan '
# doctor reports the orphan through the doctrine, and refuses
expect_exit 10 "$MJ" doctor
expect_grep "^FAIL skill +\.ai/repo/skills/alpha/SKILL\.md — unused: orphan"
printf 'majordomus://skill/alpha\n' > .ai/repo/workflows/uses-alpha.md
printf 'true\n' > test/cases/01_alpha.sh; commit "no test names alpha"
verify; has fail:untested; lacks fail:unused
expect_exit 10 "$RB" skills verify
# a marker written as data binds nothing
printf "cat > fixture.sh <<'FIXTURE'\n%s alpha\nFIXTURE\n" "$M" > test/cases/01_alpha.sh; commit "a marker as data"
verify; has fail:untested

# ---------------------------------------------------------------- bindings to nothing
printf '# %s alpha ghost\ntrue\n' "$M" > test/cases/01_alpha.sh
printf 'majordomus://skill/alpha then majordomus://skill/phantom\n' > .ai/repo/workflows/uses-alpha.md
commit "bindings to nothing"
verify; has fail:unknown_skill_in_test; has fail:unknown_skill_invoked; lacks fail:untested
expect_exit 10 "$RB" skills verify
expect_grep "test/cases/01_alpha\.sh: the test names skill 'ghost', which does not exist"
expect_grep "uses-alpha\.md: line 1 invokes skill 'phantom'"
printf '# %s alpha\ntrue\n' "$M" > test/cases/01_alpha.sh
printf 'majordomus://skill/alpha\n' > .ai/repo/workflows/uses-alpha.md
commit "bindings resolve"
expect_exit 0 "$RB" skills verify

# an unresolvable dependency between skills is the contract's: the shell judge refuses it
skill beta draft 'related: [nosuch]\n'; commit "an unresolvable related skill"
expect_exit 10 "$MJ" skills check
expect_grep "related skill 'nosuch' does not exist"
skill beta draft; commit "beta resolves"
# a draft owes nothing
"$RB" skills explain beta --format json 2>/dev/null > "$S/beta.json"
jq -e '.standing == "not_required" and .findings == []' >/dev/null \
  "$S/beta.json" || { echo "    a draft skill was held to the proof:"; cat "$S/beta.json"; exit 1; }

# ---------------------------------------------------------------- the contract and provenance
# missing required metadata
skill gamma draft; sed -i.bak '/^title: /d' .ai/repo/skills/gamma/SKILL.md && rm -f .ai/repo/skills/gamma/SKILL.md.bak
commit "gamma has no title"
verify; has fail:contract
expect_exit 10 "$RB" skills verify
expect_grep 'gamma/SKILL\.md: schema_violation: .*"title" is a required property'
expect_exit 0 "$RB" skills status
expect_grep '^gamma +unknown +invalid '
expect_exit 10 "$MJ" skills check
expect_grep 'gamma/SKILL\.md — .*title is empty'
# a provenance marker that is not the opaque form: a path, a decision the schema does not know
skill gamma draft 'provenance:\n  origin: prior-art\n  ledger: some/private/notes.md\n  decision: copied\n'
commit "an invalid provenance marker"
verify; has fail:contract
expect_exit 10 "$RB" skills verify
expect_grep 'provenance\.ledger: "some/private/notes\.md" does not match'
expect_grep 'provenance\.decision: "copied" is not one of'
# the opaque form is accepted and carried
skill gamma draft 'provenance:\n  origin: prior-art\n  ledger: import-2026-09-09#7\n  decision: adapted\n'
commit "a valid provenance marker"
expect_exit 0 "$RB" skills verify
expect_exit 0 "$RB" skills explain gamma
expect_grep 'provenance +prior-art import-2026-09-09#7 adapted'
# a duplicate id: the index refuses both claimants and both are named
skill twin draft; sed -i.bak 's/^id: twin$/id: beta/; s/^description: .*$/description: Another thing./' .ai/repo/skills/twin/SKILL.md && rm -f .ai/repo/skills/twin/SKILL.md.bak
commit "a duplicate id"
verify; has fail:contract
expect_exit 10 "$RB" skills verify
expect_grep 'beta/SKILL\.md: duplicate_identity'
expect_grep 'twin/SKILL\.md: duplicate_identity'
expect_exit 10 "$MJ" skills check
expect_grep "duplicate skill id 'beta'"
git rm -rq .ai/repo/skills/twin >/dev/null; commit "the duplicate goes"
expect_exit 0 "$RB" skills verify

# ---------------------------------------------------------------- this repository
# every skill this case names is valid, invoked, and counts this case among its tests; every
# active skill is named here
NAMED="$(sed -n "s/^# $M //p" "$ROOT/test/cases/391_skills_are_proven_capabilities.sh" | head -1)"
[ -n "$NAMED" ] || { echo "    this case names no skill"; exit 1; }
( cd "$ROOT" && "$RB" skills status --format json 2>/dev/null ) > "$S/repo.json" \
  || { echo "    skills status failed on this repository"; exit 1; }
for id in $NAMED; do
  jq -e --arg id "$id" '.skills[] | select(.id == $id) | .valid and .used.used
      and ([.tested.tests[].path] | index("test/cases/391_skills_are_proven_capabilities.sh") != null)' "$S/repo.json" >/dev/null \
    || { echo "    skill $id is not a valid, invoked capability this case tests:"; jq --arg id "$id" '.skills[] | select(.id == $id)' "$S/repo.json"; exit 1; }
done
for id in $(jq -r '.skills[] | select(.status == "active") | .id' "$S/repo.json"); do
  case " $NAMED " in *" $id "*) ;; *) echo "    active skill $id is not named by this case"; exit 1 ;; esac
done
