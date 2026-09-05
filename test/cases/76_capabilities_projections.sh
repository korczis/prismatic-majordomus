# majordomus-covers: none
# One capability, every interface. The Rust executable is built from this tree and started
# inside a repository the shell tool's own `init` wrote; then the same capability is read
# back through the registry's introspection on the command line, through MCP over pipes,
# through the OpenAPI document, and through the generated reference, with nothing but data
# changed between the checks. The HTTP socket and the Swagger shell are exercised by the
# crate's own black-box suites, which this case runs, so that no network client is needed
# here (rule project.no-network-no-eval).
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the site cases do for zola.
. "$ROOT/test/lib.sh"
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj76.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# --- the registry builds over the layer init writes, and says so check by check
expect_exit 0 "$RB" capabilities validate
expect_grep '^OK   registry '
expect_grep '^OK   mcp '
expect_grep '^OK   http '
expect_grep '^OK   openapi '
expect_grep 'validate: 0 failure\(s\)'

# --- one executable capability, described: its id, and every projection it declares
"$RB" capabilities describe objects.get --format json 2>/dev/null > "$S/cap.json"
jq -e '.id == "objects.get" and .kind == "query" and .exposure.mcp.tool == "majordomus_get" and .exposure.http.method == "GET" and .exposure.http.path == "/api/v1/object" and .provenance.source == "builtin"' "$S/cap.json" >/dev/null \
  || { echo "    capabilities describe objects.get is not the descriptor"; cat "$S/cap.json"; exit 1; }
# the same entry under MCP: the tool carries the id, and calling it answers from the layer
{
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case76","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n'
  printf '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"majordomus_get","arguments":{"uri":"majordomus://prompt/continue"}}}\n'
} > "$S/session.in"
rc=0; "$RB" mcp < "$S/session.in" > "$S/session.out" 2>/dev/null || rc=$?
[ "$rc" = 0 ] || { echo "    mcp exited $rc"; exit 1; }
sed -n 2p "$S/session.out" | jq -e '.result.tools[] | select(.name == "majordomus_get") | ._meta.majordomus.id == "objects.get"' >/dev/null \
  || { echo "    the MCP tool does not carry the canonical id"; exit 1; }
sed -n 3p "$S/session.out" | jq -e '.result.structuredContent.id == "prompt.continue" and .result.structuredContent.provenance.path == ".ai/repo/prompts/continue.md"' >/dev/null \
  || { echo "    the MCP call did not answer from the layer"; exit 1; }
# the same entry in the OpenAPI document and in the generated reference
expect_exit 0 "$RB" generate --out "$S/gen"
jq -e '.paths["/api/v1/object"].get.operationId == "objects.get" and .paths["/api/v1/object"].get["x-majordomus-mcp"].tool == "majordomus_get" and .openapi == "3.1.0"' "$S/gen/docs/generated/openapi.json" >/dev/null \
  || { echo "    the OpenAPI operation is not the same capability"; exit 1; }
grep -q '^| `objects.get` |' "$S/gen/docs/generated/capabilities.md" || { echo "    the reference lacks objects.get"; exit 1; }
# every OpenAPI operation is a registry entry and every HTTP exposure is an operation: counted both ways
ops="$(jq '[.paths[] | keys[]] | length' "$S/gen/docs/generated/openapi.json")"
routes="$("$RB" capabilities list --exposure http --format json 2>/dev/null | jq '.count')"
[ "$ops" = "$routes" ] || { echo "    $ops OpenAPI operations against $routes HTTP exposures"; exit 1; }

# --- the committed projections are derived: in sync after generation, refused when tampered
expect_exit 0 "$RB" generate --check --out "$S/gen"
expect_grep 'generate --check: in sync'
printf '\n' >> "$S/gen/docs/generated/openapi.json"
expect_exit 10 "$RB" generate --check --out "$S/gen"
expect_grep 'openapi.json \(differs\)'
# and the shell tool's own allow-lists are derived from the schemas this tree ships
expect_exit 0 "$RB" generate allow --check

# --- data only: a declarative object of a known kind, and a kind the repository defines with its schema
cat > .ai/repo/rules/project/example.v1.md <<'R'
---
id: project.example
version: 1
kind: rule
title: Example
description: An example project rule.
statement: Do the example thing.
status: active
class: advisory
depends_on: []
tags: [example]
---
R
mkdir -p .ai/repo/knowledge/schemas .ai/repo/notes
printf 'schema: majordomus-kinds/v1\nkinds:\n  note:\n    format: markdown\n    front_matter: required\n    schema: note\n    identity: [id]\n    title: title\n' > .ai/repo/knowledge/kinds.yaml
printf '{ "type": "object", "additionalProperties": false, "required": ["id", "title"], "properties": { "id": { "type": "string" }, "title": { "type": "string" } } }\n' > .ai/repo/knowledge/schemas/note.schema.json
printf '\n  - id: note\n    kind: note\n    discovery: vcs\n    pathspec: '"'"':(glob).ai/repo/notes/*.md'"'"'\n    required: false\n' >> .ai/repo/knowledge/sources.yaml
printf -- '---\nid: first\ntitle: The first note\n---\n\nBody.\n' > .ai/repo/notes/first.md
printf -- '---\nid: second\ntitle: T\ncolour: red\n---\n' > .ai/repo/notes/second.md
git add -A >/dev/null
"$RB" capabilities list --kind resource --format json 2>/dev/null > "$S/list.json"
jq -e '[.capabilities[].id] | index("rule.project.example@1") != null and index("note.first") != null and index("note.second") == null' "$S/list.json" >/dev/null \
  || { echo "    the added rule and the repository-defined note are not both listed, or the broken note is"; exit 1; }
"$MJ" rules list | grep -q '^project.example ' || { echo "    the shell tool does not see the same rule"; exit 1; }
expect_exit 10 "$RB" mcp --inspect
expect_grep '^FAIL unknown_key +\.ai/repo/notes/second\.md'
expect_grep '^resource    majordomus://note/first$'
# a repository may add a kind, not redefine one the distribution declares
printf 'schema: majordomus-kinds/v1\nkinds:\n  rule:\n    format: yaml\n' > .ai/repo/knowledge/kinds.yaml
git add -A >/dev/null
expect_exit 10 "$RB" capabilities validate
expect_grep "kind 'rule' is declared by both"

# --- the HTTP socket, the Swagger shell and MCP/HTTP parity, through the crate's own black-box
#     suites; these are cargo's, so a prebuilt executable alone cannot run them
if command -v cargo >/dev/null 2>&1; then
  RUSTFLAGS='' cargo test -q --manifest-path "$MANIFEST" --test http_serve --test projections 2>"$S/cargo.log" >"$S/cargo.out" \
    || { tail -40 "$S/cargo.log" "$S/cargo.out"; echo "    the crate's HTTP and projection suites failed"; exit 1; }
else
  echo "    skip: cargo not installed (the crate's HTTP and projection suites; the executable half ran)"
fi
