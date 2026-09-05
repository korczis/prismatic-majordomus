# majordomus-covers: none
# The canonical architecture through the built binary, in a repository the shell tool's own
# `init` wrote: the registry validates with its modules and complete benchmark coverage, the
# coverage denominator is generated (it equals the executables times their exposures plus the
# system targets), the generated projections committed in this tree are in sync, a benchmark
# run is a versioned document written under the local half, and hundreds of MCP requests
# leave every startup counter unchanged (no repository scan, no index or registry build, no
# schema generation, no projection build per request). No HTTP client is used here (rule
# project.no-network-no-eval): the socket side is the crate's own suite.
#
# Skips itself when cargo is absent, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || { echo "    skip: cargo not installed"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj91.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RUSTFLAGS='' cargo build -q --manifest-path "$MANIFEST" 2>"$S/build.log" || { cat "$S/build.log"; echo "    cargo build failed"; exit 1; }
RB="$ROOT/apps/majordomus-cli/target/debug/majordomus"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# --- the committed projections of this tree are the registry's (generate --check in the root)
( cd "$ROOT" && expect_exit 0 "$RB" generate --check && expect_grep 'docs/generated/benchmarks.md' && expect_grep 'docs/generated/registry.json' && expect_grep 'docs/generated/modules/objects.md' && expect_grep 'generate --check: in sync' ) || exit 1
jq -e '.schema == "majordomus/capability-registry/v1" and (.modules | length) >= 5 and ([.capabilities[].id] | index("objects.search") != null) and (.fingerprint == null)' "$ROOT/docs/generated/registry.json" >/dev/null \
  || { echo "    registry.json is not the builtin registry as data"; exit 1; }
grep -q '^| total | ' "$ROOT/docs/generated/benchmarks.md" || { echo "    benchmarks.md carries no coverage tally"; exit 1; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# --- the registry validates: modules, benchmark coverage, every projection
expect_exit 0 "$RB" capabilities validate
expect_grep '^OK   modules '
expect_grep '^OK   benchmarks .* cover .* requirement'
expect_grep 'validate: 0 failure\(s\)'

# --- coverage: complete, and its denominator is computed, not written
expect_exit 0 "$RB" bench coverage --check
expect_grep '^missing +0$'
expect_grep '^waived +0$'
"$RB" bench coverage --format json 2>/dev/null > "$S/coverage.json"
"$RB" capabilities list --format json 2>/dev/null > "$S/caps.json"
expected="$(jq '[.capabilities[] | select(.kind != "resource") | 1 + (if .exposure.mcp.tool then 1 else 0 end) + (if .exposure.http then 1 else 0 end)] | add' "$S/caps.json")"
system="$(jq '[.lines[] | select(.module == "system")] | length' "$S/coverage.json")"
required="$(jq '.tallies.total.required' "$S/coverage.json")"
[ "$required" = "$((expected + system))" ] || { echo "    coverage denominator $required != executables×exposures $expected + system $system"; exit 1; }
[ "$(jq '.tallies.total.covered' "$S/coverage.json")" = "$required" ] || { echo "    coverage is not complete"; cat "$S/coverage.json"; exit 1; }

# --- a benchmark run is a versioned document, written under the local half, never under the tree
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
"$RB" bench objects.get --transport direct --profile quick --format json 2>/dev/null > "$S/run.json" || { echo "    bench failed"; exit 1; }
jq -e '.schema == "majordomus/benchmark-result/v1" and .profile == "quick" and (.results | length) == 1 and .results[0].key == "objects.get|direct|first-object" and (.results[0].stats.samples > 0) and (.provenance.registry_fingerprint | length) == 64' "$S/run.json" >/dev/null \
  || { echo "    the run is not a result document"; cat "$S/run.json"; exit 1; }
ls .ai/local/benchmarks/*-quick.json >/dev/null 2>&1 || { echo "    the run was not written under .ai/local/benchmarks/"; exit 1; }
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    benchmarking changed the tracked tree"; git status --porcelain; exit 1; }
# no baseline for this platform yet: a check compares nothing and passes; a baseline is recorded explicitly
expect_exit 0 "$RB" bench objects.get --transport direct --profile quick --check --no-write
expect_grep 'no baseline for'
expect_exit 0 "$RB" bench baseline update --profile quick
expect_grep '\.ai/repo/benchmarks/rust/baseline\.'
expect_exit 0 "$RB" bench objects.get --transport direct --profile quick --check --no-write
expect_grep 'within policy'

# --- hundreds of MCP requests, and the startup counters do not move
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case91","version":"0"}}'
  req 2 tools/list
  req 3 resources/list
  req 4 tools/call '{"name":"majordomus_perf","arguments":{}}'
  i=5
  while [ "$i" -le 204 ]; do
    case $((i % 4)) in
      0) req "$i" tools/list ;;
      1) req "$i" resources/list ;;
      2) req "$i" tools/call '{"name":"majordomus_search","arguments":{"query":"scope"}}' ;;
      3) req "$i" tools/call '{"name":"majordomus_list","arguments":{"kind":"rule"}}' ;;
    esac
    i=$((i+1))
  done
  req 205 tools/call '{"name":"majordomus_perf","arguments":{}}'
} > "$S/session.in"
rc=0; "$RB" mcp --standalone < "$S/session.in" > "$S/session.out" 2>/dev/null || rc=$?
[ "$rc" = 0 ] || { echo "    mcp exited $rc"; exit 1; }
[ "$(wc -l < "$S/session.out" | tr -d ' ')" = 205 ] || { echo "    expected 205 frames"; exit 1; }
sed -n 4p "$S/session.out" | jq '.result.structuredContent | {repository_scans, index_builds, registry_builds, schema_generations, mcp_projection_builds, openapi_builds, http_projection_builds}' > "$S/before.json"
sed -n 205p "$S/session.out" | jq '.result.structuredContent | {repository_scans, index_builds, registry_builds, schema_generations, mcp_projection_builds, openapi_builds, http_projection_builds}' > "$S/after.json"
cmp -s "$S/before.json" "$S/after.json" || { echo "    a request rebuilt canonical state:"; diff "$S/before.json" "$S/after.json"; exit 1; }
jq -e '.repository_scans == 1 and .index_builds == 1 and .registry_builds == 1 and .mcp_projection_builds == 2' "$S/before.json" >/dev/null || { echo "    startup work is not one of each"; cat "$S/before.json"; exit 1; }
sed -n 205p "$S/session.out" | jq -e '.result.structuredContent.cache_hits > 0 and .result.structuredContent.executions >= 100' >/dev/null || { echo "    the repeated search was not answered from the cache"; exit 1; }

# --- one canonical change reaches every projection: the description of a fixture capability
# is what tests/projections.rs proves in the crate; here, the generated reference and the
# registry manifest agree with the live registry on every builtin
for id in $(jq -r '.capabilities[].id' "$ROOT/docs/generated/registry.json"); do
  grep -q "| \`$id\` |" "$ROOT/docs/generated/capabilities.md" || { echo "    $id is in registry.json and not in the reference"; exit 1; }
  grep -q "| \`$id\` |" "$ROOT/docs/generated/benchmarks.md" || { echo "    $id is in registry.json and not in the benchmark matrix"; exit 1; }
  "$RB" capabilities describe "$id" --format json 2>/dev/null | jq -e --arg id "$id" '.id == $id' >/dev/null || { echo "    $id is in registry.json and not in the live registry"; exit 1; }
done
