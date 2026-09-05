# majordomus-covers: none
# The OpenAPI document is inferred, not written: the tags are the modules, the examples are
# the capabilities' benchmark cases, the responses are the statuses the router answers for
# the capability's kind, the prose is the one text every projection shares, and the site's
# API reference is rendered from the committed document. The Rust executable is built from
# this tree and started inside a repository the shell tool's own `init` wrote; the document
# it generates there is inspected field by field, the committed copies in this tree are
# checked against the registry and against each other, and the routes are replayed with
# their own cases over a real socket by the crate's suite (rule project.no-network-no-eval:
# no network client here).
#
# Skips itself when cargo is absent, as the site cases do for zola.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || { echo "    skip: cargo not installed"; exit 0; }
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj92.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RUSTFLAGS='' cargo build -q --manifest-path "$MANIFEST" 2>"$S/build.log" || { cat "$S/build.log"; echo "    cargo build failed"; exit 1; }
RB="$ROOT/apps/majordomus-cli/target/debug/majordomus"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
expect_exit 0 "$RB" generate openapi --out "$S/gen"
DOC="$S/gen/docs/generated/openapi.json"

# --- the tags are the modules: every tag an operation uses is declared with a description, no other
jq -e '([.paths[][] | .tags[]] | unique) == ([.tags[].name] | sort)' "$DOC" >/dev/null \
  || { echo "    the declared tags are not exactly the tags the operations use"; exit 1; }
jq -e '[.tags[] | select((.description | length) == 0)] | length == 0' "$DOC" >/dev/null \
  || { echo "    a tag has no description"; exit 1; }
"$RB" capabilities list --exposure http --format json 2>/dev/null > "$S/http.json"
for tag in $(jq -r '.tags[].name' "$DOC"); do
  jq -e --arg t "$tag" '[.capabilities[] | select((.id | split(".")[0]) == $t)] | length > 0' "$S/http.json" >/dev/null \
    || { echo "    tag $tag names no module that puts a capability on the wire"; exit 1; }
done

# --- the examples are the benchmark cases: every GET parameter a case sets shows it, every POST body shows its cases
jq -e '[.paths[] | .get? // empty | .parameters[] | select(.schema.type | if type == "array" then index("null") != null else false end)] | length == 0' "$DOC" >/dev/null \
  || { echo "    a query parameter is declared nullable; a query string is never null"; exit 1; }
jq -e '[.paths[] | .get? // empty | .parameters[] | select(has("default") and .default == null)] | length == 0' "$DOC" >/dev/null \
  || { echo "    a query parameter carries default: null"; exit 1; }
jq -e '[.paths[] | .get? // empty | .parameters[] | select(.required == true and (has("examples") | not))] | length == 0' "$DOC" >/dev/null \
  || { echo "    a required query parameter has no example, so no case sets it"; exit 1; }
jq -e '[.paths[] | .post? // empty | select((.requestBody.content["application/json"].examples // {} | length) == 0)] | length == 0' "$DOC" >/dev/null \
  || { echo "    a POST operation has no request body example"; exit 1; }
jq -e '[.paths[][] | select((.parameters // []) | length > 0) | select([.parameters[] | has("examples")] | any | not)] | length == 0' "$DOC" >/dev/null \
  || { echo "    an operation with parameters shows no example on any of them"; exit 1; }
# the example of objects.get names an object the repository holds, and the CLI reads it back
uri="$(jq -r '.paths["/api/v1/object"].get.parameters[] | select(.name == "uri") | .examples[] | .value' "$DOC" | head -1)"
[ -n "$uri" ] || { echo "    objects.get has no uri example"; exit 1; }
"$RB" capabilities list --kind resource --format json 2>/dev/null | jq -e --arg u "$uri" '[.capabilities[] | select(.exposure.mcp.resource.uri == $u)] | length == 1' >/dev/null \
  || { echo "    the uri example $uri is not a resource of this repository"; exit 1; }

# --- the responses are the router's statuses, by kind: a query cannot be refused, a command can
jq -e '[.paths[][] | select(.responses | (has("200") and has("400") and has("404") and has("500") and has("default")) | not)] | length == 0' "$DOC" >/dev/null \
  || { echo "    an operation lacks one of 200, 400, 404, 500, default"; exit 1; }
jq -e '[.paths[][] | select((.["x-majordomus-kind"] == "command") != (.responses | has("422")))] | length == 0' "$DOC" >/dev/null \
  || { echo "    422 is not exactly the commands'"; exit 1; }
