# majordomus-covers: none
# The shared MCP server through the launcher a client configuration names: bin/majordomus-mcp
# builds the Rust executable when it must and starts `majordomus mcp` inside a repository the
# shell tool's own `init` wrote. The first client gets the server (Swagger UI and MCP over HTTP
# on the loopback interface, the URL in the log, a lease under .ai/local/), a second client in
# the same repository bridges to it instead of starting another, the server ends with its last
# client and removes the lease, and --standalone touches neither port nor lease. The client
# configurations at the root (.mcp.json, .gemini/settings.json, .codex/config.toml) all name
# the same launcher. No HTTP client is used here (rule project.no-network-no-eval): the
# socket side is the crate's own suite, tests/mcp_shared.rs, which case 72 and CI run.
#
# Skips itself when cargo is absent, as the other Rust cases do.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || { echo "    skip: cargo not installed"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj90.XXXXXX")"; trap 'rm -rf "$S"; exec 3>&- 2>/dev/null' EXIT

# --- the client configurations name one launcher, and it is this one
for f in .mcp.json .gemini/settings.json; do
  [ -f "$ROOT/$f" ] || { echo "    $f is missing"; exit 1; }
  jq -e '.mcpServers.majordomus.command | test("bin/majordomus-mcp$")' "$ROOT/$f" >/dev/null \
    || { echo "    $f does not start bin/majordomus-mcp"; cat "$ROOT/$f"; exit 1; }
done
jq -e '.mcpServers.majordomus.type == "stdio"' "$ROOT/.mcp.json" >/dev/null || { echo "    .mcp.json is not a stdio server"; exit 1; }
grep -qE '^\[mcp_servers\.majordomus\]$' "$ROOT/.codex/config.toml" || { echo "    .codex/config.toml lacks [mcp_servers.majordomus]"; exit 1; }
grep -qE '^command = ".*bin/majordomus-mcp"$' "$ROOT/.codex/config.toml" || { echo "    .codex/config.toml does not start bin/majordomus-mcp"; exit 1; }

# --- the launcher builds (or finds) the executable and speaks MCP; stdout is protocol only
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
init='{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case90-first","version":"0"}}'
{
  req 1 initialize "$init"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_peers","arguments":{}}'
} > "$S/session.in"
rc=0; "$LAUNCHER" --http-port 0 < "$S/session.in" > "$S/out.txt" 2> "$S/err.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the launcher exited $rc"; cat "$S/err.txt"; exit 1; }
[ "$(wc -l < "$S/out.txt" | tr -d ' ')" = 2 ] || { echo "    expected 2 frames on stdout:"; cat "$S/out.txt"; exit 1; }
while IFS= read -r line; do
  printf '%s' "$line" | jq -e '.jsonrpc == "2.0" and (.id | type) == "number"' >/dev/null || { echo "    not a protocol frame: $line"; exit 1; }
done < "$S/out.txt"
# the log names the shared server and its Swagger UI, with the URL
grep -q 'shared server listening on http://127\.0\.0\.1:[0-9]*' "$S/err.txt" || { echo "    no listening line with a URL on stderr"; cat "$S/err.txt"; exit 1; }
url="$(sed -n 's/.*listening on \(http:\/\/127\.0\.0\.1:[0-9]*\).*/\1/p' "$S/err.txt" | head -n 1)"
grep -qF "$url/docs" "$S/err.txt" || { echo "    the log does not name Swagger UI at $url/docs"; exit 1; }
grep -qF "$url/mcp" "$S/err.txt" || { echo "    the log does not name MCP over HTTP at $url/mcp"; exit 1; }
# the client learns the URL and its peer id from initialize, and the board lists it
sed -n 1p "$S/out.txt" | jq -e --arg url "$url" '.result.instructions | contains($url) and contains("You are peer p1")' >/dev/null \
  || { echo "    initialize instructions do not name the server and the peer"; sed -n 1p "$S/out.txt"; exit 1; }
sed -n 2p "$S/out.txt" | jq -e '.result.structuredContent.count == 1 and .result.structuredContent.peers[0].client.name == "case90-first" and .result.structuredContent.peers[0].transport == "stdio"' >/dev/null \
  || { echo "    majordomus_peers does not list the one client"; sed -n 2p "$S/out.txt"; exit 1; }
grep -q 'shared server stopped' "$S/err.txt" || { echo "    the server did not report stopping"; cat "$S/err.txt"; exit 1; }
[ ! -f .ai/local/state/mcp/server.json ] || { echo "    the lease survived the server"; exit 1; }
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    serving changed the repository"; git status --porcelain; exit 1; }

