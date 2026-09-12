# majordomus-covers: none
# The peer board, across the worktrees a branch is actually worked on in. ADR 0044.
#
# Every worker of this repository is bootstrapped with the sentence "one shared server
# serves this repository, and every worker attached to it is visible to every other".
# Measured on 2026-09-11 that was false: seven live servers over one git repository, seven
# boards, and nine agents across sixty worktrees each reading a board that held only
# itself. ADR 0035 had already given `server.status` the checkouts of a repository and a
# peer *count* per checkout; a count is not a board, and what a worker needed was the other
# worker's claimed paths.
#
# What this proves, over real processes and real MCP clients, not over a mocked board:
#   1. two worktrees of one repository, a client attached in each through the launcher a
#      client configuration names, and each one sees the other's worker, stamped with the
#      branch and worktree it is on;
#   2. an overlap between two claims made in two different worktrees is reported — the
#      collision that was invisible until now;
#   3. `checkouts: this` is narrow: one board, and no other checkout enumerated;
#   4. a peer that announced NOTHING crosses the wire intact. That shape had never been
#      read back before the board was gathered, and it is the one that breaks quietly: a
#      field skipped when absent must also be defaulted, or one silent peer makes a whole
#      sibling board unreadable and the answer says `complete: false`;
#   5. concurrent entry from both worktrees at once leaves one lease and one server per
#      checkout, with no address taken twice;
#   6. a checkout whose server was stopped is covered and empty, not dropped, and the
#      answer still says it is complete — "nobody is attached there" and "I could not ask"
#      are different facts and are reported differently.
#
# Nothing here is stopped by pattern and no default port is ever bound: this machine runs
# the servers of other checkouts, every server is started with an ephemeral port, and each
# is ended through the lease of its own checkout.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }

# the repository is nested one level down: its worktree container is by definition its
# sibling, so a repository at $T would leave a container beside $T. Paths are resolved,
# because git holds the resolved path and macOS's $TMPDIR is a symlink.
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

A="$R-wt/feature/alpha"; B="$R-wt/feature/beta"
S="$T/io"; mkdir -p "$S"

mj() { local cwd="$1"; shift; ( cd "$cwd" && "$RB" "$@" ); }
# each server is ended through its own checkout's lease, never by a pattern
cleanup() {
  exec 3>&- 2>/dev/null || true
  exec 4>&- 2>/dev/null || true
  exec 5>&- 2>/dev/null || true
  for w in "$A" "$B" "$R"; do "$RB" serve stop --repo "$w" >/dev/null 2>&1 || true; done
}
trap cleanup EXIT

req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
# wait until a client's stdout holds N complete frames; a frame per line is the launcher's
# contract, and polling the file is how a shell waits on a process it is also writing to
await_lines() {
  local file="$1" want="$2" i=0
  until [ "$(wc -l < "$file" | tr -d ' ')" -ge "$want" ]; do
    i=$((i+1)); [ "$i" -lt 600 ] || { echo "    $file never reached $want frames:"; cat "$file"; return 1; }
    sleep 0.1
  done
}
# the address a checkout's server published, from the lease it wrote. Read from the lease
# and not from the launcher's log, because the log level a case sets is not a contract and
# `warn` hides the line that names the address; the lease is the rendezvous by design.
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

# ---------------------------------------------------------------- 1. two worktrees, by the topology
expect_exit 0 mj "$R" worktree create feature/alpha --base "$trunk"
expect_exit 0 mj "$R" worktree create feature/beta --base "$trunk"
[ -d "$A" ] && [ -d "$B" ] || { echo "    the worktrees are not at the derived paths"; exit 1; }

