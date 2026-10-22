# The OpenAPI document of the Rust executable (docs/generated/openapi.json), projected into
# the shape site/templates/api.html renders: operations grouped by tag, every schema flattened
# to a property table, one curl per operation from its first example values. Nothing here is
# authored: every string comes from the document, and the document comes from the registry.
# Input: the document. $base_url: the site's base URL. Output: {schema:1, ...}.

def refname: if type == "object" and has("$ref") then .["$ref"] | split("/") | last else null end;

# A one-line name of a schema, for a table cell. Follows $ref by name, never by content.
# A description is data rendered inside a page that already has its own <h1>. Rust doc
# comments carry `# Example` sections, schemars puts them in the schema verbatim, and the
# page then renders twenty-four competing <h1>s — site-check refuses the build for it, and a
# screen reader is told the page has twenty-five titles. Headings in a description are
# demoted three levels here, in the projection, rather than in the document: the OpenAPI
# document says what the API is, and how deep a heading sits is a fact about this page.
def demoted: if type == "string" then gsub("(?m)^(?<h>#{1,3}) "; "###\(.h) ") else . end;

def typename:
  if type != "object" then "any"
  elif has("$ref") then (refname)
  elif has("const") then ("\"" + (.const | tostring) + "\"")
  elif has("enum") then (.enum | map("\"" + tostring + "\"") | join(" | "))
  elif has("oneOf") then (if all(.oneOf[]; has("const")) then (.oneOf | map(typename) | join(" | ")) else "one of " + (.oneOf | length | tostring) + " variants" end)
  elif has("anyOf") then (.anyOf | map(typename) | join(" | "))
  elif has("allOf") then (.allOf | map(typename) | join(" & "))
  elif (.type | type) == "array" then (.type | join(" | "))
  elif .type == "array" then ("array of " + ((.items // {}) | typename))
  elif .type == "object" and has("additionalProperties") and (.additionalProperties | type) == "object" then ("map of " + (.additionalProperties | typename))
  elif has("type") then (.type + (if has("format") then " (" + .format + ")" else "" end))
  else "any" end;

# Every schema name a schema refers to, for the links a row carries.
def refs: [.. | objects | select(has("$ref")) | refname] | unique;

def properties:
  (.required // []) as $req
  | [ (.properties // {}) | to_entries[] | .key as $k
      | { name: $k, type: (.value | typename), required: (($req | index($k)) != null),
          description: ((.value.description // "") | demoted), refs: (.value | refs) } ];

# The alternatives of a oneOf schema, each with its own properties (an enum of tagged objects).
def variants:
  [ (.oneOf // [])[] | { type: typename, description: ((.description // "") | demoted), properties: properties } ];

def is_enum: has("oneOf") and all(.oneOf[]; has("const"));

def url_encode: @uri;

# The route segment of a tag. Zola slugifies a page's path, so an underscore is served as a
# dash; the projection says the route Zola will actually build rather than hoping they agree.
def slug: gsub("_"; "-");
# The HTML id of an operation, as the templates form it.
def op_anchor: "op-" + gsub("\\."; "-");

. as $doc
| ($doc.info) as $info
| ($doc["x-majordomus"]) as $x
# Every operation and every schema has exactly one page. The reference was one route of 38,000
# elements, which the browser audit could not read inside its deadline; it is now an index, a
# page per tag, and one page of shared types. An operation lives on its tag's page, or on the
# shared page when it carries more than one tag. A schema lives on the page of the one tag whose
# operations reach it — directly, or through the schemas that refer to it — and on the shared
# page when two tags reach it or none does. Nothing else decides where a thing is published.
| ([ $doc.paths | to_entries[] | .value | to_entries[] | .value ]) as $ops
| ($ops | map({ key: .operationId, value: (if ((.tags // []) | length) == 1 then .tags[0] else "shared" end) }) | from_entries) as $op_home
| ($doc.components.schemas | to_entries | map({ key: .key, value: (.value | [.. | objects | select(has("$ref")) | refname] | unique) }) | from_entries) as $refs_of
| ($doc.components.schemas | keys | map(. as $n | { key: $n, value: [ $refs_of | to_entries[] | select(.value | index($n)) | .key ] }) | from_entries) as $referrers
| ($doc.components.schemas | keys | map(. as $n | { key: $n, value: [ $ops[] | select(([.. | objects | select(has("$ref")) | refname] | index($n)) != null) | $op_home[.operationId] ] | unique }) | from_entries) as $direct_tags
| def reach($n; $seen):
    if ($seen | index($n)) then [] else
      ($direct_tags[$n] // []) + ([ ($referrers[$n] // [])[] | reach(.; $seen + [$n]) ] | add // [])
    end;
  ($doc.components.schemas | keys | map(. as $n | (reach($n; []) | unique) as $t
    | { key: $n, value: (if ($t | length) == 1 and $t[0] != "shared" then $t[0] else "shared" end) }) | from_entries) as $schema_home
| {
    schema: 1,
    source: "docs/generated/openapi.json",
    generator: "scripts/generate-site-data (scripts/lib/openapi-site.jq)",
    openapi: $doc.openapi,
    dialect: $doc.jsonSchemaDialect,
    info: { title: $info.title, version: $info.version, summary: ($info.summary // ""),
            description_md: (($info.description // "") | demoted),
            license: ($info.license.name // ""), contact_url: ($info.contact.url // ""), contact_name: ($info.contact.name // "") },
    external_docs: ($doc.externalDocs // {}),
    servers: ($doc.servers // []),
    generated_by: ($x.generator // ""),
    binding: ($x.binding // ""),
    errors: ($x.errors // []),
    # The projection's own routes, as the document resolved them from the surfaces that
    # declare them: the path, what it is in its producer's words, and whether a
    # publication can carry it. Describing them again here is how `/cockpit` came out
    # with an empty description while the Cockpit had one of its own.
    infrastructure: [ ($x.infrastructure // [])[]
      | { id: .id, path: .path, what: .what, availability: .availability } ],
    tags: [ ($doc.tags // [])[] | . as $t
      | { name: $t.name, slug: ($t.name | slug), description: (($t.description // "") | demoted),
          operations: [ $doc.paths | to_entries[] | .key as $path | .value | to_entries[]
            | .key as $method | .value as $op
            | select(($op.tags // []) | index($t.name) != null)
            | ($op.parameters // []) as $params
            | ($op.requestBody.content["application/json"] // {}) as $body
            | {
                id: $op.operationId, home: ($op_home[$op.operationId] | slug), primary: ((($op.tags // [])[0]) == $t.name), method: ($method | ascii_upcase), path: $path,
                summary: ($op.summary // ""), description: (($op.description // "") | demoted),
                kind: ($op["x-majordomus-kind"] // ""), stability: ($op["x-majordomus-stability"] // ""),
                benchmark: ($op["x-majordomus-benchmark"].policy // ""),
                benchmark_reason: ($op["x-majordomus-benchmark"].reason // ""),
                cache: ($op["x-majordomus-cache"] | if type == "object" then (.policy // (keys | first)) else tostring end),
                cache_detail: ($op["x-majordomus-cache"] | if type == "object" then (to_entries | map(select(.key != "policy")) | map("\(.key)=\(.value)") | join(", ")) else "" end),
                provenance: ($op["x-majordomus-provenance"] // {}),
                mcp_tool: ($op["x-majordomus-mcp"].tool // ""), mcp_resource: ($op["x-majordomus-mcp"].resource // ""),
                cli: ($op["x-majordomus-cli"] // ""),
                parameters: [ $params[] | { name, required: (.required // false), type: (.schema | typename),
                                            description: ((.description // "") | demoted),
                                            examples: [ (.examples // {}) | to_entries[] | { case: .key, value: (.value.value | tojson) } ] } ],
                body_schema: ($body.schema | refname // ""),
                body_examples: [ ($body.examples // {}) | to_entries[] | { case: .key, json: (.value.value | tojson) } ],
                responses: [ $op.responses | to_entries[] | { status: .key, description: ((.value.description // "") | demoted),
                                                             schema: (.value.content["application/json"].schema | refname // "") } ],
                result_schema: ($op.responses["200"].content["application/json"].schema | refname // ""),
                curl: (
                  if ($method | ascii_upcase) == "GET" then
                    ([ $params[] | select((.examples // {}) | length > 0) | . as $p
                       | ($p.examples | to_entries | first | .value.value) as $v
                       | ($p.name | url_encode) + "=" + (($v | tostring) | url_encode) ] | join("&")) as $qs
                    | "curl -s \"http://127.0.0.1:8741" + $path + (if $qs == "" then "" else "?" + $qs end) + "\""
                  else
                    (($body.examples // {}) | to_entries | first | .value.value // {} | tojson) as $json
                    | "curl -s -X POST \"http://127.0.0.1:8741" + $path + "\" -H \"content-type: application/json\" -d '" + $json + "'"
                  end)
              } ] } ],
    schemas: [ $doc.components.schemas | to_entries[] | .key as $name | .value as $s
      | { name: $name, home: ($schema_home[$name] | slug), description: (($s.description // "") | demoted), type: ($s | typename),
          is_enum: ($s | is_enum),
          values: (if ($s | is_enum) then [ $s.oneOf[] | { value: (.const | tostring), description: ((.description // "") | demoted) } ] else [] end),
          properties: ($s | properties),
          variants: (if ($s | has("oneOf")) and (($s | is_enum) | not) then ($s | variants) else [] end),
          used_by: [ $doc.paths | to_entries[] | .value | to_entries[] | .value | select(([.. | objects | select(has("$ref")) | refname] | index($name)) != null) | .operationId ],
          used_by_schemas: [ $doc.components.schemas | to_entries[] | select(.key != $name) | select((.value | refs | index($name)) != null) | .key ] } ],
    # where each thing lives, for a link from any page, and the map an old single-page anchor
    # (#op-…, #tag-…, #schema-…) is sent on with: every key is an id the reference used to carry
    schema_homes: ($schema_home | map_values(slug)),
    op_homes: ($op_home | map_values(slug)),
    shared_operations: [ $ops[] | select($op_home[.operationId] == "shared") | .operationId ],
    anchors: ( ($op_home | to_entries | map({ key: (.key | op_anchor), value: ("docs/api/" + (.value | slug) + "/") }))
             + (($doc.tags // []) | map({ key: ("tag-" + .name), value: ("docs/api/" + (.name | slug) + "/") }))
             + ($schema_home | to_entries | map({ key: ("schema-" + .key), value: ("docs/api/" + (.value | slug) + "/") }))
             | from_entries ),
    counts: { operations: ([ $doc.paths | to_entries[] | .value | to_entries[] ] | length),
              tags: (($doc.tags // []) | length), schemas: ($doc.components.schemas | length) }
  }
