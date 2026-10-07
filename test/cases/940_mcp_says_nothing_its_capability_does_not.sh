# majordomus-covers: none
# majordomus-timeout: 900
# MCP says nothing its capability does not (ADR 0110), asked of the real executable in a
# repository `init` wrote — a downstream project, not this one:
#
#   1. a tool's annotations are its capability's effect: a read is read-only and
#      idempotent, a tool that writes the repository is destructive and not idempotent
#   2. a refusal carries the error's own word on both transports, and no structuredContent
#   3. `mcp.projection` describes the projection once, and the tool, the HTTP route, the
#      Cockpit page and `mcp --inspect` are that one answer
#   4. a client configuration's standing is read from the file: absent, foreign, wired
#   5. none of it writes the repository
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj940.XXXXXX")"; trap 'serve_down; rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
before="$(git status --porcelain; git ls-files -s | mj_sha256sum)"

# --- 4. the standing of a client configuration, as --inspect prints the description
expect_exit 0 "$RB" mcp --inspect
expect_grep '^server      majordomus [0-9]'
expect_grep '^protocol    2025-06-18'
expect_grep '^effect      read  *[0-9][0-9]* tool.s.$'
expect_grep '^writes      majordomus_plan_transition$'
# init writes no client configuration, so no declared client starts a server here
expect_grep '^client      \.mcp\.json  *absent$'
expect_grep '^WARN mcp_no_client_configured'

printf '{"mcpServers":{"other":{"command":"something-else"}}}\n' > .mcp.json
expect_exit 0 "$RB" mcp --inspect
expect_grep '^client      \.mcp\.json  *foreign$'
expect_grep '^WARN mcp_client_config_foreign'

printf '{"mcpServers":{"majordomus":{"type":"stdio","command":"majordomus-mcp","args":[]}}}\n' > .mcp.json
expect_exit 0 "$RB" mcp --inspect
expect_grep '^client      \.mcp\.json  *wired$'
expect_no_grep '^WARN mcp_'
rm -f .mcp.json

# --- 1-3 over the real pipes
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case940","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/list
  req 3 tools/call '{"name":"majordomus_capability","arguments":{"id":"no.such.capability"}}'
  req 4 tools/call '{"name":"majordomus_mcp","arguments":{}}'
  req 5 tools/call '{"name":"majordomus_mcp","arguments":{"effect":"repository_mutation"}}'
  req 6 prompts/list
  req 7 tools/call '{"name":"majordomus_mcp","arguments":{"effect":"everything"}}'
} > "$S/in"
rc=0; "$RB" mcp --standalone < "$S/in" > "$S/out" 2> "$S/err" || rc=$?
[ "$rc" = 0 ] || { echo "    the server exited $rc at EOF"; cat "$S/err"; exit 1; }
frame() { jq -c --argjson i "$1" 'select(.id == $i)' "$S/out"; }
[ "$(jq -s 'length' "$S/out")" = 7 ] || { echo "    expected 7 frames on stdout:"; cat "$S/out" | cut -c1-200; exit 1; }

# 1. annotations: every tool's four hints follow from the effect it carries, and nothing else
frame 2 | jq -e '
  [.result.tools[] | {e: ._meta.majordomus.effect, a: .annotations}] as $t
  | ($t | length) > 0
  and ($t | all(
        if .e == "read" then (.a.readOnlyHint and .a.idempotentHint and (.a.destructiveHint | not))
        elif .e == "process_state" then ((.a.readOnlyHint | not) and (.a.idempotentHint | not) and (.a.destructiveHint | not))
        elif .e == "repository_mutation" then ((.a.readOnlyHint | not) and (.a.idempotentHint | not) and .a.destructiveHint)
        else false end))
  and ($t | all(.a.openWorldHint == false))' >/dev/null \
  || { echo "    a tool's annotations do not follow from its effect:"; frame 2 | jq -c '.result.tools[] | select(._meta.majordomus.effect != "read") | {name, e: ._meta.majordomus.effect, annotations}'; exit 1; }
frame 2 | jq -e '.result.tools[] | select(.name == "majordomus_plan_transition") | ._meta.majordomus.effect == "repository_mutation" and .annotations.destructiveHint == true and .annotations.idempotentHint == false' >/dev/null \
  || { echo "    the transition tool is announced as safe"; exit 1; }
frame 2 | jq -e '.result.tools[] | select(.name == "majordomus_repository") | .annotations.readOnlyHint == true' >/dev/null \
  || { echo "    a read is not announced as read-only"; exit 1; }

# 2. a refusal: a result, the error's word beside the text, no structuredContent
frame 3 | jq -e '.result.isError == true and .result._meta.majordomus.error.code == "not_found" and (.result | has("structuredContent") | not) and (.result.content[0].text | startswith("not found:"))' >/dev/null \
  || { echo "    the refusal does not carry its category:"; frame 3; exit 1; }
