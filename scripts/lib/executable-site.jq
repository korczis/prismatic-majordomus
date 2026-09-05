# executable-site.jq — the site's routes and cross-links for the Rust executable, from the
# registry manifest the executable generates (docs/generated/registry.json) and the claims
# matrix (site/data/generated/capabilities.json, itself from docs/CLAIMS.yaml).
#
# Nothing here restates a fact of the registry: ids, modules and source paths are read from
# the manifest, and the templates read every other fact (descriptions, schemas, exposures,
# policies) from site/data/registry/registry.json, which the executable writes. What this
# program adds is the site's own layer — a route per module and per capability, the anchor
# of an operation on the API reference, the link back to the source on GitHub, and the
# claims that speak about each surface — computed from ids and paths, never from prose.
#
# input:  docs/generated/registry.json
# args:   $claims (the claims array), $repo_url (the repository on GitHub), $branch
#
# A claim is attached by the path of its implementation: a claim implemented in the file a
# module's descriptors are composed in belongs to that module and each of its capabilities;
# one implemented under the executable's MCP, HTTP, benchmark or command-line code belongs
# to that surface; any other claim implemented in the crate belongs to the executable as a
# whole. The prefixes are the crate's directories, listed once below.

def slug: gsub("\\."; "-");
def crate: "apps/majordomus-cli/";
def blob($p): $repo_url + "/blob/" + $branch + "/" + $p;

# the surface a repository path speaks about, or null when it is not the crate's
def surface_of($p):
  if ($p | startswith(crate) | not) then null
  else ($p | ltrimstr(crate)) as $rel
  | if   ($rel | test("^src/mcp/|^src/lease\\.rs$|^src/peers\\.rs$|^src/shared\\.rs$")) then "mcp"
    elif ($rel | test("^src/http/")) then "http"
    elif ($rel | test("^src/bench/|^benches/")) then "benchmarks"
    elif ($rel | test("^src/cli\\.rs$|^src/commands/")) then "cli"
    elif ($rel | test("^src/capability/builtin/[a-z_]+\\.rs$")) then ("module:" + ($rel | capture("builtin/(?<m>[a-z_]+)\\.rs").m))
    else "executable" end
  end;

def claim_ref: { id: .id, status: .status, route: ("/guarantees/" + .id + "/"), test: (.test // ""), implementation: (.implementation // "") };

. as $reg
| ($reg.modules | map(select(.source == "builtin"))) as $modules
| ($claims | map(select((.implementation // "") | startswith(crate)) | . + { surface: surface_of(.implementation) })) as $crate_claims
| {
    schema: 1,
    source: "docs/generated/registry.json",
    generator: $reg.generator,
    registry_schema: $reg.schema,
    routes: {
      overview: "/registry/",
      executable: "/registry/executable/",
      modules: "/registry/modules/",
      capabilities: "/registry/capabilities/",
      cli: "/registry/cli/",
      mcp: "/registry/mcp/",
      http: "/docs/api/",
      benchmarks: "/registry/benchmarks/"
    },
    # the claims about each whole surface, for the surface's page
    claims: {
      executable: [ $crate_claims[] | select(.surface == "executable") | claim_ref ],
      cli:        [ $crate_claims[] | select(.surface == "cli") | claim_ref ],
      mcp:        [ $crate_claims[] | select(.surface == "mcp") | claim_ref ],
      http:       [ $crate_claims[] | select(.surface == "http") | claim_ref ],
      benchmarks: [ $crate_claims[] | select(.surface == "benchmarks") | claim_ref ]
    },
    modules: [ $modules[] | .id as $m
      | { id: .id,
          slug: (.id | slug),
          route: ("/registry/modules/" + (.id | slug) + "/"),
          title: .title,
          source_path: ([ $reg.capabilities[] | select(.module == $m) | .source_path ] | first),
          capabilities: [ $reg.capabilities[] | select(.module == $m) | { id: .id, slug: (.id | slug), route: ("/registry/capabilities/" + (.id | slug) + "/") } ],
          claims: [ $crate_claims[] | select(.surface == ("module:" + $m)) | claim_ref ] }
      | . + { source_url: blob(.source_path) } ],
    capabilities: [ $reg.capabilities[] | .module as $m
      | { id: .id,
          slug: (.id | slug),
          route: ("/registry/capabilities/" + (.id | slug) + "/"),
          module: .module,
          module_route: ("/registry/modules/" + (.module | slug) + "/"),
          title: .title,
          kind: .kind,
          source_path: .source_path,
          source_url: blob(.source_path),
          api_anchor: (if .exposure.http then ("/docs/api/#op-" + (.id | slug)) else null end),
          tool: (.exposure.mcp.tool // null),
          resource: (.exposure.mcp.resource.uri // null),
          cli: (if .exposure.cli then ("majordomus " + (.exposure.cli.path | join(" "))) else null end),
          claims: [ $crate_claims[] | select(.surface == ("module:" + $m)) | claim_ref ],
          tests: ([ $crate_claims[] | select(.surface == ("module:" + $m)) | .test // empty ] | unique) } ],
    counts: { modules: ($modules | length), capabilities: ($reg.capabilities | length), claims: ($crate_claims | length) }
  }
