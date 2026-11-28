# majordomus-covers: none
# scripts/ci/homepage-check holds the hero's delivery counts to the delivery the page renders (I4810).
#
# site/templates/index.html renders the build's own delivery reading when site/data/build.json
# carries one (I3769) and the committed site/data/generated/delivery.json otherwise. The count
# check (I2851) still read the committed projection alone, so every master deploy whose trunk
# had moved past that projection's observation was refused: pages runs 45538902097,
# 45562941508 and 45614492427 on 46775282570 (2026-11-25), "the product view counts the
# delivery as [merged 4, released 252], and the inferred delivery gives [merged 2, released 252]",
# while the page showed exactly the build's reading.
#
# What is proved, against a fixture this case builds (case 333's, with a product view):
#   1  build.json carries a reading whose tallies differ from the committed projection, and the
#      hero renders the reading: no product finding
#   2  the same reading, and the hero renders the committed tallies: a product finding, naming
#      the build's reading as the source it was compared with
#   3  no build.json, a build.json without delivery, or delivery null: the committed projection
#      is compared exactly as before, and a finding names it
#   4  a build reading and no committed projection: the page renders the reading, and the
#      missing committed file is not a finding
#   5  a mutant of the checker that reads the committed projection alone fails situation 1
# Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
C=abcdef1234567890     # the commit the fixture's build was made from, where its reading was taken
O=0123456789abcdef     # the older commit the committed projection observed

# case 333's fixture (as case 3742 copies it), with case 333's one interface a person opens,
# so the hero opens it beside a product view, and the view's delivery card
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/public/plan/i0001" "$F/site/public/features/good" "$F/site/public/features/draft"
  printf 'order = ["hero", "how", "install"]\n\n[budget]\nhtml_bytes = 4000\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  printf '\n[promises]\n"unattended" = ""\n' >> "$F/site/data/homepage.toml"
  printf '[hero]\ntitle = "Keep the work moving"\n' > "$F/site/data/marketing.toml"
  printf '# a comment may say unattended: comments are not copy\n[intro]\nlead = "The repository decides what is done."\n' > "$F/site/data/manifesto.toml"
  mkdir -p "$F/site/templates"
  printf '{#- a comment may say what is shipped today: comments are not text -#}\n<section id="hero"><h1>{{ m.hero.title }}</h1></section>\n' > "$F/site/templates/index.html"
  mkdir -p "$F/site/public/js" "$F/site/public/docs/plain" "$F/site/public/docs/diagram"
  printf 'x%.0s' $(seq 1 40) > "$F/site/public/js/app.js"
  printf '<html><head><title>p</title></head><body>plain</body></html>\n' > "$F/site/public/docs/plain/index.html"
  printf '<html><head><title>d</title><script src="https://x.test/js/mermaid.min.js"></script></head><body><pre class="mermaid">graph TD</pre></body></html>\n' > "$F/site/public/docs/diagram/index.html"
  printf '[[groups]]\nlabel = "Plan"\nhref = "/plan/"\n\n[indexing]\nunlisted = ["plan"]\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable","weight":5,"commands":["init"]},{"id":"draft","route":"/features/draft/","status":"draft"}],"surfaces":[{"id":"cli","title":"Command line","route":"/docs/cli/"}],"domains":[],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"curl -fsSL https://x.test/install.sh | sh","next_command":"majordomus init","verify_command":"majordomus --version","latest":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/registry/distribution.json"
  mkdir -p "$F/site/data/generated" "$F/docs/generated" "$F/site/public/ref/api"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{"surfaces":[{"id":"good","category":"interface","feature":"good","mount":"/good","visibility":"public","availability":"served-only"}]}' > "$F/site/data/generated/web.json"
  printf '%s\n' '{"statuses":{"issue":["READY","BLOCKED","DONE"]},"issues":[{"id":"I1","status":"READY"},{"id":"I2","status":"DONE"},{"id":"I3","status":"READY"}]}' > "$F/site/data/generated/plan.json"
  printf '<html><head><title>r</title><script src="https://x.test/js/mermaid.min.js"></script></head><body>r</body></html>\n' > "$F/site/public/ref/index.html"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/ref/api/index.html"
  printf '%s\n' '{"use_cases":[{"id":"a"},{"id":"b"},{"id":"c"}],"categories":[{"id":"x"},{"id":"y"}]}' > "$F/site/data/generated/catalogue.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  # the committed projection, observed at the older commit: merged 1, released 3
  printf '{"observed":{"commit":"%s"},"stages":["none","branch","merged","released"],"tallies":{"none":5,"branch":0,"merged":1,"released":3}}\n' "$O" > "$F/site/data/generated/delivery.json"
  # the build's provenance: clean, made from $C, and its own reading taken there: merged 2, released 3
  printf '{"commit":"%s","dirty":false,"release":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"},"delivery":{"observed":{"commit":"%s","dirty":false},"stages":["none","branch","merged","released"],"tallies":{"none":4,"branch":0,"merged":2,"released":3},"issues":[]}}\n' "$C" "$C" > "$F/site/data/build.json"
  printf '# m\n\n**Keep the work moving**\n' > "$F/README.md"
  printf 'description = "Keep the work moving"\n' > "$F/site/config.toml"
  printf '%s\n' '{"tagline":"Keep the work moving"}' > "$F/site/data/generated/project.json"
  mkdir -p "$F/share/design/brand" "$F/site/public/use-cases/a"
  printf '<svg><text x="0" fill="#cbd5e1">Keep the work moving</text></svg>\n' > "$F/share/design/brand/social-card.svg"
  printf '# Launch\n\n## One sentence\n\nKeep the work moving\n' > "$F/docs/LAUNCH.md"
  printf '%s\n' '{"scenarios":[{"use_case":"a","route":"/use-cases/a/"}],"walkthrough":[{"label":"init"}]}' > "$F/site/data/generated/terminal.json"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/use-cases/a/index.html"
  # the hero renders the build's reading, as site/templates/index.html does when build.json carries one
  cat > "$F/site/public/index.html" <<'HTML'
