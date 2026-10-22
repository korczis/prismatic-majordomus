# majordomus-covers: capability:health.coverage
# majordomus-timeout: 600
# The coverage matrix names its evidence (I1011), asked of the real executable in a
# repository `init` wrote:
#
#   1. one answer: the MCP tool and the HTTP route give the same matrix
#   2. never green without evidence: a resolved cell names its artifacts, a gap says what is
#      missing, and only a required cell or a dangling relation is a gap
#   3. the policy decides: a `coverage:` rule for commands makes `tested` required on every
#      command and on no query, and an unknown column is the policy's own finding
#   4. reading it writes nothing
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3410.XXXXXX")"; trap 'serve_down; rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
cat >> .ai/repo/policy.yaml <<'YAML'

coverage:
  require:
    - kind: command
      columns: [tested]
    - columns: [exposed, nonsense]
YAML
git add -A >/dev/null && git commit -qm install
before="$(git status --porcelain; git ls-files -s | mj_sha256sum)"

# --- 1. MCP
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3410","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_coverage","arguments":{}}'
} > "$S/in"
rc=0; "$RB" mcp --standalone < "$S/in" > "$S/out" 2> "$S/err" || rc=$?
[ "$rc" = 0 ] || { echo "    the server exited $rc at EOF"; cat "$S/err"; exit 1; }
jq -c 'select(.id == 2) | .result.structuredContent' "$S/out" > "$S/mcp.json"
jq -e '.schema == "majordomus/capability-coverage/v1"' "$S/mcp.json" >/dev/null \
  || { echo "    the tool did not answer with the matrix:"; jq -c 'select(.id == 2)' "$S/out" | cut -c1-400; exit 1; }

# --- 1. HTTP, and the two are one answer
serve_up "$S/serve.out" "$S/serve.err" || exit 1
curl -fsS --max-time 60 "$U/api/v1/health/coverage" > "$S/http.json" || { echo "    GET /api/v1/health/coverage failed"; exit 1; }
serve_down
[ "$(jq -S -c . "$S/mcp.json")" = "$(jq -S -c . "$S/http.json")" ] \
  || { echo "    MCP and HTTP answer differently"; diff <(jq -S . "$S/mcp.json") <(jq -S . "$S/http.json") | head -20; exit 1; }
echo "    ok: MCP and HTTP give one matrix of $(jq '.rows | length' "$S/http.json") capabilities"
M="$S/http.json"

# --- 2. never green without evidence
jq -e '.rows | length > 0 and all(.kind == "query" or .kind == "command")' "$M" >/dev/null \
  || { echo "    the matrix holds no row, or a row that is not an executable capability"; exit 1; }
bad="$(jq -c '[.rows[] | .capability as $c | .cells[]
  | select((.state == "resolved" and ((.artifacts // []) | length) == 0)
        or (.state == "gap" and (.gap // "") == "")
        or (.state == "gap" and (.required | not)
            and ((.gap | startswith("test end:") or startswith("subject end:")) | not)))
  | {c: $c, column, state}] | .[:5]' "$M")"
[ "$bad" = "[]" ] || { echo "    a cell is green without evidence or a gap without a reason: $bad"; exit 1; }
jq -e '.rows[] | select(.capability == "health.report") | .cells[] | select(.column == "exposed")
  | .state == "resolved" and (.artifacts | index("mcp:majordomus_health") != null)' "$M" >/dev/null \
  || { echo "    health.report's exposure is not named"; exit 1; }
[ "$(jq '[.tallies[].gaps] | add' "$M")" = "$(jq '.gaps' "$M")" ] || { echo "    the tallies do not count the gaps"; exit 1; }
echo "    ok: every resolved cell names an artifact, every gap says why"

# --- 3. the policy decides
wrong="$(jq -c '[.rows[] | {c: .capability, k: .kind, r: (.cells[] | select(.column == "tested") | .required)}
  | select(.r != (.k == "command"))] | .[:3]' "$M")"
[ "$wrong" = "[]" ] || { echo "    'tested' is required where the policy does not say so, or not where it does: $wrong"; exit 1; }
jq -e '[.rows[].cells[] | select(.column == "exposed") | .required] | all' "$M" >/dev/null \
  || { echo "    a rule naming no kind and no stability does not reach every capability"; exit 1; }
jq -e '.findings | length == 1 and (.[0] | contains("nonsense"))' "$M" >/dev/null \
  || { echo "    an unknown column is not the policy's finding: $(jq -c '.findings' "$M")"; exit 1; }
echo "    ok: the policy decides what is required, and names its own mistake"

# --- 4. nothing written
after="$(git status --porcelain; git ls-files -s | mj_sha256sum)"
[ "$before" = "$after" ] || { echo "    reading the matrix changed the repository"; git status --porcelain; exit 1; }
echo "    ok: reading the matrix wrote nothing"