# ---------------------------------------------------------------- 2. a real client in each
mkfifo "$S/in_a" "$S/in_b"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_a" > "$S/out_a" 2> "$S/err_a" & job_a=$!
exec 3> "$S/in_a"
( cd "$B" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_b" > "$S/out_b" 2> "$S/err_b" & job_b=$!
exec 4> "$S/in_b"
url_a="$(lease_url "$A" "$S/err_a")" || exit 1
url_b="$(lease_url "$B" "$S/err_b")" || exit 1
[ -n "$url_a" ] && [ "$url_a" != "$url_b" ] \
  || { echo "    the two worktrees share one address ($url_a / $url_b)"; exit 1; }

# each announces a claim of its own; the two claims meet, which is the collision that could
# not be seen before: one is inside the other and they are in different worktrees
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-alpha","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the ordering rule","scope":["apps/majordomus-cli/src"]}}'
} >&3
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-beta","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the release model","scope":["apps/majordomus-cli/src/release"]}}'
} >&4
await_lines "$S/out_a" 2 || exit 1
await_lines "$S/out_b" 2 || exit 1

# ---------------------------------------------------------------- 3. alpha sees beta
req 3 tools/call '{"name":"majordomus_peers","arguments":{}}' >&3
await_lines "$S/out_a" 3 || exit 1
sed -n 3p "$S/out_a" | jq -e '.result.isError == false' >/dev/null \
  || { echo "    the repository-wide board is an error:"; sed -n 3p "$S/out_a"; exit 1; }
sed -n 3p "$S/out_a" | jq '.result.structuredContent' > "$S/board_a.json"

jq -e '.complete == true' "$S/board_a.json" >/dev/null \
  || { echo "    a board that could not be read is not a proof of anything:"; cat "$S/board_a.json"; exit 1; }
jq -e '.count == 2' "$S/board_a.json" >/dev/null \
  || { echo "    the board holds $(jq '.count' "$S/board_a.json") attached peers, not the two workers:"; cat "$S/board_a.json"; exit 1; }
# three checkouts of one repository are covered, the serverless primary included
jq -e '(.boards | length) == 3' "$S/board_a.json" >/dev/null \
  || { echo "    the answer covers $(jq '.boards|length' "$S/board_a.json") checkouts, not three:"; cat "$S/board_a.json"; exit 1; }
jq -e --arg t "$trunk" '.boards[] | select(.branch == $t) | .standing == "absent" and .attached == 0' "$S/board_a.json" >/dev/null \
  || { echo "    the primary has no server and the answer does not say so:"; cat "$S/board_a.json"; exit 1; }
# each worker is on the board, stamped with the checkout it is attached to
jq -e --arg w "$A" '.peers[] | select(.client.name == "worker-alpha")
  | .checkout.this_checkout == true and .checkout.worktree == $w and .checkout.branch == "feature/alpha"
    and .announcement.intent == "the ordering rule"' "$S/board_a.json" >/dev/null \
  || { echo "    alpha does not see itself, or is not stamped with its own checkout:"; cat "$S/board_a.json"; exit 1; }
jq -e --arg w "$B" '.peers[] | select(.client.name == "worker-beta")
  | .checkout.this_checkout == false and .checkout.worktree == $w and .checkout.branch == "feature/beta"
    and .announcement.intent == "the release model"
    and .announcement.scope == ["apps/majordomus-cli/src/release"]' "$S/board_a.json" >/dev/null \
  || { echo "    ALPHA CANNOT SEE BETA: the board is still per-checkout"; cat "$S/board_a.json"; exit 1; }
# the two checkout identities differ and each board names the address its lease holds
jq -e '[.boards[].id] | unique | length == 3' "$S/board_a.json" >/dev/null \
  || { echo "    three checkouts do not report three identities:"; cat "$S/board_a.json"; exit 1; }
jq -e --arg u "$url_b" '.boards[] | select(.branch == "feature/beta") | .url == $u and .attached == 1' "$S/board_a.json" >/dev/null \
  || { echo "    beta's board is not the server beta is actually running:"; cat "$S/board_a.json"; exit 1; }

# ---------------------------------------------------------------- 4. the collision, across worktrees
jq -e '(.overlaps | length) == 1' "$S/board_a.json" >/dev/null \
  || { echo "    two claims that meet in two worktrees are not reported as an overlap:"; cat "$S/board_a.json"; exit 1; }
jq -e '.overlaps[0] | .intent == "the ordering rule" and .attached == true
  and .checkout.branch == "feature/alpha"
  and .paths[0].yours == "apps/majordomus-cli/src/release"
  and .paths[0].theirs == "apps/majordomus-cli/src"' "$S/board_a.json" >/dev/null \
  || { echo "    the overlap does not name which worktree the other worker is in:"; cat "$S/board_a.json"; exit 1; }

