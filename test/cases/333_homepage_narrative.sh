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
# indexable page absent from the sitemap. Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/public/plan/i0001" "$F/site/public/features/good" "$F/site/public/features/draft"
  printf 'order = ["hero", "how", "install"]\n' > "$F/site/data/homepage.toml"
  printf '[[groups]]\nlabel = "Plan"\nhref = "/plan/"\n\n[indexing]\nunlisted = ["plan"]\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[{"id":"good","route":"/features/good/","status":"stable"},{"id":"draft","route":"/features/draft/","status":"draft"}]}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"curl -fsSL https://x.test/install.sh | sh","next_command":"majordomus init","verify_command":"majordomus --version"}' > "$F/site/data/registry/distribution.json"
  cat > "$F/site/public/index.html" <<'HTML'
<html><head><title>home</title></head><body><main>
<section id="hero"><h1>t</h1><a href="/features/good/">good</a></section>
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
<urlset><url><loc>https://x.test/</loc></url><url><loc>https://x.test/plan/</loc></url><url><loc>https://x.test/features/good/</loc></url><url><loc>https://x.test/features/draft/</loc></url></urlset>
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
for c in substance honesty install indexing; do grep -q "^OK   $c " out.txt || { echo "    a clean tree did not report $c"; cat out.txt; exit 1; }; done

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

# --- indexing, each direction
fresh; printf '<html><head><title>i</title></head><body>i</body></html>\n' > "$F/site/public/plan/i0001/index.html"
expect_finding 10 '/plan/i0001/ is below an unlisted section and carries no noindex' "an unlisted page without noindex"
fresh; sed -i.bak 's#</urlset>#<url><loc>https://x.test/plan/i0001/</loc></url></urlset>#' "$F/site/public/sitemap.xml"
expect_finding 10 '/plan/i0001/ is below an unlisted section and is in sitemap.xml' "an unlisted page in the sitemap"
fresh; sed -i.bak 's#<url><loc>https://x.test/features/good/</loc></url>##' "$F/site/public/sitemap.xml"
expect_finding 10 '/features/good/ is indexable and absent from sitemap.xml' "an indexable page missing from the sitemap"
exit 0
