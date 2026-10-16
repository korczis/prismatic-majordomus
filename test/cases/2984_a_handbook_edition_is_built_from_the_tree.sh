# majordomus-covers: none
# majordomus-negative: rules
# The handbook is built from the tree it describes, and the build refuses what came apart.
#
# `project.the-handbook-is-built-from-the-tree` (ADR 0149) says an edition of the handbook is
# built by handbook/build.py from this repository's own tree, that each chapter declares what
# it rests on, and that the build refuses a declaration the repository's graph no longer
# carries. `project.english-only` v3 says an edition may be written in its reader's language
# under three conditions. Both are proved here by mutation: a statement about a refusal is
# worth what a mutation that should trigger it does.
#
# HOW THE FIXTURE IS BUILT. The generator finds its editions beside itself and reads the tree
# named by HANDBOOK_ROOT, so the handbook is copied into the fixture and mutated there while
# the tree it reads stays this repository's: the bytes under test are the shipped generator,
# the graph is the real one, and nothing in the repository is written. Derived files go to a
# directory of the fixture, never to target/.
#
# The second half copies the shipped language check into a fixture repository, exactly as
# test/cases/403_published_translation.sh does and for the reason it gives: the check reads its
# allow list from beside itself, so a copy lands on a list this case wrote.
. "$ROOT/test/lib.sh"
command -v python3 >/dev/null 2>&1 || skip "no python3; the handbook is built by a python program"
[ -f "$ROOT/docs/generated/graph.json" ] || { echo "    docs/generated/graph.json is missing"; exit 1; }

HB="$PWD/hb"
DERIVED="$PWD/derived"
cp -R "$ROOT/handbook" "$HB"
build() { HANDBOOK_ROOT="$ROOT" HANDBOOK_DERIVED="$DERIVED" python3 "$HB/build.py" --check; }
restore() { rm -rf "$HB" && cp -R "$ROOT/handbook" "$HB"; }

# ------------------------------------------------- 1. the shipped handbook builds from this tree
expect_exit 0 build
expect_grep 'cs edition holds'
expect_grep 'every anchor resolves'
[ -s "$DERIVED/cs/prirucka-majordomusu.html" ] || { echo "    the build wrote no page"; exit 1; }
# No heading of the built page skips a level: the outline a screen reader walks has no gap.
python3 - "$DERIVED/cs/prirucka-majordomusu.html" <<'PY' || { echo "    a heading of the built page skips a level"; exit 1; }
import re, sys
body = open(sys.argv[1], encoding="utf-8").read().partition("<script")[0]
levels = [int(x) for x in re.findall(r"<h([1-6])\b", body)]
sys.exit(1 if not levels or any(b > a + 1 for a, b in zip(levels, levels[1:])) else 0)
PY
# The build reads the handbook and writes beside it nothing at all: no bytecode cache, no
# scratch file. A cache was tracked once, and every tree that ran this gate was dirty after it.
left="$(find "$HB" -name '__pycache__' -o -name '*.pyc' | head -3)"
[ -z "$left" ] || { echo "    the build left bytecode in the handbook it read: $left"; exit 1; }
if git -C "$ROOT" ls-files | grep -qE '(^|/)__pycache__/|\.pyc$'; then
  echo "    a bytecode cache is tracked"; exit 1
fi
# what the page states is what the tree holds: the commit it was built from is this one
head_sha="$(git -C "$ROOT" rev-parse --short=10 HEAD)"
grep -q "\"commit\": *\"$head_sha\"\|\"commit\":\"$head_sha\"" "$DERIVED/corpus-index.json" \
  || { echo "    the corpus does not name the commit it was read from ($head_sha)"; exit 1; }

# ------------------------------------------------- 2. an anchor the graph does not carry is refused
# The mutation a rename produces: a chapter still names a document that is no longer there.
python3 - "$HB/cs/chapter_sources.py" <<'PY'
import sys
p = sys.argv[1]; s = open(p, encoding="utf-8").read()
assert '"h-finish": "' in s
open(p, "w", encoding="utf-8").write(s.replace('"h-finish": "', '"h-finish": "doc:docs/NO_SUCH_DOCUMENT.md ', 1))
PY
expect_exit 10 build
expect_grep 'no longer carries'
expect_grep 'docs/NO_SUCH_DOCUMENT.md'
restore

