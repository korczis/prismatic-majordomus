# majordomus-covers: none
# The published-surfaces producer under share/surfaces derives a site, a wiki and Markdown
# regions from a repository, and its check refuses a repository whose published copy would
# disagree with it (I4611).
#
# The subject is share/surfaces/surfaces, which `script:` cannot name (it resolves under
# scripts/ only), hence `none`. Driven against a throwaway repository, never this checkout:
# what is asserted is the producer's behaviour, not this repository's documents.
#
# What is proved:
#   - check exits 10 and names a stale region, a relative link to nothing and an anchor no
#     heading defines; sync --check exits 11 while a region would change;
#   - sync regenerates the region from the documents, leaves an example region inside a code
#     fence alone, and then sync --check and (once the links are fixed) check exit 0;
#   - site writes the overview, the document index, one page per document with its links
#     rewritten to pages and anchors kept, the chart and search data, and a web-surface/v1
#     declaration that majordomus_web_surfaces discovers;
#   - wiki writes Home, the sidebar and the footer, and rewrites a relative .md link to the
#     wiki page it became.
. "$ROOT/test/lib.sh"

SURF="$ROOT/share/surfaces"
[ -f "$SURF/surfaces.py" ] || { echo "    no share/surfaces/surfaces.py"; exit 1; }
sh -n "$SURF/surfaces" || { echo "    share/surfaces/surfaces does not parse"; exit 1; }

# The producer needs Python 3.11+ (tomllib), markdown-it-py and PyYAML. Use an interpreter
# that already has them; build a throwaway environment only if none does, and skip when that
# cannot be done (no network), saying what was not measured.
PY=""
for cand in "${SURFACES_VENV:+$SURFACES_VENV/bin/python}" "$SURF/.venv/bin/python" python3; do
  [ -n "$cand" ] || continue
  if "$cand" -c 'import tomllib, markdown_it, yaml' 2>/dev/null; then PY="$cand"; break; fi
done
if [ -z "$PY" ]; then
  python3 -c 'import tomllib' 2>/dev/null || skip "python3 is older than 3.11 (no tomllib), so the producer was not measured"
  python3 -m venv "$T/venv" >/dev/null 2>&1 \
    && "$T/venv/bin/pip" install --quiet --disable-pip-version-check -r "$SURF/requirements.txt" >/dev/null 2>&1 \
    || skip "markdown-it-py and PyYAML are not importable and could not be installed, so the producer was not measured"
  PY="$T/venv/bin/python"
fi
surfaces() { "$PY" "$SURF/surfaces.py" --repo "$R" "$@"; }

# The repository under test: a README with a stale region, an example region in a code
# fence, one good link, one link to nothing and one anchor to nothing; a guide that links
# back to a README heading.
R="$T/repo"
mkdir -p "$R/docs"
cat > "$R/README.md" <<'MD'
# Fixture

A throwaway repository for the published-surfaces case.

See [the guide](docs/guide.md), [nothing](docs/missing.md) and [no anchor](docs/guide.md#nowhere).

<!-- surfaces:begin docs-index -->
stale
<!-- surfaces:end docs-index -->

```text
<!-- surfaces:begin docs-index -->
example
<!-- surfaces:end docs-index -->
```
MD
cat > "$R/docs/guide.md" <<'MD'
# Guide

How the fixture is used.

## Set up

Back to the [readme](../README.md#fixture).
MD
(
  cd "$R"
  git init -q
  git config user.email case@example.invalid
  git config user.name case
  git remote add origin https://github.com/example/fixture.git
  git add -A
  git commit -q -m fixture
)

expect_exit 10 surfaces check
expect_grep "region 'docs-index' is stale"
expect_grep "no tracked file or directory: docs/missing.md"
expect_grep "no heading or anchor 'nowhere' in docs/guide.md"
expect_no_grep "guide.md:.*readme"   # the guide's link to a README heading resolves

expect_exit 11 surfaces sync --check
expect_exit 0 surfaces sync
expect_grep '^\| \[Guide\]\(docs/guide.md\) \| How the fixture is used. \|$' "$R/README.md"
expect_no_grep '^stale$' "$R/README.md"
expect_grep '^example$' "$R/README.md"   # the fenced example is not a region
expect_exit 0 surfaces sync --check

# Fix the two links; the check then passes.
sed -i.bak -e 's#, \[nothing\](docs/missing.md)##' -e 's#docs/guide.md\#nowhere#docs/guide.md\#set-up#' "$R/README.md"
rm -f "$R/README.md.bak"
expect_exit 0 surfaces check
expect_grep "2 document\(s\), 0 finding\(s\)"

# The site.
S="$T/site"
expect_exit 0 surfaces site --no-assets --out "$S"
for f in index.html docs.html docs/guide.html 404.html data/site.json data/search.json surface.json .nojekyll; do
  expect_file "$S/$f"
done
expect_grep '"schema": "web-surface/v1"' "$S/surface.json"
expect_grep '"id": "site"' "$S/surface.json"
expect_grep 'href="docs/guide.html#set-up"' "$S/index.html"
expect_grep 'href="../index.html#fixture"' "$S/docs/guide.html"
expect_grep 'id="set-up"' "$S/docs/guide.html"
"$PY" - "$S" <<'PY' || exit 1
import json, sys
s = sys.argv[1]
data = json.load(open(f"{s}/data/site.json"))
index = json.load(open(f"{s}/data/search.json"))
assert data["repository"] == "example/fixture", data["repository"]
assert {d["p"] for d in index} == {"README.md", "docs/guide.md"}, index
assert sum(w[1] for w in data["activity"]) == 1, data["activity"]
PY

# The wiki.
K="$T/wiki"
expect_exit 0 surfaces wiki --out "$K"
for f in Home.md docs-guide.md _Sidebar.md _Footer.md surface.json; do expect_file "$K/$f"; done
expect_grep '\]\(docs-guide\)' "$K/Home.md"
expect_grep '\]\(Home#fixture\)' "$K/docs-guide.md"
expect_grep '"id": "wiki"' "$K/surface.json"
