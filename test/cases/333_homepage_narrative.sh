# majordomus-covers: none
# The homepage tells the story site/data/homepage.toml declares, and the site's indexing policy
# is the one site/data/nav.toml declares — scripts/ci/homepage-check, exercised against a tree
# this case builds rather than against the site in the checkout, so it runs in seconds.
#
# What is proved: a clean tree passes; each of these fails, with a finding that names it —
# a section rendered and not declared, a section declared and not rendered, the right sections
# in the wrong order, a declared section with nothing in it, a draft feature linked from the
# homepage, an install section missing a command the distribution model gives, a page below an
# unlisted section without noindex, a page below an unlisted section in the sitemap, and an
# indexable page absent from the sitemap, a page loading the Mermaid runtime with no diagram,
# a provider of the model the page does not name, a name the model does not carry, the worker
# tools named nowhere, a trust card that disagrees with its dataset, a trust card missing, and a
# trust card reading as known when its dataset is gone,
# the homepage's HTML, scripts or inline JSON over the declared budget, no budget declared, a
# budget that leaves the HTML out, and a linked asset the build did not produce; the
# working interfaces: an interface of the data the page does not list, one it lists that the
# data does not give, a link to a published copy the topology does not carry, a published
# surface the page still calls local, and a local one whose mount is not stated; and a
# declared capability word in the hand-written copy whose claim is absent, planned or
# unsupported by a recorded run, a declaration naming a claim that does not exist, and a
# draft feature named in the copy; a sentence of the homepage's template that states how the
# work stands (case 3105 holds every form of that, and the real files); and a product view
# whose release changes are not the newest release record's, counted by the changelog's groups,
# or whose delivery stages are not the inferred delivery's (I2851), a count inside a link read;
# and a count of the plan's issues back on the hero, linked or not, or a block marked as one,
# and a /roadmap/ count the plan does not give (I3917, rule v2).
# Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
# the declared promise words: one no claim permits; the promises section below adds two that
# name claims of its own matrix
promises() { printf '\n[promises]\n"unattended" = ""\n' >> "$F/site/data/homepage.toml"; }
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/public/plan/i0001" "$F/site/public/features/good" "$F/site/public/features/draft"
  printf 'order = ["hero", "how", "install"]\n\n[budget]\nhtml_bytes = 4000\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  promises
  # the hand-written copy, clean: no declared word, no feature that is not stable
  printf '[hero]\ntitle = "Keep the work moving"\n' > "$F/site/data/marketing.toml"
  printf '# a comment may say unattended: comments are not copy\n[intro]\nlead = "The repository decides what is done."\n' > "$F/site/data/manifesto.toml"
  # the homepage's template, clean: a comment may say today, the text says nothing of the present
  mkdir -p "$F/site/templates"
  printf '{#- a comment may say what is shipped today: comments are not text -#}\n<section id="hero"><h1>{{ m.hero.title }}</h1></section>\n' > "$F/site/templates/index.html"
  mkdir -p "$F/site/public/js" "$F/site/public/docs/plain" "$F/site/public/docs/diagram"
  printf 'x%.0s' $(seq 1 40) > "$F/site/public/js/app.js"
  printf '<html><head><title>p</title></head><body>plain</body></html>\n' > "$F/site/public/docs/plain/index.html"
  printf '<html><head><title>d</title><script src="https://x.test/js/mermaid.min.js"></script></head><body><pre class="mermaid">graph TD</pre></body></html>\n' > "$F/site/public/docs/diagram/index.html"
  printf '[[groups]]\nlabel = "Plan"\nhref = "/plan/"\n\n[indexing]\nunlisted = ["plan"]\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable","weight":1,"commands":["init"]},{"id":"draft","route":"/features/draft/","status":"draft"}],"domains":[],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"curl -fsSL https://x.test/install.sh | sh","next_command":"majordomus init","verify_command":"majordomus --version","latest":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/registry/distribution.json"
  mkdir -p "$F/site/data/generated" "$F/docs/generated" "$F/site/public/ref/api"
  # the topology: the app, and one surface site-build composes at /ref, whose pages are that
  # surface's own and follow neither the story nor the indexing policy (ADR 0086)
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}' > "$F/docs/generated/web.json"
  printf '<html><head><title>r</title><script src="https://x.test/js/mermaid.min.js"></script></head><body>r</body></html>\n' > "$F/site/public/ref/index.html"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/ref/api/index.html"
  printf '%s\n' '{"use_cases":[{"id":"a"},{"id":"b"},{"id":"c"}],"categories":[{"id":"x"},{"id":"y"}]}' > "$F/site/data/generated/catalogue.json"
  # the claims matrix, the design tones and no recorded evidence: the proof band says unknown
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"commit":"abcdef1234567890","dirty":false,"release":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/build.json"
  # the one product sentence (marketing.toml's hero) and every projection of it saying it
  printf '# m\n\n**Keep the work moving**\n' > "$F/README.md"
  printf 'description = "Keep the work moving"\n' > "$F/site/config.toml"
  printf '%s\n' '{"tagline":"Keep the work moving"}' > "$F/site/data/generated/project.json"
  mkdir -p "$F/share/design/brand" "$F/site/public/use-cases/a"
  printf '<svg><text x="0" fill="#cbd5e1">Keep the work moving</text></svg>\n' > "$F/share/design/brand/social-card.svg"
  printf '# Launch\n\n## One sentence\n\nKeep the work moving\n' > "$F/docs/LAUNCH.md"
  # one recorded refusal the hero offers after the challenge, and one walkthrough step whose
  # command a stable feature carries
  printf '%s\n' '{"scenarios":[{"use_case":"a","route":"/use-cases/a/"}],"walkthrough":[{"label":"init"}]}' > "$F/site/data/generated/terminal.json"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/use-cases/a/index.html"
  cat > "$F/site/public/index.html" <<'HTML'
