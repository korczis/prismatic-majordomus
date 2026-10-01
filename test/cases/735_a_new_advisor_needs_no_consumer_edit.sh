# claims: advisors-are-declared-once
# An advisor is added by declaring it, and nothing else (ADR 0098). A representative new
# advisor — `example-reviewer`, offering architecture_review through a client tool — is
# appended to the fixture's share/advisors.yaml and its executable put on the isolated PATH.
# Without one line of any consumer changed, it is reported by the command line, selected by
# the capability-driven policy for the review it offers, served over HTTP, described by the
# OpenAPI document's routes, returned over MCP and rendered on the Cockpit's Reasoning page.
# Removing its executable makes it unavailable on every surface at once.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
reasoning_fixture
# appended under `advisors:` — the shipped catalogue lists them before `peers:`
awk '
  /^peers:/ && !done {
    print "  - id: example-reviewer"
    print "    title: Example Reviewer"
    print "    transport: cli"
    print "    adapter: fixture-scripted"
    print "    executable: example-reviewer"
    print "    capabilities: [architecture_review]"
    print ""
    done = 1
  }
  { print }' "$R/share/advisors.yaml" > "$T/advisors.yaml" && mv "$T/advisors.yaml" "$R/share/advisors.yaml"
rz_stub example-reviewer

# ---------------------------------------------------------------- command line and policy
expect_exit 0 rz reasoning advisors
expect_grep "example-reviewer +available +cli +present"
expect_grep "architecture_review +example-reviewer"
expect_exit 0 rz reasoning plan --materiality high --capabilities architecture_review
expect_grep "consult     example-reviewer — covers architecture_review"
expect_grep "excluded    chatgpt — not_configured: credential_absent"
# the transport consults it, through the adapter the catalogue names
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"example-reviewer":{"answer":{"conclusion":"the_boundary_holds","stance":"supports"}}}'
A="$(rz_record '{"kind":"assessment","subject":"service boundary","materiality":"high","capabilities":["architecture_review"],"evidence":[{"kind":"adr","reference":"ADR 0001"}]}')"
expect_exit 0 rz_consult --assessment "$A"
expect_grep "advisor    example-reviewer"
expect_grep "conclusion the_boundary_holds"
RZ_ENV=""

# ---------------------------------------------------------------- MCP
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case735","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_reasoning_advisors","arguments":{}}'
  req 3 resources/read '{"uri":"majordomus://reasoning/advisors"}'
} > "$T/mcp.in"
( cd "$R" && rz_env "$RB" mcp < "$T/mcp.in" > "$T/mcp.out" 2> "$T/mcp.err" ) || { echo "    the MCP server failed"; cat "$T/mcp.err"; exit 1; }
for id in 2 3; do
  jq -e --argjson id "$id" 'select(.id==$id) | tostring | test("example-reviewer")' "$T/mcp.out" >/dev/null \
    || { echo "    MCP response $id does not name the new advisor"; cat "$T/mcp.out"; exit 1; }
done

# ---------------------------------------------------------------- HTTP, OpenAPI, Cockpit
cd "$R" || exit 1
export MAJORDOMUS_SHARE="$R/share"
PATH="$RZ_BIN:$PATH"
SRV=""; trap 'serve_down' EXIT
serve_up "$T/serve.out" "$T/serve.err" || exit 1
curl -fsS --max-time 20 "$U/api/v1/reasoning/advisors" > "$T/http.json" || { echo "    GET /api/v1/reasoning/advisors failed"; exit 1; }
jq -e '.advisors[] | select(.id=="example-reviewer")' "$T/http.json" >/dev/null || { echo "    HTTP does not serve the new advisor"; exit 1; }
curl -fsS --max-time 20 "$U/openapi.json" | jq -e '.paths["/api/v1/reasoning/advisors"].get.operationId == "reasoning.advisors"' >/dev/null \
  || { echo "    the OpenAPI document does not describe the route"; exit 1; }
curl -fsS --max-time 20 "$U/cockpit/reasoning" > "$T/cockpit.html" || { echo "    the Cockpit page failed"; exit 1; }
grep -q 'example-reviewer' "$T/cockpit.html" || { echo "    the Cockpit does not render the new advisor"; exit 1; }
serve_down

# ---------------------------------------------------------------- and its absence, everywhere at once
rm "$RZ_BIN/example-reviewer"
expect_exit 0 rz reasoning advisors
expect_grep "example-reviewer +unavailable +cli +executable_not_found"
expect_exit 0 rz reasoning plan --materiality high --capabilities architecture_review
expect_grep "outcome     decide_locally"
