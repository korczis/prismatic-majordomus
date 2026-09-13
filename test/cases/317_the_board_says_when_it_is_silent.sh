# majordomus-covers: none
# A board that nobody spoke on does not read like a board nobody is on. ADR 0044.
#
# ADR 0044 taught `peers.list` to say how much of the repository it could see: `boards`
# names every checkout the answer covers and `complete` goes false when one could not be
# asked, "so that 'nobody else is here' is never reported when the truth is 'I could not
# ask'". That is one half of the verdict discipline — the *coverage* half — and it was
# built carefully.
#
# The other half had no counterpart. A board can be perfectly complete, hold every worker
# in the repository, and still be unable to answer the question it is read for, because
# being attached is not the same as having said anything. Measured on 2026-09-12, after a
# day of five concurrent sessions: every session was attached, not one had announced, and
# the answer they each received was `complete: true` with an empty `overlaps` — the exact
# shape of a clean board. Three branches allocated one case-number range, three sessions
# started the same one-line fix, and two sessions edited one case file at once. Every one
# of those collisions was invisible in an answer that looked, field for field, like an
# answer saying there was no collision.
#
# `project.a-verdict-states-its-subject`: a board that is empty because nobody announced
# and a board that is empty because nobody is here must not read the same. `PeerList::silent`
# is that statement, and this case is its proof.
#
# What this proves, over real processes and real MCP clients, not over a mocked board:
#   1. three workers attached and silent, and three workers attached having each announced,
#      are two situations that differ in NOTHING a reader checked before this field
#      existed — same `count`, same `complete: true`, same empty `overlaps`. This is the
#      section the whole case exists for: delete `silent` and the two answers become one;
#   2. the number is the count of silent peers and moves one at a time as they speak;
#   3. it is computed across the gathered union, not per board, so a worker quiet in
#      another worktree is counted here;
#   4. a DEPARTED peer is not silent, whatever it announced — the number is about ground
#      being worked right now, and a peer that left is the opposite problem;
#   5. the `initialize` instructions and `peers.list` report the same number because they
#      ask the same predicate. Two derivations of "announced nothing" would be two numbers
#      free to disagree, and the one a worker acts on is not the instructions string.
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
# the repository-wide board, as alpha's first client sees it. `n` is the frame to read,
# which is this client's request count: the launcher's contract is a frame per line.
board() {   # board <id> <outfile>
  req "$1" tools/call '{"name":"majordomus_peers","arguments":{}}' >&3
  await_lines "$S/out_a" "$1" || return 1
  sed -n "$1p" "$S/out_a" | jq -e '.result.isError == false' >/dev/null \
    || { echo "    the board is an error:"; sed -n "$1p" "$S/out_a"; return 1; }
  sed -n "$1p" "$S/out_a" | jq '.result.structuredContent' > "$2"
}

# ---------------------------------------------------------------- the fixture: two worktrees, three workers
expect_exit 0 mj "$R" worktree create feature/alpha --base "$trunk"
expect_exit 0 mj "$R" worktree create feature/beta --base "$trunk"

