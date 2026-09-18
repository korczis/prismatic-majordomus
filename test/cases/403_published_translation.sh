# majordomus-covers: none
# majordomus-negative: rules
# claims: published-translation-is-declared
# The third kind of entry in scripts/ci/english-only-allow.txt, proved by mutation.
#
# `project.english-only` v1 said "with no exceptions", and it meant it: the check refuses a
# letter that occurs in Czech, Slovak or Polish and never in English in any tracked authored
# file, and the site's content tree is in scope. v2 (ADR 0077) names one population apart —
# content published for a reader in that reader's language — and confines the permission with
# three conditions, each decided here rather than by review:
#
#   1  it lives under the authored source of the published site, or is its projection;
#   2  it is declared `translation <path> <language> <why>`, with the reason required;
#   3  the page declares the same language itself, in its own front matter.
#
# The third condition is the one that matters. An allow list nothing else agrees with is a
# list an entry can be added to in order to silence a finding, which is the laundering shape
# this repository's exemptions exist to refuse. Requiring the page to say what it is makes the
# declaration two-sided: an entry that is not true of its file cannot pass.
#
# HOW THE FIXTURE IS BUILT, and what that costs. The check reads its allow list from beside
# itself and not from the tree under measurement — deliberately, so that a fixture cannot mint
# this repository's vocabulary (test/cases/138_governance_gates.sh says so where it relies on
# it). That decision is kept: nothing below changes how the installed check resolves its list.
# What this case does instead is copy the shipped script into the fixture and run *that*, so
# `dirname $0` lands on a list this case wrote. The bytes under test are the shipped bytes;
# only their neighbour differs. The real check measuring this real repository is asserted at
# the end, where the two published articles must be declared and the gate must be green.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
SRC="$ROOT/scripts/ci/english-only-check"
[ -x "$SRC" ] || { echo "    scripts/ci/english-only-check is not executable"; exit 1; }

export MJ_ROOT="$PWD"
mkdir -p scripts/ci site/content-src/articles site/content docs
cp "$SRC" scripts/ci/english-only-check
chmod +x scripts/ci/english-only-check
EN="$PWD/scripts/ci/english-only-check"
ALLOW="$PWD/scripts/ci/english-only-allow.txt"

# page <path> <lang-line> — a site page whose front matter carries whatever language line the
# caller passes, and whose body is a sentence that only the alphabet test has to decide.
page() {
  mkdir -p "$(dirname "$1")"
  { printf -- '+++\ntitle = "t"\ndescription = "d"\ntemplate = "page.html"\n[extra]\n'
    [ -n "${2:-}" ] && printf '%s\n' "$2"
    printf -- '+++\n\nJedna věta, která není anglicky.\n'
  } > "$1"
}

# allow [line ...] — the fixture's list, always carrying the one entry every copy of this
# check needs: it contains the letters it tests for and cannot express the rule without
# them, exactly as the real list declares beside the real script.
allow() {
  printf 'fixture scripts/ci/english-only-check it carries the list of letters it tests for\n' > "$ALLOW"
  [ $# -gt 0 ] && printf '%s\n' "$@" >> "$ALLOW"
  return 0
}

printf '# Alpha\n\nOne sentence of English prose.\n' > docs/ALPHA.md
allow
git add -A >/dev/null && git commit -qm fixture

# The tree as committed is English, and the copied check agrees: the construction itself is
# sound before anything is mutated.
expect_exit 0 "$EN" --strict
expect_grep 'every authored file is spelled in English'

# ------------------------------------------------- 1. an undeclared page is still refused
# v2 did not open the site tree; it opened declared pages in it. A Czech page nobody declared
# is the v1 finding, unchanged, and this is the assertion that keeps the amendment narrow.
page site/content-src/articles/cs.md 'lang = "cs"'
git add -A >/dev/null && git commit -qm undeclared
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only site/content-src/articles/cs\.md'

# ------------------------------------------------- 2. declared, and the page agrees: allowed
allow 'translation site/content-src/articles/cs.md cs an article written for Czech readers'
expect_exit 0 "$EN" --strict
expect_grep 'declared translation'

# ------------------------------------------------- 3. a declaration without a reason is unusable
# The same shape the fixture kind has: an entry that says nothing is a list entry, not a
# declaration, and the check refuses to run rather than quietly passing the file.
allow 'translation site/content-src/articles/cs.md cs'
expect_exit 12 "$EN" --strict
expect_grep 'must name its language and say why'

# ------------------------------------------------- 4. outside the published site: refused
# The permission is a property of publication, not of a language. A translation entry naming a
# document, a rule or a source file is the exemption escaping the population it was opened for.
allow 'translation docs/ALPHA.md cs prose that is not published content'
expect_exit 12 "$EN" --strict
expect_grep 'published site'

# ------------------------------------------------- 5. one-sided declarations are refused
# The entry says Czech; the page declares nothing. Nothing on the page agrees with the list,
# which is exactly the case an allow list added to silence a finding produces.
page site/content-src/articles/cs.md ''
allow 'translation site/content-src/articles/cs.md cs an article written for Czech readers'
git add -A >/dev/null && git commit -qm silent
expect_exit 12 "$EN" --strict
expect_grep 'does not declare'

# ...and a page that declares a different language than the entry claims is the same defect.
page site/content-src/articles/cs.md 'lang = "pl"'
git add -A >/dev/null && git commit -qm mismatch
expect_exit 12 "$EN" --strict
expect_grep 'does not declare'

# ------------------------------------------------- 6. the exemption does not travel
# A declared translation exempts its own file and nothing else: the permission is per path,
# and an ordinary source file in the same tree is measured as it always was.
page site/content-src/articles/cs.md 'lang = "cs"'
printf '#!/usr/bin/env bash\n# Tato věta anglicky není.\n' > scripts/ci/other.sh
allow 'translation site/content-src/articles/cs.md cs an article written for Czech readers'
git add -A >/dev/null && git commit -qm neighbour
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only scripts/ci/other\.sh'

# ------------------------------------------------- 7. this repository, measured by its own check
# The fixture proves the behaviour; this proves the behaviour is the one in force here. Both
# published articles are declared, the pages carry the language their entries claim, and the
# gate is green over this repository as it stands.
cd "$ROOT" || exit 1
unset MJ_ROOT
expect_exit 0 "$ROOT/scripts/ci/english-only-check" --strict
expect_grep 'every authored file is spelled in English'
grep -q '^translation site/content-src/' "$ROOT/scripts/ci/english-only-allow.txt" \
  || { echo "    no published translation is declared; this case is measuring nothing"; exit 1; }
