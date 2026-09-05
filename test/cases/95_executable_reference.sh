# majordomus-covers: none
# The Rust executable's reference on the site is a projection of the registry manifest the
# executable generates (docs/generated/registry.json): a capability that joins the manifest
# gets its route, its index entries and its links from the site generator alone; one that
# leaves it loses them, page and all; a manifest that names a module the registry does not
# hold, a source file the tree does not have, or another schema is refused; and the site's
# input hash moves with the manifest, so a regenerated manifest without a regenerated site
# is stale to `generate-site-data --check`. The Rust side of the chain — a descriptor
# changed in code moves the manifest and the dataset — is apps/majordomus-cli/tests/
# projections.rs; the rendered side — every entry a page, no page without an entry — is
# scripts/site-check, run by test/cases/12_site_build.sh. No cargo needed here: the
# manifest is data, and this case edits the data.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
fixture_repo "$T" AGENTS.md docs site/data/marketing.toml site/content-src test/cases
git -C "$T" add -A >/dev/null; git -C "$T" commit -qm fixture
REG="$T/docs/generated/registry.json"; G="$T/site/data/generated"; C="$T/site/content/registry"
expect_exit 0 "$T/scripts/generate-site-data"
cp -R "$G" "$T/before"
n="$(jq '.capabilities | length' "$REG")"
[ "$(jq '.counts.capabilities' "$G/executable.json")" = "$n" ] || { echo "    executable.json counts $(jq '.counts.capabilities' "$G/executable.json") capabilities, the manifest $n"; exit 1; }
# every capability of the manifest is a route with its stub, and links its module
for id in $(jq -r '.capabilities[].id' "$REG"); do
  slug="$(printf '%s' "$id" | tr '._' '--')"   # the route Zola serves: it slugifies `_` to `-` as well
  [ -f "$C/capabilities/$slug.md" ] || { echo "    no stub for $id"; exit 1; }
  grep -q "^id = \"$id\"" "$C/capabilities/$slug.md" || { echo "    stub of $id does not carry its id"; exit 1; }
  m="$(jq -r --arg id "$id" '.capabilities[] | select(.id==$id) | .module' "$REG")"
  [ "$(jq -r --arg id "$id" '.capabilities[] | select(.id==$id) | .module_route' "$G/executable.json")" = "/registry/modules/$m/" ] || { echo "    $id does not link module $m"; exit 1; }
done
for m in $(jq -r '.modules[] | select(.source=="builtin") | .id' "$REG"); do [ -f "$C/modules/$m.md" ] || { echo "    no stub for module $m"; exit 1; }; done
for pg in _index executable cli mcp benchmarks; do [ -f "$C/$pg.md" ] || { echo "    no $pg page"; exit 1; }; done
# a claim implemented in a module's file is attached to that module and its capabilities
[ "$(jq -r '.modules[] | select(.id=="objects") | .claims | length' "$G/executable.json")" -gt 0 ] || { echo "    no claim attached to the objects module (mcp-uri-resolution is implemented there)"; exit 1; }
[ "$(jq -r '.capabilities[] | select(.id=="objects.get") | .claims | map(.id) | index("mcp-uri-resolution")' "$G/executable.json")" != null ] || { echo "    objects.get does not carry the claim implemented beside it"; exit 1; }

# --- 1. a capability joins the manifest: its route, its links and its index entry appear,
#        and nothing was edited to tell the site about it
jq '.capabilities += [ (.capabilities[] | select(.id=="objects.search")) | .id = "objects.grep" | .title = "Grep objects" | .description = "ADDED CAPABILITY" | .exposure.http.path = "/api/v1/grep" | .exposure.mcp.tool = "majordomus_grep" ] | .modules |= map(if .id == "objects" then .capabilities += 1 else . end)' "$REG" > "$REG.new" && mv "$REG.new" "$REG"
expect_exit 0 "$T/scripts/generate-site-data"
[ -f "$C/capabilities/objects-grep.md" ] || { echo "    the added capability got no stub"; exit 1; }
expect_grep '^description = "ADDED CAPABILITY"' "$C/capabilities/objects-grep.md"
[ "$(jq -r '.capabilities[] | select(.id=="objects.grep") | .api_anchor' "$G/executable.json")" = "/docs/api/#op-objects-grep" ] || { echo "    no API anchor for the added capability"; exit 1; }
[ "$(jq -r '.capabilities[] | select(.id=="objects.grep") | .tool' "$G/executable.json")" = "majordomus_grep" ] || { echo "    no tool for the added capability"; exit 1; }
jq -e '.modules[] | select(.id=="objects") | .capabilities | map(.id) | index("objects.grep")' "$G/executable.json" >/dev/null || { echo "    the module index does not list the added capability"; exit 1; }
[ "$(jq -r .source_hash "$G/source.json")" != "$(jq -r .source_hash "$T/before/source.json")" ] || { echo "    the input hash did not move with the manifest"; exit 1; }
# the previous data is stale to --check: a manifest regenerated without the site is caught
rm -rf "$G"; cp -R "$T/before" "$G"
expect_exit 10 "$T/scripts/generate-site-data" --check
expect_grep 'STALE'

# --- 2. a capability leaves the manifest: its page goes with it, no orphan stays
jq 'del(.capabilities[] | select(.id=="objects.grep" or .id=="objects.search")) | .modules |= map(if .id == "objects" then .capabilities -= 2 else . end)' "$REG" > "$REG.new" && mv "$REG.new" "$REG"
expect_exit 0 "$T/scripts/generate-site-data"
[ ! -e "$C/capabilities/objects-grep.md" ] && [ ! -e "$C/capabilities/objects-search.md" ] || { echo "    a removed capability kept its stub"; exit 1; }
jq -e '[.capabilities[].id] | index("objects.search") | not' "$G/executable.json" >/dev/null || { echo "    executable.json still lists the removed capability"; exit 1; }
[ "$(jq '.counts.capabilities' "$G/executable.json")" = "$((n - 1))" ]

# --- 3. a broken reference is refused, loudly, and nothing is published
cp -R "$G" "$T/good"
jq '.capabilities[0].module = "nowhere"' "$REG" > "$REG.bad"
cp "$REG" "$REG.keep"; mv "$REG.bad" "$REG"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'module nowhere is not a builtin module'
diff -r "$T/good" "$G" >/dev/null || { echo "    a refused generation changed the published data"; exit 1; }
jq '.capabilities[0].source_path = "apps/majordomus-cli/src/capability/builtin/vanished.rs"' "$REG.keep" > "$REG"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'no such file exists'
jq '.schema = "majordomus/capability-registry/v0"' "$REG.keep" > "$REG"
expect_exit 10 "$T/scripts/generate-site-data"
expect_grep 'not a majordomus/capability-registry/v1 manifest'
