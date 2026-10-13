# majordomus-covers: none
# claims: mcp-peers
# The peer board was readable over MCP and HTTP only, so a worker whose client had dropped
# the majordomus_* tools had nothing to ask with. `majordomus peers` is the command-line
# projection of the same capability: a peer announced over MCP by another client of the
# repository is listed with its worktree and the paths it claimed, as text and as JSON.
#
# What it must not become:
#
#   a second board. The command is bound to `peers.list` through the capability's own
#   exposure, and the JSON it prints is the capability's answer.
#
#   an empty board that was never read. With no server answering for any checkout the
#   command says that no board could be asked and exits 12, the missing-artifact code.
#
# Real processes throughout: a real `majordomus mcp` behind the launcher, a real client on
# its stdin, and the executable asked from two checkouts of one repository.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }

# nested one level down, so the worktree container is inside the fixture; resolved, because
# git holds the resolved path and macOS's $TMPDIR is a symlink
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
trunk="$(git -C "$R" symbolic-ref --short HEAD)"
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=warn; export MAJORDOMUS_LOG

A="$R-wt/feature/alpha"
S="$T/io"; mkdir -p "$S"
mj() { local cwd="$1"; shift; ( cd "$cwd" && "$RB" "$@" ); }
cleanup() {
  exec 3>&- 2>/dev/null || true
  for w in "$A" "$R"; do "$RB" serve stop --repo "$w" >/dev/null 2>&1 || true; done
}
trap cleanup EXIT
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
await_lines() {
  local file="$1" want="$2" i=0
  until [ "$(wc -l < "$file" | tr -d ' ')" -ge "$want" ]; do
    i=$((i+1)); [ "$i" -lt 600 ] || { echo "    $file never reached $want frames:"; cat "$file"; return 1; }
    sleep 0.1
  done
}
lease_url() {   # lease_url CHECKOUT ERRFILE -> the address its server bound
  local lease="$1/.ai/local/state/mcp/server.json" url i=0
  while :; do
    url="$(jq -r '.url // empty' "$lease" 2>/dev/null || true)"
    [ -z "$url" ] || break
    i=$((i+1)); [ "$i" -lt 600 ] || { echo "    no server published a lease in $1:"; cat "$2" 2>/dev/null; return 1; }
    sleep 0.1
  done
  printf '%s\n' "$url"
}

# ---------------------------------------------------------------- the command is the capability's
exposure="$(mj "$R" capabilities list --format json 2>/dev/null | jq -c '(.capabilities // .)[] | select(.id == "peers.list") | .exposure')"
[ "$(jq -r '.cli.path | join(" ")' <<<"$exposure")" = peers ] \
  || { echo "    peers.list is not exposed as \`majordomus peers\`: $exposure"; exit 1; }
[ "$(jq -r '.mcp.tool' <<<"$exposure")" = majordomus_peers ] \
  || { echo "    the command is not the MCP tool's capability: $exposure"; exit 1; }

# ---------------------------------------------------------------- nobody serves: no board, and not 0
expect_exit 12 mj "$R" peers
expect_grep 'no board could be asked: no server answers for any checkout this answer covers'
expect_grep '^peers +0 attached, 0 of 1 board\(s\) asked'
# and the JSON is still the capability's answer, for a program that wants to see what was covered
out="$(mj "$R" peers --format json 2>/dev/null)" && { echo "    peers --format json exited 0 with no board"; exit 1; }
[ "$(jq -r '.boards | length' <<<"$out")" = 1 ] && [ "$(jq -r '.boards[0].standing' <<<"$out")" = absent ] \
  || { echo "    the JSON does not carry the checkout it covered: $out"; exit 1; }