# and beta sees the same repository from the other side
req 3 tools/call '{"name":"majordomus_peers","arguments":{}}' >&4
await_lines "$S/out_b" 3 || exit 1
sed -n 3p "$S/out_b" | jq '.result.structuredContent' > "$S/board_b.json"
jq -e '.complete == true and .count == 2' "$S/board_b.json" >/dev/null \
  || { echo "    beta does not see the repository:"; cat "$S/board_b.json"; exit 1; }
jq -e '.peers[] | select(.client.name == "worker-alpha") | .checkout.this_checkout == false' "$S/board_b.json" >/dev/null \
  || { echo "    BETA CANNOT SEE ALPHA:"; cat "$S/board_b.json"; exit 1; }
# the ids collide across boards, which is exactly why a peer carries its checkout
jq -e '[.peers[].id] | length == 2' "$S/board_b.json" >/dev/null || { cat "$S/board_b.json"; exit 1; }
jq -e '[.peers[] | {id, c: .checkout.id}] | unique | length == 2' "$S/board_b.json" >/dev/null \
  || { echo "    two peers of two boards are not told apart:"; cat "$S/board_b.json"; exit 1; }

# ---------------------------------------------------------------- 5. the narrow answer is narrow
req 4 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}' >&3
await_lines "$S/out_a" 4 || exit 1
sed -n 4p "$S/out_a" | jq '.result.structuredContent' > "$S/narrow_a.json"
jq -e '.count == 1 and (.boards | length) == 1 and .complete == true
  and .boards[0].this_checkout == true and .boards[0].branch == "feature/alpha"
  and (.peers | length) == 1 and .peers[0].client.name == "worker-alpha"
  and (.overlaps | length) == 0' "$S/narrow_a.json" >/dev/null \
  || { echo "    'checkouts: this' did not stay in this checkout:"; cat "$S/narrow_a.json"; exit 1; }

# ------------------------------------------------- 5b. a peer that announced nothing crosses too
# The shape nothing had ever read back before the board was gathered: a client attached to
# a sibling's server that has not announced serialises with no `announcement` (and, sending
# no title, no `client.title`). A field skipped when absent must also be defaulted, or one
# silent peer makes an entire sibling board unreadable and the answer says `complete:
# false` -- this decision's own failure mode, arriving by another door.
mkfifo "$S/in_c"
( cd "$B" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_c" > "$S/out_c" 2> "$S/err_c" & job_c=$!
exec 5> "$S/in_c"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-gamma","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
} >&5
await_lines "$S/out_c" 1 || exit 1
# it joined beta's server rather than electing one of its own: the lease is unchanged
[ "$(jq -r '.url' "$B/.ai/local/state/mcp/server.json")" = "$url_b" ] \
  || { echo "    the third client did not attach to beta's server"; cat "$S/err_c"; exit 1; }
req 5 tools/call '{"name":"majordomus_peers","arguments":{}}' >&3
await_lines "$S/out_a" 5 || exit 1
sed -n 5p "$S/out_a" | jq '.result.structuredContent' > "$S/silent.json"
jq -e '.complete == true and .count == 3' "$S/silent.json" >/dev/null \
  || { echo "    a peer that announced nothing was dropped, or took its whole board with it:"; cat "$S/silent.json"; exit 1; }
jq -e '.peers[] | select(.client.name == "worker-gamma")
  | .checkout.branch == "feature/beta" and .checkout.this_checkout == false
    and (.announcement | not) and (.claims | not) and .attached == true' "$S/silent.json" >/dev/null \
  || { echo "    the silent peer did not cross the wire intact:"; cat "$S/silent.json"; exit 1; }
exec 5>&-; wait "$job_c" 2>/dev/null || true

# ---------------------------------------------------------------- 6. concurrent entry is idempotent
# four `serve ensure` at once, two per worktree: the election is what makes this safe, and
# what it must not produce is a second server, a second lease or a borrowed address
ensure_pids=""; n=0
for w in "$A" "$B" "$A" "$B"; do
  n=$((n+1))
  # its own file per process: four writers appending to one are four interleaved JSON
  # documents, which is a corrupt fixture rather than a finding about the election
  ( cd "$w" && "$RB" serve ensure --port 0 --idle 120 --wait 40 --format json ) > "$S/ensure.$n.json" 2>>"$S/ensure.err" &
  ensure_pids="$ensure_pids $!"