<html><head><title>Fixture — Keep the work moving</title><script defer src="https://x.test/js/app.js"></script></head><body><main>
<section id="hero"><h1>t</h1><a href="/features/good/">good</a>
<div data-hero-scenarios><a href="/challenge/" data-scenario="challenge">c</a><a href="/use-cases/a/" data-scenario="a">a</a></div>
<ul data-node-features="step:init"><li><a href="/features/good/" data-feature="good">good</a></li></ul>
<dl><dt>generated for</dt><dd data-providers><span>Tool One</span><a href="/providers/two/">Tool Two</a></dd></dl>
<dl data-trust>
<div data-trust-key="release"><dt>release</dt><dd>9.9.9</dd></div>
<div data-trust-key="verify"><dt>confirm</dt><dd>majordomus --version</dd></div>
<div data-trust-key="commit"><dt>commit</dt><dd>abcdef1</dd></div>
<div data-trust-key="use-cases"><dt>use cases</dt><dd>3</dd><dd>2 areas</dd></div>
</dl>
<dl data-hero-figures><div><dt><a href="/features/good/">good</a></dt><dd>1</dd></div></dl>
<dl data-claims-proof><div><dt>guaranteed claims, declared</dt><dd data-declared="guaranteed">0</dd><dd data-supported="unmeasured">not yet measured</dd></div></dl>
<div data-surfaces x-data="{}"><select data-surface-picker><option value="one.show" selected>one.show</option></select>
<dl><div><dd><a data-surface-id="one.show" href="/r/">one.show</a></dd></div><div><dd data-surface="cli">majordomus one show</dd></div><div><dd><a data-surface="http" href="/docs/api/">GET /api/v1/one</a></dd></div><div><dd data-surface="mcp">majordomus_one</dd></div></dl></div></section>
<section id="how"><dl><dt>x</dt><dd>7</dd></dl></section>
<section id="install"><code>curl -fsSL https://x.test/install.sh | sh</code> then <code>majordomus init</code>, check with <code>majordomus --version</code> <a href="/getting-started/">go</a></section>
</main></body></html>
HTML
  # one capability on every surface, and the slice the picker fetches (case 406 exercises these)
  printf '%s\n' '{"registry":{"builtin":[{"id":"one.show","exposure":{"cli":{"path":["one","show"]},"http":{"method":"GET","path":"/api/v1/one"},"mcp":{"tool":"majordomus_one"}}}]}}' > "$F/site/data/registry/registry.json"
  mkdir -p "$F/site/public/graphs"
  printf '%s\n' '[{"id":"one.show","cli":"majordomus one show","http":{"method":"GET","path":"/api/v1/one"},"mcp":"majordomus_one"}]' > "$F/site/public/graphs/surfaces.json"
  printf '<html><head><title>plan</title></head><body>plan</body></html>\n' > "$F/site/public/plan/index.html"
  printf '<html><head><title>i</title><meta name="robots" content="noindex"></head><body>i</body></html>\n' > "$F/site/public/plan/i0001/index.html"
  printf '<html><head><title>g</title></head><body>g</body></html>\n' > "$F/site/public/features/good/index.html"
  printf '<html><head><title>d</title></head><body>d</body></html>\n' > "$F/site/public/features/draft/index.html"
  cat > "$F/site/public/sitemap.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<urlset><url><loc>https://x.test/</loc></url><url><loc>https://x.test/docs/plain/</loc></url><url><loc>https://x.test/docs/diagram/</loc></url><url><loc>https://x.test/plan/</loc></url><url><loc>https://x.test/features/good/</loc></url><url><loc>https://x.test/features/draft/</loc></url><url><loc>https://x.test/use-cases/a/</loc></url></urlset>