jq -e '[.paths[][] | .responses[] | select((.description | length) == 0 or (.content["application/json"].schema | not))] | length == 0' "$DOC" >/dev/null \
  || { echo "    a response has no description or no schema"; exit 1; }
jq -e '[.paths[][] | select(has("x-majordomus-benchmark") and has("x-majordomus-cache") and has("x-majordomus-stability") and has("x-majordomus-provenance") | not)] | length == 0' "$DOC" >/dev/null \
  || { echo "    an operation lacks a policy or provenance extension"; exit 1; }
jq -e '(["400","404","422","500"] - [."x-majordomus".errors[].status]) | length == 0' "$DOC" >/dev/null \
  || { echo "    x-majordomus.errors does not name every error status"; exit 1; }

# --- one source of prose: the document, the MCP instructions and the HTTP index say the same thing
summary="$(jq -r '.info.summary' "$DOC")"
[ -n "$summary" ] || { echo "    info.summary is empty"; exit 1; }
jq -e --arg s "$summary" '.info.description | startswith($s)' "$DOC" >/dev/null \
  || { echo "    info.description does not start with info.summary"; exit 1; }
jq -e '.info.description | contains("**Binding.**") and contains("**Examples are benchmark cases.**") and contains("**Generated, committed, checked.**")' "$DOC" >/dev/null \
  || { echo "    info.description lacks the paragraphs every projection shares"; exit 1; }
jq -e '.info.license.identifier == "MIT" and (.info.contact.url | startswith("https://github.com/")) and (.externalDocs.url | endswith("/docs/api/"))' "$DOC" >/dev/null \
  || { echo "    licence, contact or externalDocs are not from the manifest and the site"; exit 1; }
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case92","version":"0"}}}\n' > "$S/init.in"
"$RB" mcp --standalone < "$S/init.in" 2>/dev/null | head -1 | jq -e --arg s "$summary" '.result.instructions | startswith($s)' >/dev/null \
  || { echo "    the MCP instructions do not open with the same summary"; exit 1; }

# --- committed, checked, and the site's projection is derived from the committed document
( cd "$ROOT" && "$RB" generate openapi --check >/dev/null 2>&1 ) || { echo "    the committed docs/generated/openapi.json is stale"; exit 1; }
[ -f "$ROOT/site/data/generated/openapi.json" ] || { echo "    the site has no copy of the document"; exit 1; }
[ "$(jq -r '.source' "$ROOT/site/data/generated/openapi.json")" = "docs/generated/openapi.json" ] \
  || { echo "    site/data/generated/openapi.json does not name docs/generated/openapi.json as its source"; exit 1; }
cmp -s <(jq -c '[.paths[][] | .operationId] | sort' "$ROOT/docs/generated/openapi.json") \
       <(jq -c '[.tags[].operations[].id] | sort' "$ROOT/site/data/generated/openapi.json") \
  || { echo "    the site's projection does not carry exactly the document's operations"; exit 1; }
cmp -s <(jq -c '[.tags[].name] | sort' "$ROOT/docs/generated/openapi.json") <(jq -c '[.tags[].name] | sort' "$ROOT/site/data/generated/openapi.json") \
  || { echo "    the site's projection does not carry exactly the document's tags"; exit 1; }
base="$(sed -n 's/^base_url = "\(.*\)"$/\1/p' "$ROOT/site/config.toml")"
[ "$(jq -r '.externalDocs.url' "$ROOT/docs/generated/openapi.json")" = "$base/docs/api/" ] \
  || { echo "    externalDocs.url is not the site's /docs/api/ route"; exit 1; }
grep -q 'template = "api.html"' "$ROOT/site/content/docs/api.md" || { echo "    the site has no derived API page"; exit 1; }

# --- every route answers its own cases over a real socket, and the document shows them: the crate's suite
RUSTFLAGS='' cargo test -q --manifest-path "$MANIFEST" --test http_serve every_route_answers_its_benchmark_cases 2>"$S/cargo.log" >"$S/cargo.out" \
  || { tail -40 "$S/cargo.log" "$S/cargo.out"; echo "    the route replay failed"; exit 1; }
grep -q 'test result: ok. 1 passed' "$S/cargo.out" || { cat "$S/cargo.out"; echo "    the replay test did not run"; exit 1; }