done
# each by its own pid, never a bare `wait`: the two clients are background jobs of this
# shell too, and they are meant to outlive this step
for pid in $ensure_pids; do wait "$pid" || { echo "    a concurrent ensure failed:"; cat "$S/ensure.err"; exit 1; }; done
cat "$S"/ensure.[1-4].json > "$S/ensure.jsonl"
[ "$(grep -c '"standing"' "$S/ensure.jsonl")" = 4 ] \
  || { echo "    not every concurrent ensure answered:"; cat "$S/ensure.jsonl" "$S/ensure.err"; exit 1; }
jq -sre 'map(select(.standing != "ready")) | length == 0' "$S/ensure.jsonl" >/dev/null \
  || { echo "    a concurrent ensure did not converge on a ready server:"; cat "$S/ensure.jsonl"; exit 1; }
jq -sre 'map(select(.started == true)) | length == 0' "$S/ensure.jsonl" >/dev/null \
  || { echo "    an ensure started a second server where one was already serving:"; cat "$S/ensure.jsonl"; exit 1; }
jq -sre --arg a "$url_a" --arg b "$url_b" 'map(.url) | unique | sort == ([$a, $b] | sort)' "$S/ensure.jsonl" >/dev/null \
  || { echo "    concurrent entry moved or borrowed an address:"; cat "$S/ensure.jsonl"; exit 1; }
[ "$(jq -r '.url' "$A/.ai/local/state/mcp/server.json")" = "$url_a" ] \
  || { echo "    alpha's lease was clobbered by the concurrent entry"; cat "$A/.ai/local/state/mcp/server.json"; exit 1; }
[ "$(jq -r '.url' "$B/.ai/local/state/mcp/server.json")" = "$url_b" ] \
  || { echo "    beta's lease was clobbered by the concurrent entry"; cat "$B/.ai/local/state/mcp/server.json"; exit 1; }

# ---------------------------------------------------------------- 7. a checkout with no server is covered, not dropped
# beta's only client goes away; ADR 0003's line is that the server ends with its last
# client, and the lease goes with it. Nothing is matched by name: the job being waited on
# is this shell's own, and whatever survives the wait is ended through beta's own lease.
exec 4>&-
wait "$job_b" 2>/dev/null || true
i=0; while [ -f "$B/.ai/local/state/mcp/server.json" ]; do
  i=$((i+1)); [ "$i" -lt 100 ] || { "$RB" serve stop --repo "$B" >/dev/null 2>&1 || true; break; }
  sleep 0.1
done
[ ! -f "$B/.ai/local/state/mcp/server.json" ] || { echo "    beta's lease outlived its last client"; exit 1; }
req 6 tools/call '{"name":"majordomus_peers","arguments":{}}' >&3
await_lines "$S/out_a" 6 || exit 1
sed -n 6p "$S/out_a" | jq '.result.structuredContent' > "$S/after.json"
# "nobody is attached there" is a complete answer; only "I could not ask" is not
jq -e '.complete == true and .count == 1 and (.boards | length) == 3' "$S/after.json" >/dev/null \
  || { echo "    a checkout whose server stopped was dropped, or faked an incomplete answer:"; cat "$S/after.json"; exit 1; }
jq -e '.boards[] | select(.branch == "feature/beta")
  | .standing == "absent" and .attached == 0 and (.reason | not) and (.url | not)' "$S/after.json" >/dev/null \
  || { echo "    beta is not reported as the serverless checkout it now is:"; cat "$S/after.json"; exit 1; }
jq -e '[.peers[] | select(.client.name == "worker-beta")] | length == 0' "$S/after.json" >/dev/null \
  || { echo "    a board that is gone is still being reported:"; cat "$S/after.json"; exit 1; }

exec 3>&-
"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true
wait "$job_a" 2>/dev/null || true
echo "    two worktrees of one repository see each other's workers, their claims and their collision"
