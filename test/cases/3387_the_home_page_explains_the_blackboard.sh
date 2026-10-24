# majordomus-covers: none
# The home page explains the peer blackboard, and claims nothing it does not do (I3073, ADR 0176).
#
# The blackboard is a running server's reading of its boards and the mesh, so the published home
# page can carry none of it. What the page can do is say what the blackboard shows, where a
# running server follows it live, that the site itself is files read at one commit, and what is
# planned for it: the `blackboard` section of site/templates/index.html, its words in
# site/data/marketing.toml and its document and issues in site/data/homepage.toml `[blackboard]`.
#
# This case builds this checkout's site with zola into its scratch directory, as
# scripts/site-build runs zola once the data is generated: the templates, content and committed
# data as they are, a build record naming this checkout's commit, and no stylesheets. It proves,
# of the built home page:
#
#   1. the section is in the declared story: homepage-check's narrative, substance, present,
#      promises and honesty checks pass over the build, and the section is rendered once, with
#      no figure and no picture;
#   2. it links the page the docs dataset routes docs/BLACKBOARD.md to, and the build holds that
#      page, rendered from that document;
#   3. it names, in the document's order, exactly the issues docs/BLACKBOARD.md lists under
#      "Not yet" (I2998, I2984 and I2999 on this tree), each linked to its plan page, which the
#      build holds, with the title and status the plan gives it, and none of them done;
#   4. every route its running-server card names is one docs/BLACKBOARD.md names "In the
#      browser", none is linked, and the card says the board is followed live there;
#   5. nothing on the page is live or looks it: no follow marker, no socket client, no WebSocket
#      or /events address, and no word of the section outside the running-server card and the
#      plan's own titles calls anything live; the card about this site names the commit the
#      build record gives;
#   6. each judgement fails when its fault is planted (a document listing one issue fewer, an
#      issue the plan records as done, a follow marker, a live sentence about this site), and
#      the template stops the build on an issue the plan does not hold or a document the docs
#      dataset does not route.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || skip "zola absent"
command -v python3 >/dev/null || skip "python3 absent"
command -v jq >/dev/null || skip "jq absent"

S="$T/site-copy"
mkdir -p "$S/site" "$S/docs/generated"
cp -R "$ROOT/site/content" "$ROOT/site/templates" "$ROOT/site/static" "$ROOT/site/data" \
  "$ROOT/site/config.toml" "$S/site/"
