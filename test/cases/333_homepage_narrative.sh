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
# the homepage's scripts or inline JSON over the declared budget, no budget declared, and a
# linked asset the build did not produce. Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/public/plan/i0001" "$F/site/public/features/good" "$F/site/public/features/draft"
  printf 'order = ["hero", "how", "install"]\n\n[budget]\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  mkdir -p "$F/site/public/js" "$F/site/public/docs/plain" "$F/site/public/docs/diagram"
  printf 'x%.0s' $(seq 1 40) > "$F/site/public/js/app.js"
  printf '<html><head><title>p</title></head><body>plain</body></html>\n' > "$F/site/public/docs/plain/index.html"
  printf '<html><head><title>d</title><script src="https://x.test/js/mermaid.min.js"></script></head><body><pre class="mermaid">graph TD</pre></body></html>\n' > "$F/site/public/docs/diagram/index.html"
  printf '[[groups]]\nlabel = "Plan"\nhref = "/plan/"\n\n[indexing]\nunlisted = ["plan"]\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable"},{"id":"draft","route":"/features/draft/","status":"draft"}],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}]}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"curl -fsSL https://x.test/install.sh | sh","next_command":"majordomus init","verify_command":"majordomus --version","latest":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/registry/distribution.json"
  mkdir -p "$F/site/data/generated" "$F/docs/generated" "$F/site/public/ref/api"
  # the topology: the app, and one surface site-build composes at /ref, whose pages are that
  # surface's own and follow neither the story nor the indexing policy (ADR 0086)
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}' > "$F/docs/generated/web.json"
  printf '<html><head><title>r</title><script src="https://x.test/js/mermaid.min.js"></script></head><body>r</body></html>\n' > "$F/site/public/ref/index.html"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/ref/api/index.html"
  printf '%s\n' '{"use_cases":[{"id":"a"},{"id":"b"},{"id":"c"}],"categories":[{"id":"x"},{"id":"y"}]}' > "$F/site/data/generated/catalogue.json"
  printf '%s\n' '{"commit":"abcdef1234567890","dirty":false}' > "$F/site/data/build.json"
  cat > "$F/site/public/index.html" <<'HTML'
<html><head><title>home</title><script defer src="https://x.test/js/app.js"></script></head><body><main>
<section id="hero"><h1>t</h1><a href="/features/good/">good</a>
<dl><dt>generated for</dt><dd data-providers><span>Tool One</span><a href="/providers/two/">Tool Two</a></dd></dl>
<dl data-trust>
<div data-trust-key="release"><dt>release</dt><dd>9.9.9</dd></div>
<div data-trust-key="verify"><dt>confirm</dt><dd>majordomus --version</dd></div>
<div data-trust-key="commit"><dt>commit</dt><dd>abcdef1</dd></div>
<div data-trust-key="use-cases"><dt>use cases</dt><dd>3</dd><dd>2 areas</dd></div>
</dl></section>
<section id="how"><dl><dt>x</dt><dd>7</dd></dl></section>
<section id="install"><code>curl -fsSL https://x.test/install.sh | sh</code> then <code>majordomus init</code>, check with <code>majordomus --version</code> <a href="/getting-started/">go</a></section>
</main></body></html>
HTML
  printf '<html><head><title>plan</title></head><body>plan</body></html>\n' > "$F/site/public/plan/index.html"
  printf '<html><head><title>i</title><meta name="robots" content="noindex"></head><body>i</body></html>\n' > "$F/site/public/plan/i0001/index.html"
  printf '<html><head><title>g</title></head><body>g</body></html>\n' > "$F/site/public/features/good/index.html"
  printf '<html><head><title>d</title></head><body>d</body></html>\n' > "$F/site/public/features/draft/index.html"
  cat > "$F/site/public/sitemap.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<urlset><url><loc>https://x.test/</loc></url><url><loc>https://x.test/docs/plain/</loc></url><url><loc>https://x.test/docs/diagram/</loc></url><url><loc>https://x.test/plan/</loc></url><url><loc>https://x.test/features/good/</loc></url><url><loc>https://x.test/features/draft/</loc></url></urlset>
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
for c in substance honesty install runtime providers trust weight indexing; do grep -q "^OK   $c " out.txt || { echo "    a clean tree did not report $c"; cat out.txt; exit 1; }; done

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
fresh; rm "$F/site/public/js/app.js"
expect_finding 10 'links asset\(s\) the build did not produce: /js/app.js' "a linked asset missing"

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
exit 0
