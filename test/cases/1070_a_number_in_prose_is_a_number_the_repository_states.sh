# majordomus-covers: script:scripts/site-check
# A number in prose is a number the repository states.
#
# The hand-written pages under site/content-src/ stated their counts by hand — capabilities,
# MCP tools, guaranteed claims, rules, ledger executions — and the site went on printing them
# long after the repository had moved: "1482 capabilities" beside a registry of more than two
# thousand. A page now cites a count, `{{ <cite_figure name="capabilities" /> }}`, and the
# build reads it from generated data through the one declaration of what may be cited,
# site/data/figures.toml (the shortcode is site/templates/shortcodes/cite_figure.html).
#
# What is proved, against fixtures rather than this checkout's build: check 19 of
# scripts/site-check (`--prose-figures <dir>`) refuses a count typed beside a counted noun,
# naming the file, the line and the words, in English and in the Czech of the published
# translation, and passes the same sentence when it cites the figure; it does not take an
# identifier, a version, a year, code or a TOML comment for a count. Then the declaration:
# figures.toml parses, every figure is declared once with a source under site/data/, a path
# that resolves there and a measure, and every figure the real pages cite is declared. Then
# the real pages and site data pass the scan. Last, rendered by zola against a fixture site
# holding the real shortcode, declaration and data: a cited figure prints the number the data
# holds, and a name that is not declared fails the build naming it.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v python3 >/dev/null 2>&1 || skip "python3 absent"
python3 -c 'import tomllib' 2>/dev/null || skip "python3 has no tomllib"
CHECK="$ROOT/scripts/site-check"
FIG="$ROOT/site/data/figures.toml"
SC="$ROOT/site/templates/shortcodes/cite_figure.html"
[ -f "$FIG" ] || { echo "    site/data/figures.toml is missing"; exit 1; }
[ -f "$SC" ] || { echo "    site/templates/shortcodes/cite_figure.html is missing"; exit 1; }

# ---------------------------------------------------------------- the scan refuses a typed count
mkdir -p typed cited
cat > typed/page.md <<'EOF'
+++
title = "a page"
+++
Today the registry holds 1482 capabilities, and most of them are tools.

The suite ran 224 shell cases.
EOF
expect_exit 10 "$CHECK" --prose-figures "$PWD/typed"
expect_grep 'FAIL prose .*typed/page\.md:4: "1482 capabilities"'
expect_grep 'FAIL prose .*typed/page\.md:6: "224 shell cases"'
expect_grep 'site/data/figures\.toml'
# the Czech of the published translation, and a TOML file of site data, are read too
printf 'V grafu je 195 p\305\231\303\255kaz\305\257 a 64 z nich nem\303\241 capability.\n' > typed/czech.md
printf '# a comment may say 12 rules\nlead = "It answers 137 rules at once."\n' > typed/copy.toml
expect_exit 10 "$CHECK" --prose-figures "$PWD/typed"
expect_grep 'typed/czech\.md:1: "195 p'
expect_grep 'typed/copy\.toml:2: "137 rules"'
expect_no_grep 'typed/copy\.toml:1:'

# ---------------------------------------------------------------- and passes the cited figure
cat > cited/page.md <<'EOF'
+++
title = "a page"
+++
Today the registry holds {{ <cite_figure name="capabilities" /> }} capabilities.

ADR 0123 settled it in case 1060, issue 42 asked for version 12 tests, and I2054 and #901
claim nothing. It shipped in September 2026 with v0.20.0 commands; `224 shell cases` is
code, and so is this:

```console
$ scripts/ci/claim-proof-check --strict
guaranteed claims:   0 of 161 name a test that does not name the claim back
```
<!-- 99 rules in a comment -->
EOF
expect_exit 0 "$CHECK" --prose-figures "$PWD/cited"
expect_grep '^OK   prose '
# a directory with nothing to read is not a clean one
mkdir -p empty
expect_exit 12 "$CHECK" --prose-figures "$PWD/empty"

# ---------------------------------------------------------------- the declaration resolves
python3 - "$ROOT/site" <<'PY' > decl.out || { cat decl.out; exit 1; }
import json, pathlib, re, sys, tomllib
site = pathlib.Path(sys.argv[1])
figs = tomllib.loads((site / "data/figures.toml").read_text())["figure"]
bad = []
names = [f.get("name") for f in figs]
for n in sorted({n for n in names if names.count(n) > 1}):
    bad.append(f"figure {n} is declared {names.count(n)} times")
