# majordomus-covers: none
# One declaration, every surface: the homepage shows a capability of the registry on every surface
# the registry gives it, and nothing it shows is typed — scripts/ci/homepage-check's `surfaces`
# check, exercised against a tree this case builds, so it runs in seconds.
#
# What is proved: a conforming tree reports OK; each of these reports FAIL, with a finding that
# names it — a capability the registry's order does not put first, a surface that drifted from the
# registry, a picker that offers a capability the registry does not have, a picker missing one it
# has, a published slice that lags the registry, no slice published, no block rendered, and a tree
# without the registry. Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/site/public/graphs"
  printf 'order = ["hero"]\n\n[budget]\nassets_bytes = 100\ninline_json_bytes = 20\n' > "$F/site/data/homepage.toml"
  printf '[indexing]\nunlisted = []\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[],"providers":[]}' > "$F/site/data/registry/product.json"
  printf '%s\n' '{"install_command":"i","next_command":"n","verify_command":"v"}' > "$F/site/data/registry/distribution.json"
  # the registry, in its own order: one capability reaching two surfaces, then two reaching three
  cat > "$F/site/data/registry/registry.json" <<'JSON'
{"registry":{"builtin":[
 {"id":"alpha.list","exposure":{"http":{"method":"GET","path":"/api/v1/alpha"},"mcp":{"tool":"majordomus_alpha"}}},
 {"id":"beta.show","exposure":{"cli":{"path":["beta","show"]},"http":{"method":"GET","path":"/api/v1/beta"},"mcp":{"tool":"majordomus_beta"}}},
 {"id":"gamma.run","exposure":{"cli":{"path":["gamma"]},"http":{"method":"POST","path":"/api/v1/gamma"},"mcp":{"tool":"majordomus_gamma"}}}
]}}
JSON
  cat > "$F/site/public/graphs/surfaces.json" <<'JSON'
[{"id":"alpha.list","cli":null,"http":{"method":"GET","path":"/api/v1/alpha"},"mcp":"majordomus_alpha"},
 {"id":"beta.show","cli":"majordomus beta show","http":{"method":"GET","path":"/api/v1/beta"},"mcp":"majordomus_beta"},
 {"id":"gamma.run","cli":"majordomus gamma","http":{"method":"POST","path":"/api/v1/gamma"},"mcp":"majordomus_gamma"}]
JSON
  cat > "$F/site/public/index.html" <<'HTML'
<html><head><title>home</title></head><body><main>
<section id="hero"><a href="/x/">x</a>
<div class="c" data-surfaces x-data="{}">
<label>capability <select data-surface-picker><option value="alpha.list">alpha.list</option><option value="beta.show" selected>beta.show</option><option value="gamma.run">gamma.run</option></select></label>
<dl>
<div><dt>declared as</dt><dd><a data-surface-id="beta.show" x-bind:data-surface-id="cap ? cap.id : 'beta.show'" href="/r/">beta.show</a></dd></div>
<div><dt>command line</dt><dd data-surface="cli">majordomus beta show</dd></div>
<div><dt>HTTP</dt><dd><a data-surface="http" href="/docs/api/">GET /api/v1/beta</a></dd></div>
<div><dt>MCP tool</dt><dd data-surface="mcp">majordomus_beta</dd></div>
</dl></div></section>
</main></body></html>
HTML
  printf '<?xml version="1.0"?><urlset><url><loc>https://x.test/</loc></url></urlset>\n' > "$F/site/public/sitemap.xml"
}
run() { MJ_ROOT="$F" "$CHECK" > out.txt 2>&1 || true; }
expect_ok() {
  run; grep -qE '^OK   surfaces ' out.txt || { echo "    $1: surfaces did not report OK"; cat out.txt; exit 1; }
}
expect_fail() { # <pattern> <what>
  run; grep -qE "^FAIL surfaces .*$1" out.txt || { echo "    $2: no surfaces finding matching '$1'"; cat out.txt; exit 1; }
}

# --- a conforming tree passes
fresh; expect_ok "a conforming tree"

# --- the capability shown is the registry's first reaching every surface, not one typed in
fresh; sed -i.bak 's/data-surface-id="beta.show"/data-surface-id="gamma.run"/' "$F/site/public/index.html"
expect_fail "shows 'gamma.run'; the registry's first capability reaching every surface is 'beta.show'" "another capability shown"

# --- every surface shown is the registry's exposure
fresh; sed -i.bak 's#GET /api/v1/beta</a>#GET /api/v1/other</a>#' "$F/site/public/index.html"
expect_fail "beta.show's http reads 'GET /api/v1/other', the registry says 'GET /api/v1/beta'" "a drifted HTTP surface"
fresh; sed -i.bak 's#data-surface="mcp">majordomus_beta#data-surface="mcp">majordomus_typed#' "$F/site/public/index.html"
expect_fail "beta.show's mcp reads 'majordomus_typed'" "a drifted MCP tool"

# --- the picker offers exactly the registry's capabilities
fresh; sed -i.bak 's#<option value="gamma.run">gamma.run</option>#<option value="gamma.run">gamma.run</option><option value="typed.in">typed.in</option>#' "$F/site/public/index.html"
expect_fail "not in the registry typed.in" "a capability typed into the picker"
fresh; sed -i.bak 's#<option value="alpha.list">alpha.list</option>##' "$F/site/public/index.html"
expect_fail "missing alpha.list" "a capability the picker leaves out"
fresh; sed -i.bak 's#<select data-surface-picker>#<select>#' "$F/site/public/index.html"
expect_fail "offers no data-surface-picker" "no picker"

# --- the published slice matches the registry
fresh; sed -i.bak 's#"mcp":"majordomus_gamma"#"mcp":"majordomus_stale"#' "$F/site/public/graphs/surfaces.json"
expect_fail "graphs/surfaces.json disagrees with the registry on gamma.run mcp" "a slice that lags the registry"
fresh; printf '%s\n' '[{"id":"alpha.list"}]' > "$F/site/public/graphs/surfaces.json"
expect_fail "graphs/surfaces.json carries 1 capabilities, the registry 3" "a short slice"
fresh; rm "$F/site/public/graphs/surfaces.json"
expect_fail "graphs/surfaces.json was not published" "no slice published"

# --- no block, no registry
fresh; sed -i.bak 's# data-surfaces # #' "$F/site/public/index.html"
expect_fail "carries no data-surfaces block" "no block rendered"
fresh; rm "$F/site/data/registry/registry.json"
expect_fail "registry.json missing" "no registry"

echo "    surfaces: the homepage shows the registry's capability on every surface, both ways"
