# majordomus-covers: none
# A crash, at the level a client lives at. The crate's suite kills a server from Rust
# (tests/mcp_shared.rs, tests/serve_lifecycle.rs); nothing until now killed one from a
# shell, through the launcher a client configuration names, and asked the repository to
# keep what `project.shared-server-resilience` claims: nothing a client leaves behind locks
# another client out.
#
# The server is killed by the pid it recorded in its own lease, with SIGKILL, which it
# cannot catch and so cannot clean up after. Nothing here matches a process by name: this
# machine runs servers of other checkouts, and a pattern kill would take them with it.
#
# What it proves, in order: SIGKILL leaves the lease behind; `serve status` calls that
# lease stale and names the shape it found; the next client through the launcher takes it
# over, says which shape it was, binds a new address and gets a working session; the lease
# then names the new server and nothing of the old one; and the repository was never
# written to.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj113.XXXXXX")"
lease=.ai/local/state/mcp/server.json
# whatever this case leaves running is ended by the pid its own lease records, never by a
# pattern: the servers of other checkouts on this machine answer to the same name
cleanup() {
  if [ -f "$lease" ]; then
    p="$(jq -r '.pid // empty' "$lease" 2>/dev/null || true)"
    case "$p" in ''|*[!0-9]*) ;; *) kill "$p" 2>/dev/null || true ;; esac
  fi
  rm -rf "$S"
}
trap cleanup EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
session() {   # session NAME > file
  req 1 initialize "{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"$1\",\"version\":\"0\"}}"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_repository","arguments":{}}'
  req 3 tools/call '{"name":"majordomus_peers","arguments":{}}'
}

# ---------------------------------------------------------------- a client, and its server
mkfifo "$S/in1"
"$LAUNCHER" --http-port 0 < "$S/in1" > "$S/out1.txt" 2> "$S/err1.txt" & job1=$!
exec 3> "$S/in1"
i=0; until grep -q 'listening on http://' "$S/err1.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the first client's server never listened"; cat "$S/err1.txt"; exit 1; }; sleep 0.1
done
url1="$(sed -n 's/.*listening on \(http:\/\/127\.0\.0\.1:[0-9]*\).*/\1/p' "$S/err1.txt" | head -n 1)"
[ -n "$url1" ] || { echo "    no address in the first server's log"; cat "$S/err1.txt"; exit 1; }
[ -f "$lease" ] || { echo "    no lease while the server runs"; exit 1; }
pid1="$(jq -r '.pid' "$lease")"
case "$pid1" in ''|*[!0-9]*) echo "    the lease records no pid: $(cat "$lease")"; exit 1 ;; esac
[ "$pid1" != "$$" ] || { echo "    the lease names this shell; refusing to kill it"; exit 1; }
kill -0 "$pid1" 2>/dev/null || { echo "    the pid the lease records ($pid1) is not a live process"; exit 1; }

# ---------------------------------------------------------------- the crash
kill -9 "$pid1" || { echo "    could not kill $pid1"; exit 1; }
# both launchers exec, so the process the lease names is this shell's own job: it is reaped
# here, and only then is it gone rather than a zombie `kill -0` would still find
exec 3>&-; rc1=0; wait "$job1" 2>/dev/null || rc1=$?
[ "$rc1" != 0 ] || { echo "    the killed server reported a clean exit"; exit 1; }
i=0; while kill -0 "$pid1" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 100 ] || { echo "    $pid1 survived SIGKILL"; exit 1; }; sleep 0.1
done

# a killed process runs no handler, so the lease it wrote is still there, naming a server
# that is not
[ -f "$lease" ] || { echo "    SIGKILL cleaned up after itself, which it cannot do"; exit 1; }
[ "$(jq -r '.pid' "$lease")" = "$pid1" ] || { echo "    the lease no longer names the killed server"; cat "$lease"; exit 1; }

# ---------------------------------------------------------------- what the repository says it is
expect_exit 0 "$RB" serve status --repo "$T"
expect_grep '^standing   stale'
expect_grep "does not answer for this checkout"
expect_grep "$url1"
"$RB" serve status --repo "$T" --format json > "$S/status.json" 2> "$S/status.err" \
  || { echo "    serve status --format json failed"; cat "$S/status.err"; exit 1; }
jq -e --arg url "$url1" --argjson pid "$pid1" '
  .standing == "stale"
  and (.this_process | not)
  and (.servers | length) >= 1
  and (.servers[] | select(.this_checkout) | .lease.url == $url and .lease.pid == $pid)
' "$S/status.json" >/dev/null || { echo "    the status does not hold the dead server's lease:"; cat "$S/status.json"; exit 1; }

# ---------------------------------------------------------------- the next client takes it over
session case113-second > "$S/session2.in"
rc=0; "$LAUNCHER" --http-port 0 < "$S/session2.in" > "$S/out2.txt" 2> "$S/err2.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the client after the crash exited $rc"; cat "$S/err2.txt"; exit 1; }
# the shape it found is named, not merely worked around
grep -q 'stale lease: the server it names at .* does not answer for this repository; taking it over' "$S/err2.txt" \
  || { echo "    the client did not say which shape of lease it found"; cat "$S/err2.txt"; exit 1; }
grep -qF "$url1" "$S/err2.txt" || { echo "    the reason does not name the dead server's address"; cat "$S/err2.txt"; exit 1; }
grep -q 'listening on http://' "$S/err2.txt" || { echo "    no server after the crash"; cat "$S/err2.txt"; exit 1; }
url2="$(sed -n 's/.*listening on \(http:\/\/127\.0\.0\.1:[0-9]*\).*/\1/p' "$S/err2.txt" | head -n 1)"
[ "$url2" != "$url1" ] || { echo "    the new server bound the dead one's address"; exit 1; }

# and the session it got is a working one: the layer answers and the board is its own
[ "$(wc -l < "$S/out2.txt" | tr -d ' ')" = 3 ] || { echo "    expected 3 frames from the client after the crash:"; cat "$S/out2.txt"; exit 1; }
sed -n 1p "$S/out2.txt" | jq -e --arg url "$url2" '.result.instructions | contains($url)' >/dev/null \
  || { echo "    the client was not told about the server it started"; sed -n 1p "$S/out2.txt"; exit 1; }
sed -n 2p "$S/out2.txt" | jq -e '.result.isError == false and .result.structuredContent.state == "ok"' >/dev/null \
  || { echo "    the layer does not answer after the crash"; sed -n 2p "$S/out2.txt"; exit 1; }
sed -n 3p "$S/out2.txt" | jq -e '.result.structuredContent.count == 1 and .result.structuredContent.peers[0].client.name == "case113-second"' >/dev/null \
  || { echo "    the board carries something other than the one client"; sed -n 3p "$S/out2.txt"; exit 1; }

# nothing of the dead server is left: its lease went with its successor's session
[ ! -f "$lease" ] || { echo "    a lease outlived the last client"; cat "$lease"; exit 1; }
expect_exit 0 "$RB" serve status --repo "$T"
expect_grep '^standing   absent'

after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    the crash and the recovery changed the repository"; git status --porcelain; exit 1; }

echo "    a killed server leaves a stale lease, the repository names it, and the next client takes it over"
