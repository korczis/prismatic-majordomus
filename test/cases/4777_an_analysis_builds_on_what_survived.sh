# majordomus-skill: evidence-tagged-analysis
# claims: an-analysis-builds-on-what-survived
# An evidence-tagged multi-pass analysis is checked, not trusted (I4796). The skill's
# conforming bundle conforms and the command exits 0. Then one defect at a time is planted in
# a copy, and each is named with exit 10 (the MCP tool gives the same answer for the
# conforming bundle):
# - a synthesis root cause built on a REFUTED finding;
# - an UNVERIFIABLE finding the gaps ledger does not name;
# - an OBSERVED finding without a locator;
# - a finding no cross-check judged;
# - a Markdown file that declares no pass.
# A synthesis without its cross-checks is incomplete, not ok. A path outside the repository
# is refused.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture

reset() { rm -rf "$R/analysis"; cp -R "$ROOT/test/fixtures/analysis/conforming" "$R/analysis"; }
fails() { expect_exit 10 rz reasoning analysis --path analysis; expect_grep "fails"; expect_grep "$1"; reset; }

# ---------------------------------------------------------------- the conforming bundle
reset
expect_exit 0 rz reasoning analysis --path analysis
expect_grep "analysis    cache-misses · conforms"
expect_grep "promise     structural conformance"
J="$(rz_json reasoning analysis --path analysis)"
[ "$(printf '%s' "$J" | jq -r '.ok')" = true ] || { echo "    the conforming bundle is not ok"; exit 1; }
[ "$(printf '%s' "$J" | jq -r '.counts.findings')" = 5 ] || { echo "    five findings were expected"; exit 1; }
[ "$(printf '%s' "$J" | jq -r '[.checks[] | select(.state == "passed")] | length')" = 7 ] \
  || { echo "    not every check passed"; exit 1; }

# ---------------------------------------------------------------- the same answer over MCP
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4777","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_reasoning_analysis","arguments":{"path":"analysis"}}}\n'
} | ( cd "$R" && rz_env "$RB" mcp --standalone --repo "$R" ) 2> "$T/mcp.err" | sed -n 2p > "$T/frame.json"
jq -e '.result.structuredContent | .state == "conforms" and .ok == true and .analysis == "cache-misses"' "$T/frame.json" >/dev/null \
  || { echo "    majordomus_reasoning_analysis did not answer conforms over MCP:"; head -c 600 "$T/frame.json"; sed 's/^/    | /' "$T/mcp.err" | head -5; exit 1; }

# ---------------------------------------------------------------- planted defects
sed -i.bak 's/| RC-b | Every miss in the cold window costs a full read | load-F2 |/| RC-b | Half of the reads miss | load-F1 |/' "$R/analysis/synthesis.md"
fails "load-F1 is REFUTED and is cited outside the refuted register"

sed -i.bak 's/^- cache-F3: whether eviction drops hot keys needs a run with eviction disabled\./- eviction was not tested./' "$R/analysis/synthesis.md"
fails "cache-F3 is UNVERIFIABLE and the synthesis's gaps ledger does not name it"

sed -i.bak 's/| OBSERVED | `src\/cache.rs:12` |/| OBSERVED | the source says so |/' "$R/analysis/lens-cache.md"
fails "cache-F1 is OBSERVED and its evidence holds no locator"

printf '| cache-F4 | Keys are hashed twice | INFERRED | a profile would show it |\n' > "$T/row"
awk -v row="$(cat "$T/row")" '{print} /^\| cache-F3 /{print row}' "$R/analysis/lens-cache.md" > "$T/lens" && cp "$T/lens" "$R/analysis/lens-cache.md"
fails "cache-F4 was never put to refutation"

printf '# Notes\n\nScratch.\n' > "$R/analysis/notes.md"
fails "declares no \`analysis:\` front matter"

# ---------------------------------------------------------------- incomplete is not ok
rm "$R/analysis/cross-check-cache.md" "$R/analysis/cross-check-load.md"
expect_exit 10 rz reasoning analysis --path analysis
expect_grep "incomplete"
expect_grep "citations  not_run  no cross-check document"
reset

# ---------------------------------------------------------------- only the repository
expect_exit 13 rz reasoning analysis --path ../elsewhere
