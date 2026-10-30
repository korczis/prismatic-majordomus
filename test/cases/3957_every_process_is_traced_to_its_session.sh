# majordomus-covers: capability:peers.list
# Every worker, and every process it starts, is traced to its session (I3956).
#
# On 2026-10-22 a 33-minute remote `cargo test --doc` could be traced to its Claude Code
# session only by hand: ssh client, parent chain, provider pid, its messaging socket, and a
# start time matched against the session list. The board row named a client and a version.
#
# Slice 1, the board row carries the address that reaches the worker, derived from the
# environment the provider hands the MCP server it starts, never asked of the model:
#
#   the server's own stdio client     located from the server process's environment
#   a bridged client                  located from the bridge's environment, carried in the
#                                     initialize it forwards
#   a client exposing no provider     still named by its machine and its process
#   the messaging token               never on the board
#
# Real processes over real MCP, an ephemeral port, the server ended with its last client.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=info; export MAJORDOMUS_LOG

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3957.XXXXXX")"
trap 'exec 3>&- 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
init() { printf '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"%s","version":"0"}}' "$1"; }
# a process with no provider in its environment, whatever runs this case
bare() {
  env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_MESSAGING_SOCKET \
    -u CLAUDE_CODE_MESSAGING_TOKEN -u CLAUDE_PID "$@"
}
TOKEN="tok-3957-never-on-the-board"

# ---------------------------------------------------------------- 1. the server's own client
mkfifo "$S/in1"
bare env CLAUDECODE=1 CLAUDE_CODE_SESSION_ID=sess-a CLAUDE_CODE_MESSAGING_SOCKET="$S/a.sock" \
  CLAUDE_PID=4242 CLAUDE_CODE_MESSAGING_TOKEN="$TOKEN" \
  "$RB" mcp --http-port 0 < "$S/in1" > "$S/out1.txt" 2> "$S/err1.txt" & p1=$!
exec 3> "$S/in1"
i=0
until grep -q 'listening on http://' "$S/err1.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the first client's server never listened"; cat "$S/err1.txt"; exit 1; }
  sleep 0.1
done
req 1 initialize "$(init case3957-a)" >&3

# ---------------------------------------------------------------- 2. a bridged client
{
  req 1 initialize "$(init case3957-b)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"a remote derive","scope":["apps"]}}'
  req 3 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}'
} > "$S/in2"
rc=0
bare env CLAUDECODE=1 CLAUDE_CODE_SESSION_ID=sess-b CLAUDE_CODE_MESSAGING_SOCKET="$S/b.sock" \
  CLAUDE_PID=4343 CLAUDE_CODE_MESSAGING_TOKEN="$TOKEN" \
  "$RB" mcp < "$S/in2" > "$S/out2.txt" 2> "$S/err2.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the bridged client exited $rc"; cat "$S/err2.txt"; exit 1; }
grep -q 'bridging this stdio session to it' "$S/err2.txt" || { echo "    the second client did not bridge"; cat "$S/err2.txt"; exit 1; }
board="$(sed -n 3p "$S/out2.txt" | jq -c '.result.structuredContent.peers')"
[ -n "$board" ] && [ "$board" != null ] || { echo "    no board:"; cat "$S/out2.txt"; exit 1; }
row() { printf '%s' "$board" | jq -c --arg n "$1" '.[] | select(.client.name == $n) | .address'; }

a="$(row case3957-a)"
printf '%s' "$a" | jq -e --arg s "$S/a.sock" '
  .provider == "claude-code" and .session == "sess-a" and .pid == 4242
  and .socket == $s and (.host | length) > 0' >/dev/null \
  || { echo "    the server's own client is not located from its environment: $a"; exit 1; }
echo "    the server's own stdio client is located from the server's environment"

b="$(row case3957-b)"
printf '%s' "$b" | jq -e --arg s "$S/b.sock" '
  .provider == "claude-code" and .session == "sess-b" and .pid == 4343
  and .socket == $s and (.host | length) > 0' >/dev/null \
  || { echo "    the bridged client is not located from the bridge's environment: $b"; exit 1; }
echo "    a bridged client is located from what its bridge carried in initialize"

grep -qF "$TOKEN" "$S/out2.txt" && { echo "    the messaging token reached the board"; exit 1; }
grep -qF "$TOKEN" "$S/err1.txt" "$S/err2.txt" && { echo "    the messaging token reached a log"; exit 1; }
echo "    the provider's messaging token is never on the board nor in a log"

# ---------------------------------------------------------------- 3. a client with no provider
{
  req 1 initialize "$(init case3957-c)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}'
} > "$S/in3"
rc=0; bare "$RB" mcp < "$S/in3" > "$S/out3.txt" 2> "$S/err3.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the third client exited $rc"; cat "$S/err3.txt"; exit 1; }
c="$(sed -n 2p "$S/out3.txt" | jq -c '.result.structuredContent.peers[] | select(.client.name == "case3957-c") | .address')"
printf '%s' "$c" | jq -e '(.host | length) > 0 and (.pid | type) == "number" and (has("provider") | not) and (has("socket") | not)' >/dev/null \
  || { echo "    a client with no provider is not named by machine and process: $c"; exit 1; }
echo "    a client that exposes no provider is still named by its machine and its process"

exec 3>&-
rc=0; wait "$p1" || rc=$?
[ "$rc" = 0 ] || { echo "    the first client's server exited $rc"; cat "$S/err1.txt"; exit 1; }
