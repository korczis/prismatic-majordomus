# The OpenAPI document of the Rust executable (docs/generated/openapi.json), projected into
# the shape site/templates/api.html renders: operations grouped by tag, every schema flattened
# to a property table, one curl per operation from its first example values. Nothing here is
# authored: every string comes from the document, and the document comes from the registry.
# Input: the document. $base_url: the site's base URL. Output: {schema:1, ...}.

def refname: if type == "object" and has("$ref") then .["$ref"] | split("/") | last else null end;

# A one-line name of a schema, for a table cell. Follows $ref by name, never by content.
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
          description: (.value.description // ""), refs: (.value | refs) } ];

# The alternatives of a oneOf schema, each with its own properties (an enum of tagged objects).
def variants:
  [ (.oneOf // [])[] | { type: typename, description: (.description // ""), properties: properties } ];

def is_enum: has("oneOf") and all(.oneOf[]; has("const"));

def url_encode: @uri;

. as $doc
| ($doc.info) as $info
| ($doc["x-majordomus"]) as $x
| {
    schema: 1,
    source: "docs/generated/openapi.json",
    generator: "scripts/generate-site-data (scripts/lib/openapi-site.jq)",
    openapi: $doc.openapi,
    dialect: $doc.jsonSchemaDialect,
    info: { title: $info.title, version: $info.version, summary: ($info.summary // ""),
            description_md: ($info.description // ""),
            license: ($info.license.name // ""), contact_url: ($info.contact.url // ""), contact_name: ($info.contact.name // "") },
    external_docs: ($doc.externalDocs // {}),
    servers: ($doc.servers // []),
    generated_by: ($x.generator // ""),
    binding: ($x.binding // ""),
    errors: ($x.errors // []),
    infrastructure: [ ($x.infrastructure // [])[]
      | { path: .,
          what: (if . == "/" then "The index: name, version, repository root, and where the document, the reference, the capabilities, the peers and MCP are."
                 elif . == "/openapi.json" then "This document, rendered from the registry at every request; what Swagger UI loads."
                 elif . == "/docs" then "Swagger UI over /openapi.json, served by the running server; the page embeds no specification of its own."
                 elif . == "/mcp" then "MCP over HTTP (Streamable HTTP) for a second client; the shared server only."
                 else "" end) } ],
    tags: [ ($doc.tags // [])[] | . as $t
      | { name: $t.name, description: ($t.description // ""),
          operations: [ $doc.paths | to_entries[] | .key as $path | .value | to_entries[]
            | .key as $method | .value as $op
            | select(($op.tags // []) | index($t.name) != null)
            | ($op.parameters // []) as $params
            | ($op.requestBody.content["application/json"] // {}) as $body
            | {
                id: $op.operationId, method: ($method | ascii_upcase), path: $path,
                summary: ($op.summary // ""), description: ($op.description // ""),
                kind: ($op["x-majordomus-kind"] // ""), stability: ($op["x-majordomus-stability"] // ""),
                benchmark: ($op["x-majordomus-benchmark"].policy // ""),
                benchmark_reason: ($op["x-majordomus-benchmark"].reason // ""),
                cache: ($op["x-majordomus-cache"] | if type == "object" then (.policy // (keys | first)) else tostring end),
                cache_detail: ($op["x-majordomus-cache"] | if type == "object" then (to_entries | map(select(.key != "policy")) | map("\(.key)=\(.value)") | join(", ")) else "" end),
                provenance: ($op["x-majordomus-provenance"] // {}),
                mcp_tool: ($op["x-majordomus-mcp"].tool // ""), mcp_resource: ($op["x-majordomus-mcp"].resource // ""),
                cli: ($op["x-majordomus-cli"] // ""),
                parameters: [ $params[] | { name, required: (.required // false), type: (.schema | typename),
                                            description: (.description // ""),
                                            examples: [ (.examples // {}) | to_entries[] | { case: .key, value: (.value.value | tojson) } ] } ],
                body_schema: ($body.schema | refname // ""),
                body_examples: [ ($body.examples // {}) | to_entries[] | { case: .key, json: (.value.value | tojson) } ],
                responses: [ $op.responses | to_entries[] | { status: .key, description: (.value.description // ""),
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
      | { name: $name, description: ($s.description // ""), type: ($s | typename),
          is_enum: ($s | is_enum),
          values: (if ($s | is_enum) then [ $s.oneOf[] | { value: (.const | tostring), description: (.description // "") } ] else [] end),
          properties: ($s | properties),
          variants: (if ($s | has("oneOf")) and (($s | is_enum) | not) then ($s | variants) else [] end),
          used_by: [ $doc.paths | to_entries[] | .value | to_entries[] | .value | select(([.. | objects | select(has("$ref")) | refname] | index($name)) != null) | .operationId ],
          used_by_schemas: [ $doc.components.schemas | to_entries[] | select(.key != $name) | select((.value | refs | index($name)) != null) | .key ] } ],
    counts: { operations: ([ $doc.paths | to_entries[] | .value | to_entries[] ] | length),
              tags: (($doc.tags // []) | length), schemas: ($doc.components.schemas | length) }
  }