# ---------------------------------------------------------------- a real client, in a worktree, announces
expect_exit 0 mj "$R" worktree create feature/alpha --base "$trunk"
[ -d "$A" ] || { echo "    the worktree is not at the derived path"; exit 1; }
mkfifo "$S/in_a"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_a" > "$S/out_a" 2> "$S/err_a" &
exec 3> "$S/in_a"
url_a="$(lease_url "$A" "$S/err_a")" || exit 1
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-alpha","version":"7"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the ordering rule","scope":["apps/majordomus-cli/src","docs/MCP.md"]}}'
} >&3
await_lines "$S/out_a" 2 || exit 1

# ---------------------------------------------------------------- read from its own checkout: through its server
out="$(mj "$A" peers --format json)" || { echo "    peers failed in the served checkout"; mj "$A" peers 2>&1 | tail -5; exit 1; }
peer() { jq -c '[.peers[] | select(.client.name == "worker-alpha")] | first' <<<"$1"; }
p="$(peer "$out")"
[ "$p" != null ] || { echo "    the announced peer is not on the board: $out"; exit 1; }
[ "$(jq -r '.checkout.worktree' <<<"$p")" = "$A" ] || { echo "    the peer is not stamped with its worktree: $p"; exit 1; }
[ "$(jq -r '.attached' <<<"$p")" = true ] || { echo "    the peer is not attached: $p"; exit 1; }
[ "$(jq -r '.claims[0].intent' <<<"$p")" = "the ordering rule" ] || { echo "    the claim's intent is not on the board: $p"; exit 1; }
[ "$(jq -r '.claims[0].scope | join(" ")' <<<"$p")" = "apps/majordomus-cli/src docs/MCP.md" ] \
  || { echo "    the claimed paths are not on the board: $p"; exit 1; }
# the same answer the server gives over HTTP's route, which is the MCP tool's capability
[ "$(jq -r '.boards | map(select(.this_checkout)) | first | .url' <<<"$out")" = "$url_a" ] \
  || { echo "    the answer did not come from this checkout's server: $(jq -c '.boards' <<<"$out")"; exit 1; }

# ---------------------------------------------------------------- read from a checkout nothing serves: the sibling's board
expect_exit 0 mj "$R" peers
expect_grep '^peers +1 attached, 1 of 2 board\(s\) asked'
expect_grep "^$A  \[feature/alpha\]  server ready  $url_a\$"
expect_grep '^  p[0-9]+ +worker-alpha 7  (stdio|http)  attached, seen [0-9]+s ago$'
expect_grep '^       the ordering rule$'
expect_grep '^       paths  apps/majordomus-cli/src docs/MCP\.md$'
expect_grep "^$R  \(this checkout\)  \[$trunk\]  server absent\$"
expect_no_grep 'no board could be asked'
out="$(mj "$R" peers --format json)"
[ "$(jq -r '.checkout.worktree' <<<"$(peer "$out")")" = "$A" ] \
  || { echo "    a checkout with no server does not see the sibling's peer: $out"; exit 1; }

# this checkout alone has no server, and so no board
expect_exit 12 mj "$R" peers --checkouts this
expect_grep 'no board could be asked'
expect_no_grep 'worker-alpha'

# ---------------------------------------------------------------- the shell tool routes it, never answers itself
out="$(cd "$R" && "$MJ" peers 2>&1)" || { echo "    the shell tool did not route peers: $out"; exit 1; }
grep -q 'worker-alpha' <<<"$out" || { echo "    the shell tool's peers does not list the peer: $out"; exit 1; }

# ---------------------------------------------------------------- the server goes: no board again, and not 0
exec 3>&-
"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true
i=0; until ! mj "$R" peers >/dev/null 2>&1; do i=$((i+1)); [ "$i" -lt 100 ] || { echo "    the board is still read after its server stopped"; mj "$R" peers; exit 1; }; sleep 0.2; done
expect_exit 12 mj "$R" peers
expect_grep 'no board could be asked'

# ---------------------------------------------------------------- no layer: the same code, and the reason
mkdir -p "$T/plain" && git -C "$T/plain" init -q .
expect_exit 12 mj "$T/plain" peers
expect_grep 'no Majordomus repository found'
exit 0