# --- two clients, one server: the second bridges to the first and sees it as a peer
mkfifo "$S/in1"
"$LAUNCHER" --http-port 0 < "$S/in1" > "$S/out1.txt" 2> "$S/err1.txt" & p1=$!
exec 3> "$S/in1"
i=0; until grep -q 'listening on http://' "$S/err1.txt" 2>/dev/null; do i=$((i+1)); [ "$i" -lt 200 ] || { echo "    the first client's server never listened"; cat "$S/err1.txt"; exit 1; }; sleep 0.1; done
[ -f .ai/local/state/mcp/server.json ] || { echo "    no lease while the server runs"; exit 1; }
jq -e '.schema == "majordomus-mcp-lease/v1" and (.url | startswith("http://127.0.0.1:"))' .ai/local/state/mcp/server.json >/dev/null || { echo "    the lease is malformed"; cat .ai/local/state/mcp/server.json; exit 1; }
printf '%s' "$(req 1 initialize "$init")" >&3; printf '\n' >&3
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case90-second","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"case 90 is running","scope":["test/cases"]}}'
  req 3 tools/call '{"name":"majordomus_peers","arguments":{}}'
} > "$S/session2.in"
rc=0; "$LAUNCHER" < "$S/session2.in" > "$S/out2.txt" 2> "$S/err2.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the second client exited $rc"; cat "$S/err2.txt"; exit 1; }
grep -q 'bridging this stdio session to it' "$S/err2.txt" || { echo "    the second client did not bridge"; cat "$S/err2.txt"; exit 1; }
grep -q 'listening on' "$S/err2.txt" && { echo "    the second client started a second server"; cat "$S/err2.txt"; exit 1; }
sed -n 1p "$S/out2.txt" | jq -e '.result.instructions | contains("You are peer p2") and contains("case90-first")' >/dev/null \
  || { echo "    the second client was not told about the first"; sed -n 1p "$S/out2.txt"; exit 1; }
sed -n 2p "$S/out2.txt" | jq -e '.result.isError == false and .result.structuredContent.announcement.intent == "case 90 is running"' >/dev/null \
  || { echo "    the announcement was not recorded"; sed -n 2p "$S/out2.txt"; exit 1; }
sed -n 3p "$S/out2.txt" | jq -e '.result.structuredContent.count == 2 and ([.result.structuredContent.peers[].client.name] | sort == ["case90-first","case90-second"])' >/dev/null \
  || { echo "    the board does not list both clients"; sed -n 3p "$S/out2.txt"; exit 1; }
# the first client goes: the server ends, the lease is gone
exec 3>&-
rc=0; wait "$p1" || rc=$?
[ "$rc" = 0 ] || { echo "    the first client's server exited $rc"; cat "$S/err1.txt"; exit 1; }
[ ! -f .ai/local/state/mcp/server.json ] || { echo "    the lease survived the last client"; exit 1; }

# --- standalone: no port, no lease, the client alone
rc=0; "$LAUNCHER" --standalone < "$S/session.in" > "$S/out3.txt" 2> "$S/err3.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    standalone exited $rc"; cat "$S/err3.txt"; exit 1; }
grep -q 'listening on' "$S/err3.txt" && { echo "    standalone bound a port"; exit 1; }
[ ! -f .ai/local/state/mcp/server.json ] || { echo "    standalone wrote a lease"; exit 1; }
sed -n 1p "$S/out3.txt" | jq -e '.result.instructions | contains("http://") | not' >/dev/null || { echo "    standalone named a URL"; exit 1; }

# --- the launcher refuses, by name, when told not to build and the executable is absent
( MAJORDOMUS_BIN="$S/no-such-binary"; export MAJORDOMUS_BIN; expect_exit 12 "$LAUNCHER" --inspect; expect_grep 'is not an executable' ) || exit 1
( MAJORDOMUS_NO_BUILD=1 MAJORDOMUS_BUILD_PROFILE=nonexistent; export MAJORDOMUS_NO_BUILD MAJORDOMUS_BUILD_PROFILE; expect_exit 12 "$LAUNCHER" --inspect; expect_grep 'MAJORDOMUS_NO_BUILD is set' ) || exit 1
# and everything after the launcher's name reaches `majordomus mcp`
expect_exit 0 "$LAUNCHER" --help
expect_grep '^Usage: majordomus mcp'
expect_grep 'standalone'

# --- the justfile routes the Rust commands to the executable (when just is installed)
if command -v just >/dev/null 2>&1; then
  (cd "$ROOT" && just --list --unsorted) > "$S/just.txt" 2>&1 || { echo "    just --list failed"; cat "$S/just.txt"; exit 1; }
  for r in mcp serve inspect capabilities validate generate rust-check bench; do
    grep -qE "^\s+$r( |$)" "$S/just.txt" || { echo "    justfile lacks the recipe $r"; cat "$S/just.txt"; exit 1; }
  done
else
  echo "    note: just not installed; the justfile recipes are not listed"
fi