# every page's base template loads i18n/languages.json for its language switch (ADR 0218)
[ ! -d "$ROOT/site/i18n" ] || cp -R "$ROOT/site/i18n" "$S/site/"
cp "$ROOT/docs/BLACKBOARD.md" "$S/docs/BLACKBOARD.md"
# the web topology homepage-check reads to tell this site's pages from a composed surface's
cp "$ROOT/docs/generated/web.json" "$S/docs/generated/web.json"
# the stylesheets are Tailwind build products, ignored by git; the pages only link them
[ -f "$S/site/static/app.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$S/site/static/app.css"
[ -f "$S/site/static/kit.css" ] || printf '/* fixture: no Tailwind build here. */\n' > "$S/site/static/kit.css"
# the build record scripts/site-build writes, with what the templates read of it: the commit,
# its cleanliness, and no composed surface, snapshot or release
commit="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo 0123456789abcdef0123456789abcdef01234567)"
jq -n --arg c "$commit" '{schema: 1, commit: $c, dirty: false, surfaces: [], snapshots: [], release: null}' \
  > "$S/site/data/build.json"

build() { # the copy, built the way scripts/site-build runs zola; what zola said is in $S/zola.err
  rm -rf "$S/site/public"
  ( cd "$S/site" && zola build --force -o "$S/site/public" ) > "$S/zola.out" 2> "$S/zola.err"
}

cat > "$S/judge.py" <<'PY'
import html, json, os, re, sys, tomllib

s, commit = sys.argv[1], sys.argv[2]
pub = os.path.join(s, "site", "public")
fails = []


def bad(msg):
    fails.append(msg)


def read(rel):
    with open(os.path.join(s, rel), encoding="utf-8") as f:
        return f.read()


def toml(rel):
    with open(os.path.join(s, rel), "rb") as f:
        return tomllib.load(f)


def text_of(fragment):
    return " ".join(html.unescape(re.sub(r"<[^>]+>", " ", fragment)).split())


def href(tag):
    m = re.search(r'\bhref="([^"]*)"', tag)
    return re.sub(r"^https?://[^/]+", "", html.unescape(m.group(1))) if m else ""


def between(text, start, ends):
    a = text.find(start)
    if a < 0:
        return ""
    found = [i for i in (text.find(e, a) for e in ends) if i >= 0]
    return text[a:min(found) if found else len(text)]


def part(md, heading):
    m = re.search(r"^## " + re.escape(heading) + r"[ \t]*\n(.*?)(?=^## |\Z)", md, re.S | re.M)
    return m.group(1) if m else ""


home = read("site/public/index.html")
main = re.search(r"<main\b[^>]*>(.*)</main>", home, re.S)
main = main.group(1) if main else ""
opened = re.findall(r'<section\b[^>]*\bid="blackboard"', main)
found = re.search(r'<section\b[^>]*\bid="blackboard".*?</section>', main, re.S)
section = found.group(0) if found else ""
if len(opened) != 1:
    bad(f"the home page renders {len(opened)} blackboard section(s) inside <main>, and declares one")
bb = toml("site/data/homepage.toml").get("blackboard") or {}
words = toml("site/data/marketing.toml").get("blackboard") or {}
md = read("docs/BLACKBOARD.md")

# 1. no figure and no picture: the published site holds no reading of the blackboard
if re.search(r"<(img|svg|picture|video|canvas)\b", section):
    bad("the section draws a picture of the blackboard, which only a running server can read")
if re.search(r"<dd\b[^>]*>\s*[0-9][0-9.,%]*\s*</dd>", section):
    bad("the section prints a figure, and the published site holds no reading of the blackboard to count")

# 2. the document, by the route the docs dataset gives its path
doc = bb.get("doc") or ""
route = ""
if doc != "BLACKBOARD.md":
    bad(f"site/data/homepage.toml [blackboard] doc is {doc!r};"
        " the blackboard is explained by docs/BLACKBOARD.md")
docs = json.loads(read("site/data/generated/docs.json")).get("docs") or []
routes = [d.get("route") for d in docs if d.get("path") == doc]
doc_links = [href(t) for t in re.findall(r"<a\b[^>]*>", section) if "data-blackboard-doc" in t]
if len(routes) != 1:
    bad(f"site/data/generated/docs.json routes {doc!r} {len(routes)} time(s), so the section leads nowhere")
else:
    route = routes[0]
    page = os.path.join(pub, route.strip("/"), "index.html")
    if doc_links != [route]:
        bad(f"the section leads to {doc_links} for the document, and the docs dataset routes {doc} to {route}")
    if not os.path.isfile(page):
        bad(f"the section links {route}, and the build holds no page there")
    else:
        title = re.search(r"^# (.+)$", md, re.M)
        h1 = re.search(r"<h1\b[^>]*>(.*?)</h1>", open(page, encoding="utf-8").read(), re.S)
        if not title or not h1 or title.group(1).strip() not in text_of(h1.group(1)):
            said = text_of(h1.group(1)) if h1 else None
            bad(f"{route} is not the page of docs/{doc}: its heading reads {said!r}")

# 3. the issues: the document's list under "Not yet", the declaration, the page, and the plan
listed = []
for item in re.findall(r"^- (.*(?:\n[ \t]+.*)*)", part(md, "Not yet"), re.M):
    for iid in re.findall(r"\((I[0-9]+)\)", item):
        if iid not in listed:
            listed.append(iid)
waits = bb.get("waits") or []
if waits != listed:
    bad(f"site/data/homepage.toml [blackboard] waits {waits}, and docs/BLACKBOARD.md lists"
        f" {listed} under \"Not yet\"")
blocks = re.findall(r'(<li\b[^>]*\bdata-blackboard-wait="([^"]+)".*?</li>)', section, re.S)
shown_ids = [iid for _, iid in blocks]
if shown_ids != waits:
    bad(f"the section lists {shown_ids}, and site/data/homepage.toml [blackboard] waits {waits}")
plan = {i.get("id"): i for i in json.loads(read("site/data/generated/plan.json")).get("issues") or []}
for block, iid in blocks:
    issue = plan.get(iid)
    if not issue:
        bad(f"the section lists {iid}, and the plan holds no such issue")
        continue
    status = (issue.get("status") or "").upper()
    if status in ("DONE", "CANCELLED"):
        bad(f"{iid} is {status} in the plan, and the home page and docs/BLACKBOARD.md still list it as planned")
    want = f"/plan/{iid.lower()}/"
    if want not in [href(t) for t in re.findall(r"<a\b[^>]*>", block)]:
        bad(f"{iid} does not lead to its plan page {want}")
    elif not os.path.isfile(os.path.join(pub, "plan", iid.lower(), "index.html")):
        bad(f"{iid} leads to {want}, and the build holds no page there")
    said = re.findall(r'\bdata-status="([^"]+)"', block)
    if said != [status.lower()]:
        bad(f"{iid} is shown as {said}, and the plan derives {status}")
    if (issue.get("title") or "") not in text_of(block):
        bad(f"{iid} is not shown with the plan's title {issue.get('title')!r}")

# 4. the routes a running server follows it at: the document's, named and never linked
served = between(section, "data-blackboard-served", ["data-blackboard-published"])
published = between(section, "data-blackboard-published", ["data-blackboard-waits", "data-blackboard-doc"])
waiting = between(section, "data-blackboard-waits", ["</ul>"])
named = re.findall(r"<code\b[^>]*\bdata-blackboard-route\b[^>]*>([^<]+)</code>", served)
browser = part(md, "In the browser")
if not named:
    bad("the running-server card names no route the blackboard is followed at")
for r in named:
    if f"`{r}`" not in browser:
        bad(f"the section names {r} as following the blackboard, and docs/BLACKBOARD.md's"
            " \"In the browser\" does not")
    if re.search(r'\bhref="[^"]*' + re.escape(r) + r'/?"', home):
        bad(f"the home page links {r}, which only a running server answers")
if not re.search(r"\blive\b", text_of(served), re.I):
    bad("the running-server card does not say the board is followed live there")
if not words.get("served") or words["served"] not in text_of(served):
    bad("the running-server card does not carry site/data/marketing.toml [blackboard] served")
if not words.get("published") or words["published"] not in text_of(published):
    bad("the card about this site does not carry site/data/marketing.toml [blackboard] published")

# 5. nothing on the page is live, and nothing says so but the card about a running server
if re.search(r'\sdata-follow(-[a-z]+)?(=|\s|>)', home):
    bad("the home page carries a data-follow marker, which makes a page follow the live channel")
for src in re.findall(r'<script\b[^>]*\bsrc="([^"]+)"', home):
    if re.search(r"(socket|live|monitor|peers)\.js(\?|$)", src):
        bad(f"the home page loads {src}, a live channel's client")
if re.search(r"wss?://", home) or re.search(r'\b(href|src)="[^"]*/events([?"/])', home):
    bad("the home page names a WebSocket or the live channel /events")
rest = text_of(section.replace(served, " ").replace(waiting, " "))
if re.search(r"\blive\b", rest, re.I):
    bad("outside the running-server card and the plan's titles the section calls something live: "
        + " ".join(re.findall(r"[^.]*\blive\b[^.]*", rest, re.I))[:200])
labels = re.findall(r'\bdata-blackboard-commit="([^"]+)"', published)
if labels != [commit] or commit[:7] not in text_of(published):
    bad(f"the card about this site names the commit(s) {labels}, and the build record gives {commit}")

for f in fails:
    print("    " + f)
if not fails:
    print(f"    the home page names {', '.join(shown_ids) or 'no issue'} as planned and none done,"
          f" leads to {route}, names {', '.join(named)} as followed live on a running server,"
          f" and is live nowhere itself")
sys.exit(1 if fails else 0)
PY
judge() { python3 -I "$S/judge.py" "$S" "$commit" > "$S/judge.out" 2>&1; }
refuses() { # <pattern> <what>: the judge refuses the planted fault, naming it
  if judge; then echo "    $2: passed"; cat "$S/judge.out"; exit 1; fi
  grep -qE -- "$1" "$S/judge.out" || { echo "    $2: no finding matching /$1/"; cat "$S/judge.out"; exit 1; }
}
keep() { cp "$S/$1" "$S/keep.${1//\//_}"; }
restore() { cp "$S/keep.${1//\//_}" "$S/$1"; }

# ---------------------------------------------------------------- 1. the declared story
build || { echo "    zola could not build the copy of the site"; tail -5 "$S/zola.err"; exit 1; }
rc=0
MJ_ROOT="$S" "$ROOT/scripts/ci/homepage-check" --only narrative,substance,present,promises,honesty \
  > "$S/homepage.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    homepage-check refused the built home page (exit $rc)"; cat "$S/homepage.out"; exit 1; }