for f in figs:
    n = f.get("name", "?")
    for k in ("name", "source", "path", "measure", "description"):
        if k not in f:
            bad.append(f"figure {n} declares no {k}")
    if f.get("measure") not in ("value", "length"):
        bad.append(f"figure {n}: measure is {f.get('measure')!r}, not value or length")
    if ("where_attribute" in f) != ("where_value" in f):
        bad.append(f"figure {n}: where_attribute and where_value come together")
    src = site / "data" / f.get("source", "")
    if not src.is_file():
        bad.append(f"figure {n}: site/data/{f.get('source')} does not exist")
        continue
    v = json.loads(src.read_text())
    for k in f.get("path", []):
        if not isinstance(v, dict) or k not in v:
            bad.append(f"figure {n}: {'.'.join(f['path'])} does not resolve in site/data/{f['source']}")
            v = None
            break
        v = v[k]
    if v is None:
        continue
    if f.get("measure") == "value" and not isinstance(v, (int, float)):
        bad.append(f"figure {n}: the value at its path is not a number")
    if f.get("measure") == "length" and not isinstance(v, (list, dict)):
        bad.append(f"figure {n}: a length is taken of something that is neither a list nor a map")
# every figure a page cites is declared
cited = set()
for p in sorted((site / "content-src").rglob("*.md")):
    for n in re.findall(r'\{\{ *<cite_figure name="([^"]*)" */> *\}\}', p.read_text()):
        cited.add(n)
        if n not in names:
            bad.append(f"{p.relative_to(site.parent)} cites the figure {n}, which site/data/figures.toml does not declare")
if not cited:
    bad.append("no page under site/content-src cites a figure; the citation syntax is not being read")
print("\n".join("    " + b for b in bad))
sys.exit(1 if bad else 0)
PY

# ---------------------------------------------------------------- the real pages pass
expect_exit 0 "$CHECK" --prose-figures "$ROOT/site/content-src"
mkdir -p data-toml; cp "$ROOT"/site/data/*.toml data-toml/
expect_exit 0 "$CHECK" --prose-figures "$PWD/data-toml"

# ---------------------------------------------------------------- the shortcode, rendered
if command -v zola >/dev/null; then
  Z="$PWD/zsite"; mkdir -p "$Z/templates/shortcodes" "$Z/content" "$Z/data"
  cp "$SC" "$Z/templates/shortcodes/"; cp "$FIG" "$Z/data/"
  for s in $(python3 -c 'import sys, tomllib; print("\n".join(sorted({f["source"] for f in tomllib.load(open(sys.argv[1], "rb"))["figure"]})))' "$FIG"); do
    mkdir -p "$Z/data/$(dirname "$s")"; cp "$ROOT/site/data/$s" "$Z/data/$s"
  done
  printf 'base_url = "https://example.invalid"\ntitle = "fixture"\n' > "$Z/config.toml"
  printf '{{ page.content | safe }}\n' > "$Z/templates/page.html"
  printf '{{ section.content | safe }}\n' > "$Z/templates/index.html"
  printf '+++\ntitle = "home"\n+++\n' > "$Z/content/_index.md"
  printf '+++\ntitle = "p"\ntemplate = "page.html"\n+++\nholds [{{ <cite_figure name="capabilities" /> }}] and [{{ <cite_figure name="claims_guaranteed" /> }}].\n' > "$Z/content/p.md"
  ( cd "$Z" && run_quiet "$PWD/../zola.err" zola build --force -o "$Z/public" >/dev/null ) \
    || { echo "    zola could not render the figure fixture:"; cat zola.err; exit 1; }
  caps="$(jq '.telemetry.capabilities' "$ROOT/site/data/registry/product.json")"
  guaranteed="$(jq '[.claims[] | select(.status == "guaranteed")] | length' "$ROOT/site/data/generated/capabilities.json")"
  grep -qF "holds [$caps] and [$guaranteed]." "$Z/public/p/index.html" \
    || { echo "    the cited figures did not render as the data holds them ($caps, $guaranteed):"; cat "$Z/public/p/index.html"; exit 1; }
  printf '+++\ntitle = "p"\ntemplate = "page.html"\n+++\nholds {{ <cite_figure name="no_such_figure" /> }}.\n' > "$Z/content/p.md"
  if ( cd "$Z" && zola build --force -o "$Z/public" > ../zola.err 2>&1 ); then
    echo "    a page citing an undeclared figure built anyway"; exit 1
  fi
  grep -qF "figure 'no_such_figure' is not declared in site/data/figures.toml" zola.err \
    || { echo "    the build refused an undeclared figure without naming it:"; cat zola.err; exit 1; }
else
  echo "    (zola absent: the rendered figure is not checked)"
fi
