# majordomus-covers: none
# A published page reads delivery at the commit it was built from, and names one commit (I3769).
#
# The committed delivery projection can only observe the commit before the one it lands in
# (case 51: a derived file cannot name the commit it lands in), so the homepage's delivery card
# named a commit older than the page and a visitor could not tell which reading was current.
# scripts/site-build now reads `plan delivery --at` its own commit into the build's provenance
# (site/data/build.json, never committed), the homepage and /roadmap/ read that reading when
# the build carries one, and scripts/ci/homepage-check holds a clean build to it.
#
# What is proved, against a fixture this case builds rather than the site in the checkout:
#   1  a clean build whose card names its own commit, and whose reading was taken there, passes
#   2  a clean build whose card names an older commit is refused, naming both commits
#   3  a clean build whose own reading was observed elsewhere is refused, naming both commits
#   4  a dirty build is rendered from bytes no commit holds, and is not judged on either
#   5  the templates read the build's reading when there is one and the committed projection
#      otherwise, so a plain `zola serve` still renders delivery
# Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
C=abcdef1234567890     # the commit the fixture's build was made from (its trust card says abcdef1)
O=0123456789abcdef     # an older commit, where a committed projection would have been observed

# the fixture of case 333, with the hero's delivery card and a build record that carries the
# build's own delivery reading; everything else is what every other check of the script reads
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
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable","weight":1,"commands":["init"]},{"id":"draft","route":"/features/draft/","status":"draft"}],"domains":[],"providers":[{"id":"one","title":"Tool One"},{"id":"two","title":"Tool Two","route":"/providers/two/"}],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"curl -fsSL https://x.test/install.sh | sh","next_command":"majordomus init","verify_command":"majordomus --version","latest":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"}}' > "$F/site/data/registry/distribution.json"
  mkdir -p "$F/site/data/generated" "$F/docs/generated" "$F/site/public/ref/api"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"},{"id":"ref","kind":"static-directory","mount":"/ref","artifact":"target/web/ref","availability":"both"}]}' > "$F/docs/generated/web.json"
  printf '<html><head><title>r</title><script src="https://x.test/js/mermaid.min.js"></script></head><body>r</body></html>\n' > "$F/site/public/ref/index.html"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/ref/api/index.html"
  printf '%s\n' '{"use_cases":[{"id":"a"},{"id":"b"},{"id":"c"}],"categories":[{"id":"x"},{"id":"y"}]}' > "$F/site/data/generated/catalogue.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  # the build's provenance: clean, made from $C, and its own delivery reading observed there
  printf '{"commit":"%s","dirty":false,"release":{"version":"9.9.9","published_at":"2026-09-16T00:00:00Z"},"delivery":{"observed":{"commit":"%s","dirty":false},"stages":[],"tallies":{},"issues":[]}}\n' "$C" "$C" > "$F/site/data/build.json"
  printf '# m\n\n**Keep the work moving**\n' > "$F/README.md"
  printf 'description = "Keep the work moving"\n' > "$F/site/config.toml"
  printf '%s\n' '{"tagline":"Keep the work moving"}' > "$F/site/data/generated/project.json"
  mkdir -p "$F/share/design/brand" "$F/site/public/use-cases/a"
  printf '<svg><text x="0" fill="#cbd5e1">Keep the work moving</text></svg>\n' > "$F/share/design/brand/social-card.svg"
  printf '# Launch\n\n## One sentence\n\nKeep the work moving\n' > "$F/docs/LAUNCH.md"
  printf '%s\n' '{"scenarios":[{"use_case":"a","route":"/use-cases/a/"}],"walkthrough":[{"label":"init"}]}' > "$F/site/data/generated/terminal.json"
  printf '<html><head><title>a</title></head><body>a</body></html>\n' > "$F/site/public/use-cases/a/index.html"
  cat > "$F/site/public/index.html" <<HTML
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
<p>Delivery the history proves, inferred at <a href="https://x.test/commit/$C" data-product-delivery-commit="$C">abcdef1</a></p>
<dl data-hero-figures><div><dt><a href="/features/good/">good</a></dt><dd>1</dd></div></dl>
<dl data-claims-proof><div><dt>guaranteed claims, declared</dt><dd data-declared="guaranteed">0</dd><dd data-supported="unmeasured">not yet measured</dd></div></dl>
<div data-surfaces x-data="{}"><select data-surface-picker><option value="one.show" selected>one.show</option></select>
<dl><div><dd><a data-surface-id="one.show" href="/r/">one.show</a></dd></div><div><dd data-surface="cli">majordomus one show</dd></div><div><dd><a data-surface="http" href="/docs/api/">GET /api/v1/one</a></dd></div><div><dd data-surface="mcp">majordomus_one</dd></div></dl></div></section>
<section id="how"><dl><dt>x</dt><dd>7</dd></dl></section>
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
run() { MJ_ROOT="$F" MJ_HOMEPAGE_ONLY=product "$CHECK" > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -q -- "$2" out.txt || { echo "    $3: no finding matching /$2/"; cat out.txt; exit 1; }
}
build_json() { jq "$1" "$F/site/data/build.json" > "$F/site/data/build.json.tmp" && mv "$F/site/data/build.json.tmp" "$F/site/data/build.json"; }

