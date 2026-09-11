# majordomus-exclusive: builds a fixture repository and runs the site generator in it
# The site generator and the layer's optional sections.
#
# The layer says use cases are optional: the source classes `use_case`, `taxonomy` and
# `application` in sources.yaml carry `required: false`, and lib/usecase.sh honours it. The
# generator used to be the one reader that made absence fatal, so every fixture carried this
# repository's use cases to get past it — and then executed every one of their scenarios
# against the tool, on every run of every case that built a fixture. Neither end wanted them.
#
# This case holds all four combinations. It exists because turning a fatal into a no-op is
# how a check becomes one that cannot fail: rows two and three are the guard, and without a
# case that watches them fire, the guard is a comment.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }

fixture_repo "$T/r" AGENTS.md docs
cd "$T/r"
GEN="$T/r/scripts/generate-site-data"
MAN=".ai/manifest.yaml"

# --- row four: named by nothing, present nowhere.
# This is what `fixture_repo` now builds, and the point of the change: a fixture renders no
# use-case page, so it runs no scenario and does not carry the objects a scenario needs.
grep -q '^  use-cases: ' "$MAN" && { echo "    the fixture's manifest still names a use-cases section"; exit 1; }
[ -d .ai/repo/use-cases ] && { echo "    the fixture still carries the use cases"; exit 1; }
expect_exit 0 "$GEN" --inputs
expect_no_grep '^\.ai/[^ ]*/use-cases/' -
expect_no_grep '^\.ai/[^ ]*/applications/' -
# and the section is gone from the manifest, so nothing downstream will look for it

# The refusals below are refused before the generator reads anything, which is why they are
# observable through --inputs and why a refused build cannot have written a tree.
#
# --- row three: present, named by nothing.
# Objects nobody named would be rendered by nothing and proved by nothing, so the generator
# refuses and names the source class that matched rather than a directory it guessed.
mkdir -p .ai/repo/use-cases
printf -- '---\nschema: use-case/v1\nid: stray\nkind: use-case\ntitle: A stray use case\n---\n# Situation\n\nx\n' > .ai/repo/use-cases/stray.md
expect_exit 10 "$GEN" --inputs
expect_grep 'names no use-cases section' -
expect_grep 'use_case' -

# --- row two: named, absent.
# The manifest promises a section the layer does not have. That is a broken layer, not a
# repository without use cases, and it stays refused.
rm -rf .ai/repo/use-cases
# under `sections:`, not at the end of the file: a key appended at top level is a different
# key, and the case would then be re-proving row four while claiming to prove row two
sed 's/^sections:$/sections:\
  use-cases: repo\/use-cases/' "$MAN" > "$MAN.tmp" && mv "$MAN.tmp" "$MAN"
grep -q '^  use-cases: repo/use-cases$' "$MAN" || { echo "    the case did not put the section back under sections:"; exit 1; }
expect_exit 10 "$GEN" --inputs
expect_grep 'names a use-cases section' -

# --- row one: named and present.
# The repository itself. The section's files are inputs again, and the generator asks for
# them exactly as it always did.
mkdir -p .ai/repo/use-cases
cp "$ROOT"/.ai/repo/use-cases/taxonomy.yaml .ai/repo/use-cases/ 2>/dev/null || true
printf -- '---\nschema: use-case/v1\nid: stray\nkind: use-case\ntitle: A stray use case\n---\n# Situation\n\nx\n' > .ai/repo/use-cases/stray.md
expect_exit 0 "$GEN" --inputs
expect_grep '^\.ai/[^ ]*/use-cases/stray\.md$' -

exit 0