mkfifo "$S/in_a" "$S/in_b" "$S/in_c"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_a" > "$S/out_a" 2> "$S/err_a" & job_a=$!
exec 3> "$S/in_a"
url_a="$(lease_url "$A" "$S/err_a")" || exit 1
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_b" > "$S/out_b" 2> "$S/err_b" & job_b=$!
exec 4> "$S/in_b"
( cd "$B" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_c" > "$S/out_c" 2> "$S/err_c" & job_c=$!
exec 5> "$S/in_c"
url_b="$(lease_url "$B" "$S/err_c")" || exit 1

# three workers: two sharing alpha's server, one in beta's own checkout. None announces —
# which is not a contrived fixture, it is what five sessions of this repository did all day.
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-alpha","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&3
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-alpha-two","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&4
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-beta","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&5
await_lines "$S/out_a" 1 || exit 1
await_lines "$S/out_b" 1 || exit 1
await_lines "$S/out_c" 1 || exit 1
[ "$url_a" != "$url_b" ] || { echo "    the two checkouts share one address"; exit 1; }

# ---------------------------------------------------------------- 1. silent, and the answer says so
board 2 "$S/silent.json" || exit 1
jq -e '.complete == true and .count == 3' "$S/silent.json" >/dev/null \
  || { echo "    the three workers are not all on the gathered board:"; cat "$S/silent.json"; exit 1; }
jq -e '.silent == 3' "$S/silent.json" >/dev/null \
  || { echo "    THREE ATTACHED WORKERS ANNOUNCED NOTHING AND THE BOARD DOES NOT SAY SO: silent is $(jq '.silent' "$S/silent.json")"; cat "$S/silent.json"; exit 1; }
# every field a reader checked before this one says the board is clean
jq -e '(.overlaps | length) == 0' "$S/silent.json" >/dev/null \
  || { echo "    the fixture was supposed to make no claims at all:"; cat "$S/silent.json"; exit 1; }
# 3. across the union: beta's worker is counted here, from alpha's side of the repository
jq -e '[.peers[] | select(.client.name == "worker-beta") | select(.checkout.this_checkout == false)] | length == 1' "$S/silent.json" >/dev/null \
  || { echo "    the silent count is not being taken across the gathered union:"; cat "$S/silent.json"; exit 1; }

# ---------------------------------------------------------------- 2. it moves one at a time
req 3 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the ordering rule","scope":["apps/majordomus-cli/src/order.rs"]}}' >&3
await_lines "$S/out_a" 3 || exit 1
board 4 "$S/one.json" || exit 1
jq -e '.count == 3 and .silent == 2' "$S/one.json" >/dev/null \
  || { echo "    one worker spoke and the silent count did not follow:"; cat "$S/one.json"; exit 1; }

req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the release model","scope":["apps/majordomus-cli/src/release"]}}' >&4
await_lines "$S/out_b" 2 || exit 1
board 5 "$S/two.json" || exit 1
jq -e '.count == 3 and .silent == 1' "$S/two.json" >/dev/null \
  || { echo "    two workers spoke and the silent count did not follow:"; cat "$S/two.json"; exit 1; }

req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"the why catalogue","scope":["apps/majordomus-cli/src/why.rs"]}}' >&5
await_lines "$S/out_c" 2 || exit 1
board 6 "$S/spoken.json" || exit 1

# ------------------------------- 1b. THE ASSERTION THIS CASE EXISTS FOR: the two answers differ
# Everybody has now announced, and nothing collides. Compare that answer with the one from
# section 1, where NOBODY had announced. Before `silent` existed these two were identical in
# every field a worker reads to decide whether it is safe to allocate an identifier or fan
# work out — same count, same completeness, same empty overlaps — and the repository spent a
# day acting on the first as though it were the second. The one field that tells them apart
# is the whole of the fix; remove it and this section is what fails.
jq -e '.complete == true and .count == 3 and (.overlaps | length) == 0' "$S/spoken.json" >/dev/null \
  || { echo "    the announced board is not the clean board this section needs:"; cat "$S/spoken.json"; exit 1; }
jq -e '.silent == 0' "$S/spoken.json" >/dev/null \
  || { echo "    every worker announced and the board still calls one of them silent:"; cat "$S/spoken.json"; exit 1; }
# the discrimination itself, stated as the rule states it
for f in complete count overlaps; do
  a="$(jq -c ".$f" "$S/silent.json")"; b="$(jq -c ".$f" "$S/spoken.json")"
  [ "$a" = "$b" ] || { echo "    the fixture drifted: .$f differs ($a / $b) and the two boards must differ ONLY in .silent"; exit 1; }
done
[ "$(jq -c '.silent' "$S/silent.json")" != "$(jq -c '.silent' "$S/spoken.json")" ] \
  || { echo "    A BOARD NOBODY SPOKE ON READS EXACTLY LIKE A BOARD EVERYBODY SPOKE ON: the verdict does not state its subject"; exit 1; }

# ---------------------------------------------------------------- 4. a departed peer is not silent
# It announced and then went away, so its claim stands and it is not a blind spot; and a
# peer that went away having said nothing is not counted either, because the number is
# about ground being worked right now.
exec 5>&-
wait "$job_c" 2>/dev/null || true
i=0; while [ -f "$B/.ai/local/state/mcp/server.json" ]; do
  i=$((i+1)); [ "$i" -lt 100 ] || { "$RB" serve stop --repo "$B" >/dev/null 2>&1 || true; break; }
  sleep 0.1