frame 7 | jq -e '.result.isError == true and .result._meta.majordomus.error.code == "invalid_input"' >/dev/null \
  || { echo "    an effect that does not exist is not invalid input:"; frame 7; exit 1; }
# prompts are not served, and the description says so rather than leaving it to be discovered
frame 6 | jq -e '.error.code == -32601' >/dev/null || { echo "    prompts/list was answered:"; frame 6; exit 1; }

# 3. the description agrees with the listing it describes
tools="$(frame 2 | jq '.result.tools | length')"
frame 4 | jq -e --argjson n "$tools" '
  .result.structuredContent as $p
  | $p.tool_count == $n and ($p.tools | length) == $n
  and $p.serving.prompts == false and $p.serving.tools == true
  and ($p.serving.methods | index("tools/call") != null) and ($p.serving.methods | index("prompts/list") == null)
  and $p.server.name == "majordomus" and ($p.protocol_versions | index("2025-06-18") != null)
  and ([$p.effects[].tools] | add) == $n
  and ($p.transports | map(.id) | sort) == ["http", "stdio"]
  and ($p.clients | map(select(.config == ".mcp.json"))[0].standing) == "absent"' >/dev/null \
  || { echo "    mcp.projection disagrees with tools/list ($tools tools):"; frame 4 | jq -c '.result.structuredContent | del(.tools)'; exit 1; }
frame 2 | jq -c '[.result.tools[] | {name, effect: ._meta.majordomus.effect}] | sort_by(.name)' > "$S/listed"
frame 4 | jq -c '[.result.structuredContent.tools[] | {name, effect}] | sort_by(.name)' > "$S/described"
cmp -s "$S/listed" "$S/described" || { echo "    the described tools are not the listed tools"; exit 1; }
frame 5 | jq -e '.result.structuredContent as $p | ($p.tools | length) > 0 and ($p.tools | all(.effect == "repository_mutation")) and ([$p.tools[].name] | sort) == ($p.writers | sort) and $p.tool_count > ($p.tools | length)' >/dev/null \
  || { echo "    the effect filter did not narrow to the writers:"; frame 5 | jq -c '.result.structuredContent | {tool_count, writers, tools: [.tools[].name]}'; exit 1; }

# the same answer over HTTP, and on the Cockpit's page
serve_up "$S/serve.out" "$S/serve.err" || exit 1
curl -fsS "$U/api/v1/mcp" > "$S/http.json" || { echo "    GET /api/v1/mcp failed"; exit 1; }
jq -c '[.tools[] | {name, effect}] | sort_by(.name)' "$S/http.json" > "$S/http"
cmp -s "$S/described" "$S/http" || { echo "    HTTP describes other tools than MCP does"; exit 1; }
# the word HTTP answers for a refusal is the one MCP carried
code="$(curl -s -o "$S/miss.json" -w '%{http_code}' "$U/api/v1/capability?id=no.such.capability")"
[ "$code" = 404 ] || { echo "    an unknown capability answered $code over HTTP"; exit 1; }
jq -e '.error.code == "not_found"' "$S/miss.json" >/dev/null || { echo "    HTTP names another category:"; cat "$S/miss.json"; exit 1; }

code="$(curl -s -o "$S/page.html" -w '%{http_code}' "$U/cockpit/mcp")"
[ "$code" = 200 ] || { echo "    /cockpit/mcp answered $code"; exit 1; }
# every tool the registry projects is on the page, and the page says where its numbers came from
jq -r '.[].name' "$S/described" > "$S/names"
missing=0
while IFS= read -r name; do
  grep -qF ">$name<" "$S/page.html" || { echo "    /cockpit/mcp does not show $name"; missing=1; }
done < "$S/names"
[ "$missing" = 0 ] || exit 1
grep -q 'data-capability="mcp.projection"' "$S/page.html" || { echo "    the page's figures name no capability"; exit 1; }
grep -qF "writes the repository" "$S/page.html" || { echo "    the page does not say which tools write"; exit 1; }
grep -qF 'majordomus-mcp' "$S/page.html" || { echo "    the page shows no client configuration"; exit 1; }
# narrowed, it shows the writers and not a read
code="$(curl -s -o "$S/writers.html" -w '%{http_code}' "$U/cockpit/mcp?effect=repository_mutation")"
[ "$code" = 200 ] || { echo "    the narrowed page answered $code"; exit 1; }
grep -qF '>majordomus_plan_transition<' "$S/writers.html" || { echo "    the narrowed page lost the writer"; exit 1; }
# the area is in the navigation of every page, so a reader and the route crawl both reach it
curl -fsS "$U/cockpit" | grep -q 'href="/cockpit/mcp"' || { echo "    the Cockpit's navigation has no MCP entry"; exit 1; }
serve_down

# --- 5. describing a projection changes nothing
after="$(git status --porcelain; git ls-files -s | mj_sha256sum)"
[ "$before" = "$after" ] || { echo "    the repository changed:"; git status --porcelain; exit 1; }
