# majordomus-covers: script:scripts/site-check
# Every article ends with the articles related to it, in its reader's language, and each of them
# answers it back (I3074).
#
# Case 3502 holds an article to its counterparts: the same article in another language. On
# 2026-10-19 the owner asked that the articles be interlinked in every language as well, so an
# article names the other articles related to it in [extra] related, as routes, and
# site/templates/page.html ends it with a Related section drawn from those pages. Check 21 of
# scripts/site-check reads the list with the reader it has for the counterparts.
#
# What is proved, against fixtures of three groups, each one article in English and its Czech
# counterpart: a complete relation passes, counting its links, and a site that relates nothing
# owes nothing. Each of these is refused, naming the file: a route that is no page, a page that
# is no article, the article itself, one of its own counterparts, a route listed twice, a page
# in another language where its group has one in the article's own, a related article that
# names neither the article nor any of its counterparts back, two counterparts that relate
# different articles, an article the navigation lists that names none while the site relates
# its articles, a list written over several lines, and a list on a page that is no article.
# Then the real tree passes, and, where zola is installed, a build of this checkout's site draws
# every article's Related section in the order its list names, each link to the page it names,
# in that page's language and with its title, and no other page draws one.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/site-check"

# art FILE LANG RELATED COUNTERPART-LANG COUNTERPART-ROUTE: an article with one line of body.
# RELATED is the array as front matter writes it, or "-" for none.
art() {
  {
    printf '+++\ntitle = "%s"\ntemplate = "page.html"\n[extra]\nlang = "%s"\n' "${1##*/}" "$2"
    if [ "$3" != - ]; then printf 'related = %s\n' "$3"; fi
    printf '[extra.translations]\n%s = "%s"\n+++\nThe body of %s.\n' "$4" "$5" "${1##*/}"
  } > "$1"
}
# fixture DIR: three groups, A (a, b), C (c, d) and E (e, f), English then Czech, each article
# naming the two other groups in its own language; a page that is no article; a navigation
# listing all seven; and a config whose default language is English
fixture() {
  rm -rf "$1"; mkdir -p "$1/content"
  printf 'base_url = "https://example.invalid"\ndefault_language = "en"\n' > "$1/config.toml"
  {
    printf '[[groups]]\nlabel = "Articles"\nitems = [\n'
    for r in a b c d e f plain; do printf '  { label = "%s", href = "/%s/" },\n' "$r" "$r"; done
    printf ']\n'
  } > "$1/nav.toml"
  art "$1/content/a.md" en '["c/", "e/"]' cs b/
  art "$1/content/b.md" cs '["d/", "f/"]' en a/
  art "$1/content/c.md" en '["a/", "e/"]' cs d/
  art "$1/content/d.md" cs '["b/", "f/"]' en c/
  art "$1/content/e.md" en '["a/", "c/"]' cs f/
  art "$1/content/f.md" cs '["b/", "d/"]' en e/
  printf '+++\ntitle = "plain"\n+++\nA page.\n' > "$1/content/plain.md"
}
run() { expect_exit "$1" "$CHECK" --articles "$PWD/$2/content" "$PWD/$2/nav.toml" "$PWD/$2/config.toml"; }

# ---------------------------------------------------------------- a complete relation passes
fixture fx
run 0 fx
expect_grep '^OK   articles .*\(6 of 6\) .*; 12 related link\(s\) are answered back, each in its reader.s language'
expect_no_grep 'FAIL'

# ---------------------------------------------------------------- a route that is no page
fixture fx
art fx/content/a.md en '["c/", "e/", "nowhere/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names nowhere/ as related, and no page under .*fx/content is at that route'

# ---------------------------------------------------------------- a page that is no article
fixture fx
art fx/content/a.md en '["c/", "e/", "plain/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/plain\.md as related, and .*plain\.md declares no lang, so it is no article'

# ---------------------------------------------------------------- the article itself
fixture fx
art fx/content/a.md en '["a/", "c/", "e/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names itself as related'

# ---------------------------------------------------------------- one of its own counterparts
fixture fx
art fx/content/a.md en '["b/", "c/", "e/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/b\.md as related, and .*b\.md is one of its own counterparts'

# ---------------------------------------------------------------- a route listed twice
fixture fx
art fx/content/a.md en '["c/", "c/", "e/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md lists c/ twice as related'

# ---------------------------------------------------------------- another language where its own exists
fixture fx
art fx/content/a.md en '["d/", "e/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/d\.md as related, which is written in cs; its en counterpart .*fx/content/c\.md is the one to name'
# the relation itself holds: d answers a through b, and a and b relate the same groups
expect_no_grep 'names neither'
expect_no_grep 'relate different articles'

# ---------------------------------------------------------------- not answered back
fixture fx
art fx/content/c.md en '["e/"]' cs d/
art fx/content/d.md cs '["f/"]' en c/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/c\.md as related, and .*c\.md names neither it nor any of its counterparts back'
expect_grep 'FAIL articles .*fx/content/b\.md names .*fx/content/d\.md as related, and .*d\.md names neither it nor any of its counterparts back'
expect_no_grep 'relate different articles'

# ---------------------------------------------------------------- counterparts that relate different articles
fixture fx
art fx/content/a.md en '["c/"]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md and its cs counterpart .*fx/content/b\.md relate different articles'

# ---------------------------------------------------------------- a listed article that names none
fixture fx
art fx/content/a.md en - cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md is an article the navigation lists and names no related article, and the site relates its articles'

# ---------------------------------------------------------------- a list over several lines
fixture fx
art fx/content/a.md en '[
  "c/", "e/",
]' cs b/
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md writes \[extra\] related other than as one array of routes on one line'