XML
}
run() { MJ_ROOT="$F" "$CHECK" > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# --- a clean tree passes, every check reporting
fresh
expect_finding 0 '^OK   narrative ' "a clean tree"
for c in substance figures sentence scenarios nodes honesty map install runtime providers trust surfaces interfaces product evidence declared promises present weight indexing; do grep -q "^OK   $c " out.txt || { echo "    a clean tree did not report $c"; cat out.txt; exit 1; }; done

# --- narrative, both directions and the order
fresh; sed -i.bak 's#<section id="how">#<section id="extra"><a href="/x/">x</a></section><section id="how">#' "$F/site/public/index.html"
expect_finding 10 'renders section\(s\) site/data/homepage.toml does not declare: extra' "an undeclared section"
fresh; printf 'order = ["hero", "how", "proof", "install"]\n' > "$F/site/data/homepage.toml"
expect_finding 10 'declares section\(s\) the homepage does not render: proof' "a declared section not rendered"
fresh; printf 'order = ["how", "hero", "install"]\n' > "$F/site/data/homepage.toml"
expect_finding 10 'out of the declared order' "sections out of order"

# --- substance
fresh; sed -i.bak 's#<dl><dt>x</dt><dd>7</dd></dl>#<p>nothing</p>#' "$F/site/public/index.html"
expect_finding 10 'carry no link and no derived figure: how' "an empty declared section"

# --- honesty
fresh; sed -i.bak 's#<a href="/features/good/">good</a>#<a href="/features/draft/">draft</a>#' "$F/site/public/index.html"
expect_finding 10 'does not call stable: /features/draft/ \(draft\)' "a draft feature linked"

# --- install
fresh; sed -i.bak 's#, check with <code>majordomus --version</code>##' "$F/site/public/index.html"
expect_finding 10 'does not carry the distribution model.s verify_command' "a missing verification command"

# --- runtime: the Mermaid runtime loaded with nothing to draw
fresh; sed -i.bak 's#<title>p</title>#<title>p</title><script src="https://x.test/js/mermaid.min.js"></script>#' "$F/site/public/docs/plain/index.html"
expect_finding 10 'load the Mermaid runtime with no diagram to render: /docs/plain/' "a page loading Mermaid for nothing"
grep -q '/docs/diagram/' out.txt && { echo "    a page with a diagram was reported as loading Mermaid for nothing"; cat out.txt; exit 1; }

# --- providers: the worker tools named are the model's, both ways
fresh; sed -i.bak 's#<span>Tool One</span>##' "$F/site/public/index.html"
expect_finding 10 'names worker tool\(s\) the homepage does not: Tool One' "a provider of the model the page does not name"
fresh; sed -i.bak 's#<dd data-providers>#<dd data-providers><span>Tool Three</span>#' "$F/site/public/index.html"
expect_finding 10 'names worker tool\(s\) the product model does not: Tool Three' "a name the model does not carry"
fresh; sed -i.bak 's# data-providers##' "$F/site/public/index.html"
expect_finding 10 'carries no <dd data-providers>' "the worker tools named nowhere"

# --- trust: every card carries what its dataset says, and says unknown when it says nothing
fresh; sed -i.bak 's#<dd>9.9.9</dd>#<dd>1.2.3</dd>#' "$F/site/public/index.html"
expect_finding 10 "release does not carry '9.9.9'" "a trust card disagreeing with its dataset"
fresh; sed -i.bak 's# data-trust-key="commit"##' "$F/site/public/index.html"
expect_finding 10 'carries no trust card for: commit' "a trust card missing"
fresh; rm "$F/site/data/build.json"
expect_finding 10 "commit does not carry 'unknown'" "a commit card reading as known with no build.json"
fresh; rm "$F/site/data/generated/catalogue.json"
expect_finding 10 "use-cases does not carry 'unknown'" "a use-case card reading as known with no catalogue"

# --- weight: over the asset budget, over the inline JSON budget, no budget, a linked asset missing
fresh; printf 'x%.0s' $(seq 1 200) > "$F/site/public/js/app.js"
expect_finding 10 'loads 200 bytes of scripts and stylesheets from this site, over the declared 100' "scripts over budget"
fresh; sed -i.bak 's#<section id="hero">#<script type="application/json" id="g">{"nodes":["a","b","c","d","e"]}</script><section id="hero">#' "$F/site/public/index.html"
expect_finding 10 'bytes of inline JSON, over the declared 20' "inline JSON over budget"
fresh; printf 'order = ["hero", "how", "install"]\n' > "$F/site/data/homepage.toml"
expect_finding 10 'declares no \[budget\]' "no budget declared"
# the HTML is budgeted too: a page over it fails, and a budget that names the other two and
# leaves the HTML out is no budget — which is what this file declared until 2026-10-12
fresh; printf 'order = ["hero", "how", "install"]\n\n[budget]\nhtml_bytes = 300\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
expect_finding 10 'bytes of HTML, over the declared 300' "HTML over budget"
fresh; printf 'order = ["hero", "how", "install"]\n\n[budget]\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
expect_finding 10 'declares no \[budget\] html_bytes' "a budget without the HTML"
fresh; rm "$F/site/public/js/app.js"
expect_finding 10 'links asset\(s\) the build did not produce: /js/app.js' "a linked asset missing"

# --- interfaces: the list is the data's, and where each runs is the topology's. The model
#     gives one surface (cli); the topology gives the stable feature `good` an interface at
#     /good that only a process serves, so the page lists good, then cli, states the mount
#     and links no published copy
faces() {
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable","weight":5,"commands":["init"]},{"id":"draft","route":"/features/draft/","status":"draft"}],"surfaces":[{"id":"cli","title":"Command line","route":"/docs/cli/"}],"domains":[],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"surfaces":[{"id":"good","category":"interface","feature":"good","mount":"/good","visibility":"public","availability":"served-only"}]}' > "$F/site/data/generated/web.json"
  sed -i.bak 's#<section id="how">#<section id="how"><ul><li data-interface="good">Good, at <code>/good</code></li><li data-interface="cli">Command line</li></ul>#' "$F/site/public/index.html"
  # and the hero opens the first of them, at the page about it, beside a view of it
  sed -i.bak 's#<h1>t</h1>#<h1>t</h1><a href="/features/good/" data-cta="product" data-product="good">open</a><figure data-product-view="good"><dl></dl></figure>#' "$F/site/public/index.html"
  printf '%s\n' '{"statuses":{"issue":["READY","BLOCKED","DONE"]},"issues":[{"id":"I1","status":"READY"},{"id":"I2","status":"DONE"},{"id":"I3","status":"READY"}]}' > "$F/site/data/generated/plan.json"
}
fresh; faces
expect_finding 0 '^OK   interfaces +the homepage lists the 2 interface' "an interface list that is the data's"
fresh; faces; sed -i.bak 's#<li data-interface="cli">Command line</li>##' "$F/site/public/index.html"
expect_finding 10 'lists the interfaces \[good\] and the data gives \[good, cli\]' "an interface of the model the page does not list"
fresh; faces; sed -i.bak 's#<li data-interface="cli">#<li data-interface="typed">typed</li><li data-interface="cli">#' "$F/site/public/index.html"
expect_finding 10 'lists the interfaces \[good, typed, cli\]' "an interface typed into the template"
fresh; faces; sed -i.bak 's#Good, at <code>/good</code>#<a data-published="good" href="/good/">open</a>#' "$F/site/public/index.html"
expect_finding 10 'good is linked as published at /good, and no publication holds it' "a link to a copy nobody publishes"
fresh; faces; sed -i.bak 's#Good, at <code>/good</code>#Good#' "$F/site/public/index.html"
expect_finding 10 'its entry does not state that mount' "an interface with no address"
fresh; faces; printf '%s\n' '{"surfaces":[{"id":"good","category":"interface","feature":"good","mount":"/good","visibility":"public","availability":"served-only"},{"id":"good-snapshot","category":"interface","feature":"good","mount":"/good","visibility":"public","availability":"published-only"}]}' > "$F/site/data/generated/web.json"
expect_finding 10 'the publication holds good-snapshot at /good, and the homepage still describes good as running only locally' "a published surface the page calls local"
sed -i.bak 's#Good, at <code>/good</code>#<a href="/good/" data-published="good">open the snapshot</a>, live at <code>/good</code>#' "$F/site/public/index.html"
expect_finding 10 "the publication holds good-snapshot at /good, and the hero's call to open good does not lead there" "a published product the hero still sends to the page about it"
sed -i.bak 's#<a href="/features/good/" data-cta="product" data-product="good">#<a href="/good/" data-cta="product" data-product="good" data-published-product="good">#' "$F/site/public/index.html"
expect_finding 0 '1 of them held by the publication' "a published surface linked as one"
expect_finding 0 '^OK   product +the hero opens good, the first interface the data gives, at the copy the publication holds' "a published product the hero opens"

# --- product: the hero opens the first interface the data gives, once, where the data says it
#     is, and the view beside it counts what the plan counts
fresh; faces
expect_finding 0 '^OK   product +the hero opens good, the first interface the data gives, at the page that says how to open it' "a hero that opens the product"
fresh; faces; sed -i.bak 's#<a href="/features/good/" data-cta="product" data-product="good">open</a>##' "$F/site/public/index.html"
expect_finding 10 'the hero carries 0 call\(s\) to open the product' "a hero that opens nothing"
fresh; faces; sed -i.bak 's#data-product="good"#data-product="typed"#' "$F/site/public/index.html"
expect_finding 10 'the hero opens typed, and the first interface the data gives is good' "a product typed into the template"
fresh; faces; sed -i.bak 's#<a href="/features/good/" data-cta="product"#<a href="/good/" data-cta="product"#' "$F/site/public/index.html"
expect_finding 10 'the hero opens good at /good, and no publication holds it' "a hero that sends a visitor to an address only a process answers"
fresh; faces; sed -i.bak 's#data-product-view="good"#data-product-view="other"#' "$F/site/public/index.html"
expect_finding 10 'the hero shows the product view\(s\) \[other\], and the product is good' "a view of something else"
# the plan's issues are not the hero's (I3917, rule v2): a count of them returned to the view
# is refused, linked or not, and so is a block marked as the plan's counts that holds none;
# /roadmap/, when built, counts every status the plan gives, in the plan's order
fresh; faces; sed -i.bak 's#<figure data-product-view="good"><dl>#<figure data-product-view="good"><dl data-product-issues><dd data-issues="READY">2</dd><dd data-issues="DONE">1</dd>#' "$F/site/public/index.html"
expect_finding 10 "the hero counts the plan's issues by status \[READY 2, DONE 1\]: the owner took the plan's counts off the hero" "the plan's counts back on the hero"
fresh; faces; sed -i.bak 's#<figure data-product-view="good"><dl>#<figure data-product-view="good"><dl><dd data-issues="READY"><a href="/roadmap/">2</a></dd>#' "$F/site/public/index.html"
expect_finding 10 "the hero counts the plan's issues by status \[READY 2\]" "a linked plan count on the hero"
fresh; faces; sed -i.bak 's#<figure data-product-view="good"><dl>#<figure data-product-view="good"><dl data-product-issues>#' "$F/site/public/index.html"
expect_finding 10 "the hero counts the plan's issues by status \[\]" "an empty block of the plan's counts on the hero"
roadmap() { mkdir -p "$F/site/public/roadmap"; printf '<html><head><title>r</title></head><body><dl data-roadmap-issues><dd><span data-roadmap-issue-status="READY">%s</span> ready <span data-roadmap-issue-status="DONE">1</span> done</dd></dl></body></html>\n' "$1" > "$F/site/public/roadmap/index.html"; sed -i.bak 's#</urlset>#<url><loc>https://x.test/roadmap/</loc></url></urlset>#' "$F/site/public/sitemap.xml"; }
fresh; faces; roadmap 2
expect_finding 0 "^OK   product .*and no count of the plan's issues" "/roadmap/ counting what the plan gives"
fresh; faces; roadmap 7
expect_finding 10 "/roadmap/ counts the plan's issues as \[READY 7, DONE 1\], and the plan gives \[READY 2, DONE 1\]" "a /roadmap/ count the plan does not give"
# and the changes it counts are the newest release record's, group by group, in the
# changelog's order: what landed is read from the records, never said in a sentence
landings() {
  printf '%s\n' '{"sections":[{"version":"2.0.0","unreleased":false,"groups":[{"kind":"feat","changes":[{},{}]},{"kind":"fix","changes":[]},{"kind":"chore","changes":[{}]}]},{"version":"1.0.0","unreleased":false,"groups":[{"kind":"feat","changes":[{}]}]}]}' > "$F/site/data/generated/changelog.json"
  sed -i.bak 's#</dl></figure>#</dl><dl data-product-landings="2.0.0"><dd data-landings="feat">2</dd><dd data-landings="chore">1</dd></dl></figure>#' "$F/site/public/index.html"
}
fresh; faces; landings
expect_finding 0 "^OK   product .*counts what the newest release record counts, and no count of the plan's issues" "a view counting what the newest release carried"
fresh; faces; landings; sed -i.bak 's#data-landings="feat">2#data-landings="feat">5#' "$F/site/public/index.html"
expect_finding 10 "counts v2.0.0's changes as \[feat 5, chore 1\], and the changelog gives \[feat 2, chore 1\]" "a change count the changelog does not give"
fresh; faces; landings; sed -i.bak 's#data-product-landings="2.0.0"#data-product-landings="1.0.0"#' "$F/site/public/index.html"
expect_finding 10 'counts the changes of v1.0.0, and the newest release record is v2.0.0' "an older release presented as the newest"
fresh; faces; landings; sed -i.bak 's#<dl data-product-landings="2.0.0">.*</dl></figure>#</figure>#' "$F/site/public/index.html"
expect_finding 10 'says nothing of what v2.0.0 carried' "a view silent about the release the records hold"
# how far the work travelled is the inferred delivery's count, stage by stage in its order,
# a count may be a link into /roadmap/, and `none` is no stage (I2851, ADR 0166)
delivered() {
  printf '%s\n' '{"stages":["none","branch","merged","released"],"tallies":{"none":5,"branch":0,"merged":1,"released":3}}' > "$F/site/data/generated/delivery.json"
  sed -i.bak 's|<figure data-product-view="good"><dl>|<figure data-product-view="good"><dl data-product-delivery><dd data-delivery="merged"><a href="/roadmap/#delivery">1</a></dd><dd data-delivery="released"><a href="/roadmap/#delivery">3</a></dd></dl><dl>|' "$F/site/public/index.html"
}
fresh; faces; delivered
expect_finding 0 '^OK   product .*counts what the inferred delivery and the newest release record count' "a view counting the inferred delivery, its counts links"
fresh; faces; delivered; sed -i.bak 's|data-delivery="released"><a href="/roadmap/#delivery">3|data-delivery="released"><a href="/roadmap/#delivery">4|' "$F/site/public/index.html"
expect_finding 10 'counts the delivery as \[merged 1, released 4\], and the inferred delivery gives \[merged 1, released 3\]' "a stage count the delivery does not give"
fresh; faces; delivered; sed -i.bak 's|<dl data-product-delivery>.*</dl><dl>|<dl>|' "$F/site/public/index.html"
expect_finding 10 'counts no delivery stage, and the inferred delivery gives \[merged 1, released 3\]' "a view silent about the delivery the history proves"
fresh; faces; delivered; sed -i.bak 's|<dl data-product-delivery>|<dl data-product-delivery><dd data-delivery="none">5</dd>|' "$F/site/public/index.html"
expect_finding 10 'counts the delivery as \[none 5, merged 1, released 3\]' "the absence of a stage drawn as one"
fresh; sed -i.bak 's#<h1>t</h1>#<h1>t</h1><a href="/x/" data-cta="product" data-product="x">open</a>#' "$F/site/public/index.html"
expect_finding 10 'the hero opens a product, and the data gives no interface a person opens' "a product where the data gives none"
# the other way a publication holds a surface: the build names a snapshot of it (ADR 0141),
# which the topology does not carry because it exists only where its producer ran
snap() { printf '%s\n' '{"commit":"abcdef1234567890","dirty":false,"snapshots":[{"id":"good","mount":"/good"}],"release":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/build.json"; }
fresh; faces; snap
expect_finding 10 'the publication holds a snapshot of good at /good, and the homepage still describes good as running only locally' "a snapshot the page does not link"
sed -i.bak 's#Good, at <code>/good</code>#<a href="/elsewhere/" data-published="good">open</a>, live at <code>/good</code>#' "$F/site/public/index.html"
expect_finding 10 'the link does not lead to its mount /good/' "a snapshot link that leads somewhere else"
fresh; faces; snap; sed -i.bak 's#Good, at <code>/good</code>#<a href="/good/" data-published="good">open the snapshot</a>, live at <code>/good</code>#' "$F/site/public/index.html"
sed -i.bak 's#<a href="/features/good/" data-cta="product" data-product="good">#<a href="/good/" data-cta="product" data-product="good" data-published-product="good">#' "$F/site/public/index.html"
expect_finding 0 '1 of them held by the publication' "a snapshot linked as one"
fresh; faces; rm "$F/site/data/generated/web.json"
expect_finding 10 'site/data/generated/web.json is missing' "an interface list with no topology to hold it to"

# --- indexing, each direction
fresh; printf '<html><head><title>i</title></head><body>i</body></html>\n' > "$F/site/public/plan/i0001/index.html"
expect_finding 10 '/plan/i0001/ is below an unlisted section and carries no noindex' "an unlisted page without noindex"
fresh; sed -i.bak 's#</urlset>#<url><loc>https://x.test/plan/i0001/</loc></url></urlset>#' "$F/site/public/sitemap.xml"
expect_finding 10 '/plan/i0001/ is below an unlisted section and is in sitemap.xml' "an unlisted page in the sitemap"
fresh; sed -i.bak 's#<url><loc>https://x.test/features/good/</loc></url>##' "$F/site/public/sitemap.xml"
expect_finding 10 '/features/good/ is indexable and absent from sitemap.xml' "an indexable page missing from the sitemap"

# --- the composed surface's pages are not the site's: the clean tree above carried /ref/ and
#     /ref/api/, out of the sitemap, without noindex, one loading the Mermaid runtime with
#     nothing to draw, and passed. The exemption is the topology's and nothing else's: the
#     same tree with /ref no longer declared is judged like any other page, and a tree whose
#     topology cannot be read is not judged at all rather than judged without it
fresh; printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
expect_finding 10 '/ref/api/ is indexable and absent from sitemap.xml' "a page under a mount the topology does not compose"
grep -q 'load the Mermaid runtime with no diagram to render: .*/ref/' out.txt \
  || { echo "    a page under an undeclared mount escaped the runtime check"; cat out.txt; exit 1; }
fresh; printf '%s\n' '{"surfaces":[{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"served-only"}]}' > "$F/docs/generated/web.json"
expect_finding 10 '/ref/ is indexable and absent from sitemap.xml' "a served-only surface is not composed into the publication"
fresh; rm "$F/docs/generated/web.json"
expect_finding 12 'web.json is missing' "a topology that cannot be read"

# --- the product map is the model's domains, both ways (ADR 0104). A tree whose model shows
#     one domain, drawn with its member and page linked and its problem rendered, passes; the
#     map drawing a domain the model does not show, missing one it does, leaving a member out,
#     or a declared problem that is not a shown domain each fail
domains() { # the model shows domain alpha, whose one member is the stable feature `good`
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable","domain":"alpha","weight":1,"commands":["init"]},{"id":"draft","route":"/features/draft/","status":"draft"}],"domains":[{"id":"alpha","route":"/domains/alpha/","status":"stable","features":[{"id":"good","route":"/features/good/"}]},{"id":"empty","route":"/domains/empty/","status":"stable","features":[]}],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf 'order = ["hero", "how", "install"]\nproblems = ["alpha"]\n\n[budget]\nhtml_bytes = 4000\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  promises
  sed -i.bak 's#<section id="how">#<section id="how"><a href="\#domain-alpha" data-problem="alpha">p</a><ol><li id="domain-alpha" data-domain="alpha"><a href="/features/good/">good</a><a href="/domains/alpha/">alpha</a></li></ol><p data-node-features="problem:alpha"><a href="/features/good/" data-feature="good">good</a></p>#' "$F/site/public/index.html"
}
fresh; domains
expect_finding 0 '^OK   map +the map draws the 1 domain' "a map that draws the model's domains"
fresh; domains; sed -i.bak 's#<li id="domain-alpha"#<li data-domain="typed"></li><li id="domain-alpha"#' "$F/site/public/index.html"
expect_finding 10 'the map draws \[typed, alpha\]' "a domain typed into the template"
fresh; domains; sed -i.bak 's#data-domain="alpha"#data-x="alpha"#' "$F/site/public/index.html"
expect_finding 10 'the product model shows \[alpha\]' "a shown domain the map does not draw"
fresh; domains; sed -i.bak 's#data-domain="alpha"><a href="/features/good/">good</a>#data-domain="alpha">#' "$F/site/public/index.html"
expect_finding 10 'member good is not linked from its own map entry' "a member linked elsewhere on the page and not in its entry"
fresh; sed -i.bak 's#"domains":\[\],##' "$F/site/data/registry/product.json"
expect_finding 10 'carries no domains key' "a model with no domains key is not a model with no domains"
fresh; domains; printf 'order = ["hero", "how", "install"]\nproblems = ["empty"]\n\n[budget]\nhtml_bytes = 4000\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
expect_finding 10 "problem 'empty', which is not a domain the map shows" "a problem whose domain is not shown"

# --- promises: a declared word is held to its claim, and a feature that is not stable is not named
# Run with --only promises: the claims these mutations add would move the proof band's figure,
# which is the declared check's business, not this one's.
promise_finding() { # <exit> <pattern> <what>
  rc=0; MJ_ROOT="$F" "$CHECK" --only promises > out.txt 2>&1 || rc=$?
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}
claims() { # <status of c-ok> <supported true|false>
  printf '{"status_order":["guaranteed","planned"],"claims":[{"id":"c-ok","status":"%s"},{"id":"c-planned","status":"planned"}]}\n' "$1" > "$F/site/data/generated/capabilities.json"
  printf '"self-healing" = "c-ok"\n"autonomous" = "c-planned"\n' >> "$F/site/data/homepage.toml"
  sed -i.bak "s#\"evidence\":{\"available\":false,\"claims\":{}}#\"evidence\":{\"available\":true,\"claims\":{\"c-ok\":{\"supported\":$2}}}#" "$F/site/data/registry/product.json"
}
copy() { printf '[hero]\ntitle = "x"\nlead = "%s"\n' "$1" > "$F/site/data/marketing.toml"; }
fresh; claims guaranteed true
promise_finding 0 '^OK   promises .*self-healing \(c-ok: permitted\)' "a clean copy with every declaration resolving"
fresh; claims guaranteed true; copy "It works Autonomous of you."
promise_finding 10 "site/data/marketing.toml:3 promises 'autonomous': claim 'c-planned' is planned" "a word whose claim is planned"
fresh; claims guaranteed true; copy "Unattended runs, all night."
promise_finding 10 "site/data/marketing.toml:3 promises 'unattended': no claim permits it" "a word no claim permits"
fresh; claims guaranteed true; copy "A self-healing queue."
promise_finding 0 '^OK   promises ' "a word whose claim is guaranteed and supported by a recorded run"
fresh; claims guaranteed false; copy "A self-healing queue."
promise_finding 10 "promises 'self-healing': claim 'c-ok' is guaranteed and no recorded run supports it" "a guaranteed claim no run supports"
fresh; claims planned true; copy "A self-healing queue."
promise_finding 10 "promises 'self-healing': claim 'c-ok' is planned" "the same word once its claim is only planned"
fresh; claims guaranteed true; printf '"zero-touch" = "c-missing"\n' >> "$F/site/data/homepage.toml"
promise_finding 10 "\[promises\] 'zero-touch' is permitted by claim 'c-missing', which docs/CLAIMS.yaml does not hold" "a declaration naming a claim that does not exist"
fresh; claims guaranteed true; printf 'order = ["hero", "how", "install"]\n' > "$F/site/data/homepage.toml"
promise_finding 10 'declares no \[promises\]' "no declaration at all"
fresh; claims guaranteed true
sed -i.bak 's#{"id":"draft","route":"/features/draft/","status":"draft"}#{"id":"draft","route":"/features/draft/","status":"draft","short_title":"Deployments"}#' "$F/site/data/registry/product.json"
printf '[intro]\nlead = "Deployments are one object."\n' > "$F/site/data/manifesto.toml"
promise_finding 10 "site/data/manifesto.toml:2 names feature 'draft' \('Deployments'\), which the product model calls draft" "a draft feature named in the copy"
# --- sentence, scenarios and nodes (I3134, I3223, I3224): every projection says the declared
# sentence, the hero offers what the recorded refusals derive, a node names what the model derives
fresh; printf '# m\n\n**Keep the work going**\n' > "$F/README.md"
expect_finding 10 "README.md's bold line says 'Keep the work going'" "a README saying it in other words"
fresh; sed -i.bak 's#<title>Fixture — Keep the work moving</title>#<title>Fixture — keep the work moving</title>#' "$F/site/public/index.html"
expect_finding 10 "the homepage title says 'Fixture — keep the work moving', not the declared sentence" "a title that lowercases the sentence"
fresh; sed -i.bak 's#<a href="/use-cases/a/" data-scenario="a">a</a>##' "$F/site/public/index.html"
expect_finding 10 "the hero offers \\[\\], terminal.json's recorded refusals derive \\['a'\\]" "a recorded refusal not offered"
fresh; sed -i.bak 's#<li><a href="/features/good/" data-feature="good">good</a></li>##' "$F/site/public/index.html"
expect_finding 10 "step:init names \\[\\], the product model derives \\['good'\\]" "a step naming none of its features"
# an input only these checks read is their finding when missing, and stops no other check
fresh; rm "$F/site/data/generated/terminal.json"
expect_finding 10 'scenarios +site/data/generated/terminal.json missing' "terminal.json missing"
grep -qE '^FAIL nodes +site/data/generated/terminal.json missing' out.txt || { echo "    nodes did not report terminal.json missing"; cat out.txt; exit 1; }
grep -q '^OK   narrative ' out.txt || { echo "    a missing terminal.json stopped the other checks"; cat out.txt; exit 1; }
rc=0; MJ_ROOT="$F" "$CHECK" --only evidence > out.txt 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    --only evidence exited $rc when terminal.json was missing"; cat out.txt; exit 1; }

# --- present: the template's text states no present it does not read (case 3105 holds the rest)
fresh; printf '<p>Milestones under way</p>\n' >> "$F/site/templates/index.html"
expect_finding 10 "present +site/templates/index.html:3 states the present \('under way'\)" "a template sentence stating how the work stands"

# a copy file that is missing is a finding of promises alone: every other check still runs, and a
# caller asking --only for another check is not stopped (case 544 builds a tree without them)
fresh; rm "$F/site/data/marketing.toml"
expect_finding 10 'promises +site/data/marketing.toml cannot be read as TOML' "a missing copy file"
grep -q '^OK   narrative ' out.txt || { echo "    a missing copy file stopped the other checks"; cat out.txt; exit 1; }
rc=0; MJ_ROOT="$F" "$CHECK" --only narrative > out.txt 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    --only narrative exited $rc when a copy file was missing"; cat out.txt; exit 1; }
exit 0