for c in narrative substance present promises honesty; do
  grep -qE "^OK   $c " "$S/homepage.out" || { echo "    homepage-check printed no verdict for $c"; cat "$S/homepage.out"; exit 1; }
done
grep -qE '^OK   narrative .* surfaces > blackboard > ' "$S/homepage.out" \
  || { echo "    the blackboard section is not in the declared story after surfaces"; cat "$S/homepage.out"; exit 1; }

# ---------------------------------------------------------------- 2-5. the built page, judged
judge || { echo "    the built home page does not explain the blackboard as declared:"; cat "$S/judge.out"; exit 1; }
cat "$S/judge.out"

# ---------------------------------------------------------------- 6. each judgement can fail
# The two faults of the list are planted while it lists anything: once every issue in it is
# done, the document's "Not yet" and the declaration are empty together and the page lists none.
first="$(python3 -I -c 'import sys, tomllib
w = tomllib.load(open(sys.argv[1], "rb"))["blackboard"]["waits"]
print(w[0] if w else "")' "$S/site/data/homepage.toml")"
if [ -n "$first" ]; then
  # the document lists one issue fewer than the page
  keep docs/BLACKBOARD.md
  python3 -I - "$S/docs/BLACKBOARD.md" <<'PY'
import re, sys
p = sys.argv[1]
md = open(p, encoding="utf-8").read()
head = md.index("\n## Not yet")
cut = re.compile(r" ?\(I[0-9]+\)").search(md, head)
open(p, "w", encoding="utf-8").write(md[:cut.start()] + md[cut.end():])
PY
  refuses 'under "Not yet"' "a document that lists one issue fewer"
  restore docs/BLACKBOARD.md
  # an issue the plan records as done, still listed as planned
  keep site/data/generated/plan.json
  jq --arg i "$first" '(.issues[] | select(.id == $i) | .status) = "DONE"' "$S/keep.site_data_generated_plan.json" \
    > "$S/site/data/generated/plan.json"
  refuses "$first is DONE in the plan" "an issue the plan records as done"
  restore site/data/generated/plan.json
