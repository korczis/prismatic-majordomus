# majordomus-covers: none
# An active skill is named where a test demands every active skill (I2664, gate skill-naming).
#
# apps/majordomus-cli/tests/skills.rs and case 391 each fail when an active skill of the
# repository is missing from their marker line. On 2026-10-13 a pull request added an active
# skill named by neither; the structure job passed (skills-proof asks only that SOME test
# names a skill), the pull request was merged with --admin, the trunk's rust lane turned red
# an hour later, and the first fix named the skill in the crate test alone, so case 391 would
# have been the next red. scripts/ci/skill-naming-check asks both questions in the structure
# job. This case plants each shape of the omission in a fixture tree and watches it refused,
# and holds this checkout to the gate.
#
# The marker is assembled from two halves, as case 391 and tests/skills.rs assemble theirs,
# so this file binds itself to no skill.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/skill-naming-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
M="majordomus-""skill:"
CRATE="apps/majordomus-cli/tests/skills.rs"
CASE="test/cases/391_skills_are_proven_capabilities.sh"

# ---------------------------------------------------------------- this checkout
expect_exit 0 "$GATE"
expect_grep 'active skill\(s\), each named by apps/majordomus-cli/tests/skills\.rs and test/cases/391_'

# ---------------------------------------------------------------- a fixture tree
F="$T/tree"
skill() { # <id> [status]
  mkdir -p "$F/.ai/repo/skills/$1"
  printf -- '---\nschema: skill/v1\nid: %s\nversion: 1\ntitle: Skill %s\ndescription: The %s procedure.\nstatus: %s\n---\n# Purpose\n\nWhy.\n' \
    "$1" "$1" "$1" "${2:-active}" > "$F/.ai/repo/skills/$1/SKILL.md"
}
crate_names() { # <ids...>: the crate test's header, plus a fixture line that is data, not a marker
  mkdir -p "$F/apps/majordomus-cli/tests"
  { printf '//! Skills.\n//!\n//! %s %s\n\n' "$M" "$*"
    printf '        &format!("# {MARKER} ghost\\n"),\n'; } > "$F/$CRATE"
}
case_names() { # <ids...>: case 391's first marker line, plus a printf of a fixture marker
  mkdir -p "$F/test/cases"
  { printf '# majordomus-covers: skills\n# %s %s\n' "$M" "$*"
    printf "printf '# %%s alpha ghost\\\\n' \"\$M\" > test/cases/01_alpha.sh\n"; } > "$F/$CASE"
}
gate() { MJ_ROOT="$F" expect_exit "$1" "$GATE"; }

# both name every active skill: clean, and a draft owes nothing
skill alpha; skill beta; skill gamma draft
crate_names alpha beta; case_names alpha beta
gate 0
expect_grep '2 active skill\(s\), each named'

# the night of 2026-10-13: an active skill added and named by neither test
skill delta
gate 10
expect_grep "UNNAMED  active skill 'delta' is not named by apps/majordomus-cli/tests/skills\.rs"
expect_grep "UNNAMED  active skill 'delta' is not named by test/cases/391_skills_are_proven_capabilities\.sh"
expect_no_grep 'ghost'

# its first fix: named in the crate test alone, and case 391 is still owed
crate_names alpha beta delta
gate 10
expect_no_grep "'delta' is not named by apps/majordomus-cli/tests/skills\.rs"
expect_grep "UNNAMED  active skill 'delta' is not named by test/cases/391_"

# case 391 reads its FIRST marker line only, so a second one names nothing
printf '# %s delta\n' "$M" >> "$F/$CASE"
gate 10
expect_grep "'delta' is not named by test/cases/391_"

# named in both, commas allowed as marker_ids allows them: clean again
case_names alpha, beta, delta
gate 0

# a quoted status in the front matter is still active
sed 's/^status: active$/status: "active"/' "$F/.ai/repo/skills/delta/SKILL.md" > "$T/q" && mv "$T/q" "$F/.ai/repo/skills/delta/SKILL.md"
crate_names alpha beta
gate 10
expect_grep "UNNAMED  active skill 'delta' is not named by apps/majordomus-cli/tests/skills\.rs"
crate_names alpha beta delta

# a name with no skill behind it is refused, as both tests refuse it
case_names alpha beta delta epsilon
gate 10
expect_grep "UNKNOWN  test/cases/391_skills_are_proven_capabilities\.sh names skill 'epsilon'"
case_names alpha beta delta

# a naming file that is gone is not a pass
rm "$F/$CASE"
gate 12

# ---------------------------------------------------------------- the real lines, mutated
# the parser reads this repository's own headers: drop one id from a copy and it is named
rm -rf "$F"; mkdir -p "$F/apps/majordomus-cli/tests" "$F/test/cases" "$F/.ai/repo"
cp -R "$ROOT/.ai/repo/skills" "$F/.ai/repo/skills"
cp "$ROOT/$CASE" "$F/$CASE"
first="$(sed -n "s#^//! $M ##p" "$ROOT/$CRATE" | head -n 1 | awk '{ print $1 }')"
[ -n "$first" ] || { echo "    $CRATE has no '//! $M' line to mutate"; exit 1; }
sed "s#^//! $M $first #//! $M #" "$ROOT/$CRATE" > "$F/$CRATE"
cmp -s "$ROOT/$CRATE" "$F/$CRATE" && { echo "    the mutation did not change $CRATE"; exit 1; }
gate 10
expect_grep "UNNAMED  active skill '$first' is not named by apps/majordomus-cli/tests/skills\.rs"

echo "    an active skill missing from the crate test's header or from case 391's first marker line is refused, a draft is not, an unknown name is, and this checkout names every active skill in both"