# ---------------------------------------------------------------- 1  a clean build names its own commit
fresh
expect_finding 0 '^OK   product ' "a clean build whose card names the build commit, read there"
echo "    ok   a_built_page_reads_delivery_at_its_build_commit"

# ---------------------------------------------------------------- 2  the card names an older commit
fresh; sed -i.bak "s#data-product-delivery-commit=\"$C\"#data-product-delivery-commit=\"$O\"#" "$F/site/public/index.html"
expect_finding 10 "the hero names delivery inferred at ${O:0:11}, and this clean build was made from ${C:0:11}" "a card naming an older commit on a clean build"

# ---------------------------------------------------------------- 3  the build's reading was taken elsewhere
fresh; build_json ".delivery.observed.commit = \"$O\""
expect_finding 10 "the build's own delivery reading was observed at ${O:0:11}, not at the build commit ${C:0:11}" "a reading observed elsewhere on a clean build"

# ---------------------------------------------------------------- 4  a dirty build is not judged
fresh; sed -i.bak "s#data-product-delivery-commit=\"$C\"#data-product-delivery-commit=\"$O\"#" "$F/site/public/index.html"
build_json ".dirty = true | .delivery.observed.commit = \"$O\" | .delivery.observed.dirty = true"
expect_finding 0 '^OK   product ' "a dirty build naming an older commit"

# ---------------------------------------------------------------- 5  the templates read the build's reading, and fall back
for t in site/templates/index.html site/templates/roadmap.html; do
  grep -q 'load_data(path="data/generated/delivery.json", required=false)' "$ROOT/$t" \
    || { echo "    $t no longer reads the committed delivery projection"; exit 1; }
  grep -q '{%- if build and build.delivery %}{% set delivery = build.delivery %}{% endif -%}' "$ROOT/$t" \
    || { echo "    $t does not read the delivery reading of the build when the build carries one"; exit 1; }
done
grep -q 'delivery_at_build "$commit" "$dirty" "$DELIVERY"' "$ROOT/scripts/site-build" \
  || { echo "    scripts/site-build does not read delivery at its build commit"; exit 1; }
echo "    ok   a_render_without_a_build_record_falls_back_to_committed_delivery"

# ---------------------------------------------------------------- 6  no executable: the committed projection, never a failed build
# The real function, lifted out of scripts/site-build and run against a stub launcher: a refusal
# for want of an executable (12, lines prefixed majordomus-cli) records no reading and returns 0;
# any other failure of the reading still stops the build with 12 (2026-11-07, run 42339976144).
FN="$PWD/delivery_at_build.sh"
sed -n '/^delivery_at_build() {/,/^}/p' "$ROOT/scripts/site-build" > "$FN"
grep -q '^delivery_at_build() {' "$FN" || { echo "    delivery_at_build could not be lifted out of scripts/site-build"; exit 1; }
stub_root() { # <exit> <stderr line>
  R="$PWD/stub-root"; rm -rf "$R"; mkdir -p "$R/bin"
  printf '#!/usr/bin/env bash\necho "%s" >&2\nexit %s\n' "$2" "$1" > "$R/bin/majordomus"; chmod +x "$R/bin/majordomus"
}
lifted() { ( ROOT="$R"; . "$FN"; delivery_at_build 0123456789abcdef0123456789abcdef01234567 false "$PWD/dl.json" ) > lifted.out 2>&1; echo $?; }
stub_root 12 "majordomus-cli: /x/target/debug/majordomus is missing, and no ordinary command builds it (ADR 0224)"
rc="$(lifted)"
[ "$rc" = 0 ] || { echo "    a missing executable stopped the build (exit $rc):"; cat lifted.out; exit 1; }
[ "$(cat "$PWD/dl.json")" = null ] || { echo "    a missing executable left a reading behind:"; cat "$PWD/dl.json"; exit 1; }
grep -q 'no executable of this tree, so delivery is not read' lifted.out || { echo "    a missing executable was not said:"; cat lifted.out; exit 1; }
stub_root 12 "plan delivery: the history could not be read"
rc="$(lifted)"
[ "$rc" = 12 ] || { echo "    a reading the tool could not give did not stop the build (exit $rc):"; cat lifted.out; exit 1; }
echo "    ok   a_build_without_the_executable_reads_the_committed_delivery"