# ---------------------------------------------------------------- a list on a page that is no article
fixture fx
printf '+++\ntitle = "plain"\n[extra]\nrelated = ["a/"]\n+++\nA page.\n' > fx/content/plain.md
run 10 fx
expect_grep 'FAIL articles .*fx/content/plain\.md names related articles and declares no lang'

# ---------------------------------------------------------------- a site that relates nothing owes nothing
fixture fx
for x in a:en:cs:b c:en:cs:d e:en:cs:f b:cs:en:a d:cs:en:c f:cs:en:e; do
  IFS=: read -r n l o t <<<"$x"; art "fx/content/$n.md" "$l" - "$o" "$t/"
done
run 0 fx
expect_no_grep 'related link'

# ---------------------------------------------------------------- the real tree
expect_exit 0 "$CHECK" --articles "$ROOT/site/content-src" "$ROOT/site/data/nav.toml" "$ROOT/site/config.toml"
expect_grep '^OK   articles .*; [0-9]+ related link\(s\) are answered back'

# ---------------------------------------------------------------- the built section
if ! command -v zola >/dev/null 2>&1 || ! command -v python3 >/dev/null 2>&1; then
  echo "    zola or python3 is absent: the Related section was judged by its sources alone"
  exit 0
fi
S="$PWD/site-copy"
rm -rf "$S"; mkdir -p "$S/site"
cp -R "$ROOT/site/content" "$ROOT/site/templates" "$ROOT/site/static" "$ROOT/site/data" "$ROOT/site/config.toml" "$S/site/"
# every page's base template loads i18n/languages.json for its language switch (ADR 0218)
[ ! -d "$ROOT/site/i18n" ] || cp -R "$ROOT/site/i18n" "$S/site/"
# the stylesheets are Tailwind build products, ignored by git; the pages only link them
for css in app.css kit.css; do [ -f "$S/site/static/$css" ] || printf '/* fixture */\n' > "$S/site/static/$css"; done
printf '{"schema": 1, "commit": "%s", "dirty": false, "surfaces": [], "snapshots": [], "release": null}\n' \
  "$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo 0123456789abcdef0123456789abcdef01234567)" > "$S/site/data/build.json"
( cd "$S/site" && zola build --force -o "$S/site/public" ) > "$S/zola.out" 2>&1 \
  || { echo "    zola could not build the copy of the site:"; tail -5 "$S/zola.out" | sed 's/^/    | /'; exit 1; }
python3 -I - "$ROOT/site/content-src" "$S/site/public" <<'PY'
import html, os, re, sys, tomllib

src, pub = sys.argv[1], sys.argv[2]
fails = []


def front(path):
    text = open(path, encoding="utf-8").read()
    m = re.match(r"\+\+\+\n(.*?)\n\+\+\+\n", text, re.S)
    return tomllib.loads(m.group(1)) if m else {}


def built(route):
    return os.path.join(pub, route.strip("/"), "index.html")


pages = {}
for name in sorted(os.listdir(src)):
    if name.endswith(".md"):
        pages[name[:-3] + "/"] = front(os.path.join(src, name))
drawn = 0
for route, fm in pages.items():
    extra = fm.get("extra") or {}
    related = extra.get("related") or []
    if not related:
        continue
    here = extra.get("lang", "en")
    page = open(built(route), encoding="utf-8").read()
    section = re.search(r'<section id="related"[^>]*\bdata-related\b.*?</section>', page, re.S)
    if not section:
        fails.append(f"/{route} names {len(related)} related article(s) and draws no Related section")
        continue
    section = section.group(0)
    heading = re.search(r"<h2\b([^>]*)>Related articles</h2>", section)
    if not heading or (here != "en") != ('lang="en"' in heading.group(1)):
        fails.append(f"/{route} draws no Related heading, or does not mark it English on a {here} page")
    links = re.findall(r'<a href="([^"]*)" hreflang="([^"]*)" lang="([^"]*)"[^>]*\bdata-related-route="([^"]*)"[^>]*>(.*?)</a>', section, re.S)
    if [x[3] for x in links] != related:
        fails.append(f"/{route} draws {[x[3] for x in links]}, and its front matter names {related}")
        continue
    for href, hreflang, lang, target, inner in links:
        t = pages.get(target)
        if t is None:
            fails.append(f"/{route} links {target}, which is no article of {src}")
            continue
        t_lang = (t.get("extra") or {}).get("lang", "en")
        if not href.endswith("/" + target) or not os.path.isfile(built(target)):
            fails.append(f"/{route} links {href} for {target}, which is not that route or is not built")
        if hreflang != t_lang or lang != t_lang:
            fails.append(f"/{route} marks {target} as {hreflang}/{lang}, and it is written in {t_lang}")
        if html.escape(t.get("title", ""), quote=False) not in inner and t.get("title", "") not in html.unescape(inner):
            fails.append(f"/{route} does not show {target} by its title {t.get('title')!r}")
        drawn += 1
for root, _, files in os.walk(pub):
    for name in files:
        if name == "index.html":
            path = os.path.join(root, name)
            route = os.path.relpath(root, pub).strip(".") + "/"
            if route.lstrip("/") not in pages or not ((pages[route.lstrip("/")].get("extra") or {}).get("related")):
                if 'data-related>' in open(path, encoding="utf-8").read():
                    fails.append(f"/{route.lstrip('/')} draws a Related section and names no related article")
for f in fails:
    print("    " + f)
if not fails:
    print(f"    every article draws its Related section: {drawn} link(s), each to the page it names, in that page's language")
sys.exit(1 if fails else 0)
PY