done
board 7 "$S/departed.json" || exit 1
jq -e '.complete == true and .count == 2 and .silent == 0' "$S/departed.json" >/dev/null \
  || { echo "    a departed worker changed the silent count:"; cat "$S/departed.json"; exit 1; }

# and a peer that leaves WITHOUT announcing leaves no trace at all: `detach` retains a peer
# only for the claim it made, so a silent one is removed rather than kept with
# `attached: false`. That is why this section cannot prove the `attached` term of the
# predicate — the state is unreachable through a board, and a mutation dropping that term
# survives this whole case. It is reachable off the wire, where a sibling server's board is
# parsed from JSON, and it is proven there: `a_departed_peer_is_not_silent_however_it_arrives`
# in apps/majordomus-cli/src/peers.rs. What this section does prove is that a silent worker
# arriving is counted and that its departure is not left behind as a phantom blind spot.
mkfifo "$S/in_d"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_d" > "$S/out_d" 2> "$S/err_d" & job_d=$!
exec 6> "$S/in_d"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-delta","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&6
await_lines "$S/out_d" 1 || exit 1
board 8 "$S/delta_here.json" || exit 1
jq -e '.count == 3 and .silent == 1' "$S/delta_here.json" >/dev/null \
  || { echo "    a fourth silent worker arrived and was not counted:"; cat "$S/delta_here.json"; exit 1; }
exec 6>&-
wait "$job_d" 2>/dev/null || true
board 9 "$S/delta_gone.json" || exit 1
jq -e '.count == 2 and .silent == 0' "$S/delta_gone.json" >/dev/null \
  || { echo "    a worker that left without announcing is still counted as a blind spot:"; cat "$S/delta_gone.json"; exit 1; }

# ---------------------------------------------------------------- 5. one predicate, two surfaces
# The `initialize` instructions have said "N attached peer(s) have announced nothing" since
# before `peers.list` could. Two independent derivations of that predicate are two numbers
# free to drift, and the one a worker acts on is the capability, not the greeting. They ask
# `peers::is_silent` and this is the assertion that would fail if one of them stopped.
mkfifo "$S/in_e"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_e" > "$S/out_e" 2> "$S/err_e" & job_e=$!
exec 7> "$S/in_e"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-epsilon","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&7
await_lines "$S/out_e" 1 || exit 1
# a second silent worker, so the instructions have a non-zero number to state about someone
# other than the caller
mkfifo "$S/in_f"
( cd "$A" && exec "$LAUNCHER" --http-port 0 ) < "$S/in_f" > "$S/out_f" 2> "$S/err_f" & job_f=$!
exec 8> "$S/in_f"
{ req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker-zeta","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'; } >&8
await_lines "$S/out_f" 1 || exit 1
sed -n 1p "$S/out_f" | jq -r '.result.instructions' > "$S/instructions.txt"
# zeta is the caller and excludes itself; epsilon is the one other silent peer
expect_grep '1 attached peer\(s\) have announced nothing' "$S/instructions.txt"
req 10 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}' >&3
await_lines "$S/out_a" 10 || exit 1
sed -n 10p "$S/out_a" | jq '.result.structuredContent' > "$S/narrow.json"
# alpha's own board: alpha (announced), alpha-two (announced), epsilon, zeta => 2 silent,
# which is the instructions' 1 plus the caller the instructions excluded
jq -e '.silent == 2' "$S/narrow.json" >/dev/null \
  || { echo "    the capability and the instructions disagree about who is silent:"; cat "$S/narrow.json" "$S/instructions.txt"; exit 1; }

exec 7>&-; exec 8>&-
wait "$job_e" 2>/dev/null || true
wait "$job_f" 2>/dev/null || true
exec 3>&-; exec 4>&-
"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true
wait "$job_a" 2>/dev/null || true
wait "$job_b" 2>/dev/null || true
echo "    a board nobody spoke on no longer reads like a board everybody spoke on"