<html><head><title>Fixture — Keep the work moving</title><script defer src="https://x.test/js/app.js"></script></head><body><main>
<section id="hero"><h1>t</h1><a href="/features/good/" data-cta="product" data-product="good">open</a><figure data-product-view="good"><dl data-product-delivery><dd data-delivery="merged"><a href="/roadmap/#delivery">2</a></dd><dd data-delivery="released"><a href="/roadmap/#delivery">3</a></dd></dl><dl></dl></figure><a href="/features/good/">good</a>
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
<section id="how"><ul><li data-interface="good">Good, at <code>/good</code></li><li data-interface="cli">Command line</li></ul><dl><dt>x</dt><dd>7</dd></dl></section>
<section id="install"><code>curl -fsSL https://x.test/install.sh | sh</code> then <code>majordomus init</code>, check with <code>majordomus --version</code> <a href="/getting-started/">go</a></section>
</main></body></html>
HTML
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
renders() { # <merged> — the count the hero's merged cell shows
  sed -i.bak "s|data-delivery=\"merged\"><a href=\"/roadmap/#delivery\">[0-9]*|data-delivery=\"merged\"><a href=\"/roadmap/#delivery\">$1|" "$F/site/public/index.html"
}
build_json() { jq "$1" "$F/site/data/build.json" > "$F/site/data/build.json.tmp" && mv "$F/site/data/build.json.tmp" "$F/site/data/build.json"; }
run() { MJ_ROOT="$F" "${CHECKER:-$CHECK}" --only product > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE -- "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# ---------------------------------------------------------------- 1  the hero renders the build's reading
fresh
expect_finding 0 '^OK   product .*counts what the inferred delivery and the newest release record count' \
  "a hero rendering the build's delivery reading, which differs from the committed projection"
echo "    ok   homepage_check_compares_the_delivery_the_page_renders"

# ---------------------------------------------------------------- 2  the hero renders the committed tallies instead
fresh; renders 1
expect_finding 10 "counts the delivery as \[merged 1, released 3\], and the inferred delivery gives \[merged 2, released 3\] \(the build's delivery reading at ${C:0:11}\)" \
  "a hero rendering the committed tallies while the build carries its own reading"
echo "    ok   a_hero_counting_other_than_the_builds_reading_is_refused_naming_it"

# ---------------------------------------------------------------- 3  no reading: the committed projection, as before
for variant in absent no-key null; do
  for shown in 1 2; do
    fresh; renders "$shown"
    case "$variant" in
      absent) rm "$F/site/data/build.json" ;;
      no-key) build_json 'del(.delivery)' ;;
      null)   build_json '.delivery = null' ;;
    esac
    if [ "$shown" = 1 ]; then
      expect_finding 0 '^OK   product ' "build.json $variant, a hero rendering the committed projection"
    else
      expect_finding 10 "counts the delivery as \[merged 2, released 3\], and the inferred delivery gives \[merged 1, released 3\] \(the committed projection at ${O:0:11}\)" \
        "build.json $variant, a hero counting other than the committed projection"
    fi
  done
done
# and a committed projection silent about where it was observed is still named as that projection
fresh; renders 2; build_json 'del(.delivery)'
jq 'del(.observed)' "$F/site/data/generated/delivery.json" > "$F/d.tmp" && mv "$F/d.tmp" "$F/site/data/generated/delivery.json"
expect_finding 10 "and the inferred delivery gives \[merged 1, released 3\] \(the committed projection\)$" \
  "a committed projection with no observed commit"
echo "    ok   without_a_build_reading_the_committed_projection_is_compared"

# ---------------------------------------------------------------- 4  a reading and no committed projection
fresh; rm "$F/site/data/generated/delivery.json"
expect_finding 0 '^OK   product ' "a build reading rendered, and no committed projection"
fresh; rm "$F/site/data/generated/delivery.json"; build_json 'del(.delivery)'
expect_finding 10 'the product view counts delivery stages, and site/data/generated/delivery.json is missing' \
  "neither a reading nor a committed projection, and the hero counts stages"
echo "    ok   a_build_reading_needs_no_committed_projection"

# ---------------------------------------------------------------- 5  the mutant that reads the committed projection alone
# The checker, lifted beside a link to the real lib/ it sources, with the build's reading
# switched off: situation 1 must then fail, or this case proves nothing about the source.
M="$PWD/mutant"; rm -rf "$M"; mkdir -p "$M/scripts/ci"; ln -s "$ROOT/lib" "$M/lib"
sed 's/^    if build and build.get("delivery"):$/    if False:/' "$CHECK" > "$M/scripts/ci/homepage-check"; chmod +x "$M/scripts/ci/homepage-check"
cmp -s "$CHECK" "$M/scripts/ci/homepage-check" && { echo "    the mutation could not be applied: the checker no longer chooses the build's reading on that line"; exit 1; }
fresh
rc="$(CHECKER="$M/scripts/ci/homepage-check" run)"
[ "$rc" = 10 ] || { echo "    the mutant reading the committed projection alone passed situation 1 (exit $rc)"; cat out.txt; exit 1; }
grep -qE "counts the delivery as \[merged 2, released 3\], and the inferred delivery gives \[merged 1, released 3\]" out.txt \
  || { echo "    the mutant failed situation 1 for another reason"; cat out.txt; exit 1; }
echo "    ok   the_mutant_reading_the_committed_projection_alone_is_caught"