# ------------------------------------------------- 3. a chapter that cites nothing is refused
# Its anchors are removed and its text is left alone; a chapter whose text happens to link a
# source would still pass, so the chapter chosen is one that rests on its declaration alone.
python3 - "$HB/cs/chapter_sources.py" <<'PY'
import re, sys
p = sys.argv[1]; s = open(p, encoding="utf-8").read()
s2 = re.sub(r'\n    "uvod": "[^"\n]*",', "", s, count=1)
assert s2 != s
open(p, "w", encoding="utf-8").write(s2)
PY
expect_exit 10 build
expect_grep 'cite nothing of the repository'
expect_grep 'uvod'
restore

# ------------------------------------------------- 4. a caption with no figure behind it is refused
python3 - "$HB/cs/tldr.py" <<'PY'
import sys
p = sys.argv[1]; s = open(p, encoding="utf-8").read()
assert "%%adrs%%" in s
open(p, "w", encoding="utf-8").write(s.replace("%%adrs%%", "%%nosuchfigure%%", 1))
PY
expect_exit 10 build
expect_grep 'unknown fact placeholder'
expect_grep 'nosuchfigure'
restore

# ------------------------------------------------- 5. an edition is named for the language it declares
mv "$HB/cs" "$HB/xx"
expect_exit 10 build
expect_grep "an edition's directory is named for its language"
restore

# ------------------------------------------------- 6. the language rule, v3: an edition is declared, two-sided
"$MJ" init >/dev/null
SRC="$ROOT/scripts/ci/english-only-check"
[ -x "$SRC" ] || { echo "    scripts/ci/english-only-check is not executable"; exit 1; }
export MJ_ROOT="$PWD"
rm -rf "$HB" "$DERIVED"
mkdir -p scripts/ci handbook/cs docs
cp "$SRC" scripts/ci/english-only-check
chmod +x scripts/ci/english-only-check
EN="$PWD/scripts/ci/english-only-check"
ALLOW="$PWD/scripts/ci/english-only-allow.txt"
allow() {
  printf 'fixture scripts/ci/english-only-check it carries the list of letters it tests for\n' > "$ALLOW"
  [ $# -gt 0 ] && printf '%s\n' "$@" >> "$ALLOW"
  return 0
}
printf '# Alpha\n\nOne sentence of English prose.\n' > docs/ALPHA.md
printf 'lang = "cs"\n' > handbook/cs/edition.toml
printf '<p>Jedna věta, která není anglicky.</p>\n' > handbook/cs/chapter.html
allow
git add -A >/dev/null && git commit -qm fixture

# undeclared: the edition's file is the v1 finding, unchanged
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only handbook/cs/chapter\.html'

# declared, and the directory agrees: allowed
allow 'edition handbook/cs cs the Czech edition, written for Czech readers'
expect_exit 0 "$EN" --strict

# a declaration without a reason is unusable
allow 'edition handbook/cs cs'
expect_exit 12 "$EN" --strict
expect_grep 'must name its language and say why'

# outside the handbook: refused, whatever the directory says of itself
allow 'edition docs cs prose that is not an edition'
expect_exit 12 "$EN" --strict
expect_grep 'one directory of the handbook named for its language'

# one-sided: the list says Czech and the directory says another language
printf 'lang = "pl"\n' > handbook/cs/edition.toml
allow 'edition handbook/cs cs the Czech edition, written for Czech readers'
git add -A >/dev/null && git commit -qm mismatch
expect_exit 12 "$EN" --strict
expect_grep 'does not declare that language itself'

# the generator beside an edition is not inside one: a Czech sentence there is still a finding
printf 'lang = "cs"\n' > handbook/cs/edition.toml
printf '# Poznámka psaná česky, která sem nepatří.\n' > handbook/NOTES.md
git add -A >/dev/null && git commit -qm generator
expect_exit 10 "$EN" --strict
expect_grep 'FAIL english-only handbook/NOTES\.md'
unset MJ_ROOT

# ------------------------------------------------- 7. this repository declares its own edition
grep -qE '^edition handbook/cs cs ' "$ROOT/scripts/ci/english-only-allow.txt" \
  || { echo "    the Czech edition is not declared in scripts/ci/english-only-allow.txt"; exit 1; }
grep -qE '^lang = "cs"' "$ROOT/handbook/cs/edition.toml" \
  || { echo "    handbook/cs/edition.toml does not declare its language"; exit 1; }
# an edition's source pages are chapters and nothing else: no page shell, no stylesheet of
# their own and no script, so the only design an edition wears is the declared one
if grep -lE '<!doctype|<style|<script' "$ROOT"/handbook/cs/sources/*.html 2>/dev/null | grep -q .; then
  echo "    a source page of the Czech edition carries a page shell, a stylesheet or a script"; exit 1
fi
grep -q 'handbook/build.py --check' "$ROOT/.ai/repo/ci/gates.yaml" \
  || { echo "    no gate runs the handbook build"; exit 1; }