fi

# a follow marker, and a sentence calling this static page live
keep site/public/index.html
sed 's#<section id="blackboard"#<section data-follow="/events?follow=blackboard" id="blackboard"#' \
  "$S/keep.site_public_index.html" > "$S/site/public/index.html"
refuses 'data-follow marker' "a follow marker on the home page"
sed 's#data-blackboard-published>#data-blackboard-published><p>This page is live.</p>#' \
  "$S/keep.site_public_index.html" > "$S/site/public/index.html"
refuses 'calls something live: .*This page is live' "a live sentence about this site"
restore site/public/index.html

# the template stops the build on an issue the plan does not hold, or a document it cannot route
keep site/data/homepage.toml
sed 's/^waits = \[/waits = ["I0000", /' "$S/keep.site_data_homepage.toml" > "$S/site/data/homepage.toml"
if build; then echo "    the site built with an issue the plan does not hold"; exit 1; fi
grep -q 'waits names I0000, which is no issue of the plan' "$S/zola.err" \
  || { echo "    the refused build does not name the unknown issue"; tail -5 "$S/zola.err"; exit 1; }
sed 's/^doc = "BLACKBOARD.md"/doc = "NO_SUCH_DOCUMENT.md"/' "$S/keep.site_data_homepage.toml" \
  > "$S/site/data/homepage.toml"
if build; then echo "    the site built with a document the docs dataset does not route"; exit 1; fi
grep -q "doc 'NO_SUCH_DOCUMENT.md' is a path data/generated/docs.json does not route" "$S/zola.err" \
  || { echo "    the refused build does not name the unrouted document"; tail -5 "$S/zola.err"; exit 1; }
restore site/data/homepage.toml
exit 0
