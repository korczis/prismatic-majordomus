# majordomus-covers: capture session
# Entry converges for a client that is not Claude Code.
#
# Three entries, one server. A provider whose start event is documented enters through the
# shim `capture install` wrote for it, with the payload that provider's own schema publishes;
# a client with no event of its own enters through the launcher its client configuration
# names, which is the one thing every MCP client of this repository genuinely runs. Each of
# them ensures the shared server, opens or resumes the episode, and the second of them
# starts nothing.
#
# Nothing here starts a server by hand, and nothing drives Claude Code's shim: that path is
# case 108's, and this case exists because it was the only one that worked.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || { echo "    curl absent; skipping"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
lease=.ai/local/state/mcp/server.json

# --- the adapters are data, and every provider with one is wired here
"$MJ" capture status > "$T/status.txt" 2>&1 || { echo "    capture status failed"; cat "$T/status.txt"; exit 1; }
for p in codex gemini; do
  grep -qE "^$p:session +(wired|verified) " "$T/status.txt" \
    || { echo "    $p's lifecycle is not wired after capture install:"; cat "$T/status.txt"; exit 1; }
  grep -qE "^$p +unsupported " "$T/status.txt" \
    || { echo "    $p's prompt aspect claims an adapter it has no documented event for:"; cat "$T/status.txt"; exit 1; }
done

# --- a documented start event: Gemini CLI's, with the payload its hook reference publishes
[ ! -f "$lease" ] || { echo "    a fresh repository has a lease before anything started"; exit 1; }
gem='{"session_id":"gem-1","transcript_path":"","cwd":"'"$T"'","hook_event_name":"SessionStart","timestamp":"2026-09-10T00:00:00Z","source":"startup"}'
out="$(printf '%s' "$gem" | ./.gemini/hooks/majordomus-session-start 2>"$T/err")"; code=$?
[ "$code" = 0 ] || { echo "    the gemini start shim exited $code"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' \
  || { echo "    the briefing does not name a ready server:"; printf '%s\n' "$out"; cat "$T/err"; cat .ai/local/state/mcp/ensure.log 2>/dev/null; exit 1; }
printf '%s\n' "$out" | grep -q 'started by this call' || { echo "    the first entry did not start the server"; printf '%s\n' "$out"; exit 1; }
[ -f "$lease" ] || { echo "    no lease after a gemini start event"; exit 1; }
grep -q 'provider: "gemini"' .ai/local/state/sessions-open/*.yaml \
  || { echo "    no episode was opened for gemini"; ls .ai/local/state/sessions-open/ 2>/dev/null; exit 1; }
grep -q 'provider_session: "gem-1"' .ai/local/state/sessions-open/*.yaml \
  || { echo "    the episode does not carry the provider session from the payload"; exit 1; }
url="$(printf '%s\n' "$out" | sed -n 's/^Shared server: ready \(http:[^ ]*\).*/\1/p')"
[ -n "$url" ] || { echo "    no address in the briefing line"; exit 1; }
curl -fsS -m 10 "$url/api/v1/server" > "$T/server.json" || { echo "    the server does not answer its status route"; exit 1; }
grep -q '"standing": "ready"' "$T/server.json" || { echo "    the server does not call itself ready"; cat "$T/server.json"; exit 1; }

# --- the same provider session again: the episode is kept and no second server is started
out2="$(printf '%s' "$gem" | ./.gemini/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out2" | grep -q "^Shared server: ready $url" \
  || { echo "    the second entry did not find the same server:"; printf '%s\n' "$out2"; exit 1; }
printf '%s\n' "$out2" | grep -q 'started by this call' && { echo "    the second entry started a second server"; exit 1; }
[ "$(ls .ai/local/state/sessions-open/*.yaml | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    the second entry opened a second episode"; ls .ai/local/state/sessions-open/; exit 1; }

# --- Codex's start event, with the payload its generated schema publishes
cdx='{"session_id":"cdx-1","transcript_path":null,"cwd":"'"$T"'","hook_event_name":"SessionStart","model":"a-model","permission_mode":"default","source":"startup"}'
out3="$(printf '%s' "$cdx" | ./.codex/hooks/majordomus-session-start 2>/dev/null)"; code=$?
[ "$code" = 0 ] || { echo "    the codex start shim exited $code"; exit 1; }
printf '%s\n' "$out3" | grep -q "^Shared server: ready $url" \
  || { echo "    codex's entry did not converge on the running server:"; printf '%s\n' "$out3"; exit 1; }
grep -q 'provider: "codex"' .ai/local/state/sessions-open/*.yaml \
  || { echo "    no episode was opened for codex"; exit 1; }

# --- the launcher: entry for a client that has no start event at all
#
# The real file, driven where a client would start it: a checkout of its own, the two
# launchers it reaches for beside it, and the repository named the way a client's
# configuration names it. `--help` is what makes it bounded — the entry runs, then the
# server prints its usage and exits, so nothing is left holding stdin.
mkdir -p bin
cp "$ROOT/bin/majordomus-mcp" bin/majordomus-mcp
printf '#!/bin/sh\nexec "%s" "$@"\n' "$MJ" > bin/majordomus
printf '#!/bin/sh\nexec "%s" "$@"\n' "$RB" > bin/majordomus-cli
chmod +x bin/majordomus bin/majordomus-cli
"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true
[ ! -f "$lease" ] || { echo "    the lease survived serve stop"; exit 1; }

out4="$(./bin/majordomus-mcp --provider codex --repo "$T" --help 2>"$T/err4")"; code=$?
[ "$code" = 0 ] || { echo "    the launcher exited $code"; cat "$T/err4"; exit 1; }
printf '%s\n' "$out4" | grep -qi 'usage' || { echo "    the launcher did not reach the server's own command:"; printf '%s\n' "$out4"; exit 1; }
printf '%s\n' "$out4" | grep -q 'Shared server:' && { echo "    the entry wrote the briefing to stdout, which belongs to the protocol"; exit 1; }
grep -q '^Shared server: ready http://' "$T/err4" \
  || { echo "    the launcher's entry did not ensure a server:"; cat "$T/err4"; exit 1; }
[ -f "$lease" ] || { echo "    the launcher's entry left no lease"; exit 1; }
url2="$(sed -n 's/^Shared server: ready \(http:[^ ]*\).*/\1/p' "$T/err4" | head -n 1)"
curl -fsS -m 10 "$url2/api/v1/server" > "$T/server2.json" || { echo "    the launcher's server does not answer"; exit 1; }
grep -q '"standing": "ready"' "$T/server2.json" || { echo "    the launcher's server does not call itself ready"; cat "$T/server2.json"; exit 1; }

# --- and the launcher's second entry converges: the same server, no second episode
before="$(ls .ai/local/state/sessions-open/*.yaml | wc -l | tr -d ' ')"
./bin/majordomus-mcp --provider gemini --repo "$T" --help >/dev/null 2>"$T/err5" || { echo "    the second launcher entry failed"; cat "$T/err5"; exit 1; }
grep -q "^Shared server: ready $url2" "$T/err5" \
  || { echo "    the second launcher entry did not find the same server:"; cat "$T/err5"; exit 1; }
grep -q 'started by this call' "$T/err5" && { echo "    the second launcher entry started a second server"; exit 1; }
[ "$(ls .ai/local/state/sessions-open/*.yaml | wc -l | tr -d ' ')" = "$before" ] \
  || { echo "    the second launcher entry opened an episode of its own"; ls .ai/local/state/sessions-open/; exit 1; }

# --- an MCP client attached through the launcher lands on the peer board
#
# The board is the serving process's memory of who is attached now, so it has to be read
# while the client still is. The connection is held open by a descriptor of this shell's
# rather than by a sleep, every wait is bounded by a counter, and closing the descriptor is
# what ends the client: nothing is left running behind this case.
init='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"a-client","version":"0"}}}'
mkfifo "$T/mcp.in"
./bin/majordomus-mcp --provider gemini --repo "$T" < "$T/mcp.in" > "$T/mcp.out" 2>/dev/null &
client=$!
exec 9> "$T/mcp.in"
printf '%s\n' "$init" >&9
waited=0
while [ "$waited" -lt 40 ] && ! grep -q '"serverInfo"' "$T/mcp.out" 2>/dev/null; do sleep 1; waited=$((waited+1)); done
if ! grep -q '"serverInfo"' "$T/mcp.out" 2>/dev/null; then
  exec 9>&-; kill "$client" 2>/dev/null || true; wait "$client" 2>/dev/null || true
  echo "    the launcher's client never completed initialize"; cat "$T/mcp.out"; exit 1
fi
curl -fsS -m 10 "$url2/api/v1/peers" > "$T/peers.json" || cp /dev/null "$T/peers.json"
exec 9>&-
waited=0
while [ "$waited" -lt 20 ] && kill -0 "$client" 2>/dev/null; do sleep 1; waited=$((waited+1)); done
kill "$client" 2>/dev/null || true
wait "$client" 2>/dev/null || true
grep -q 'a-client' "$T/peers.json" \
  || { echo "    the client that entered through the launcher is not on the peer board"; cat "$T/peers.json"; exit 1; }

# --- the switch turns the launcher's half off exactly as it turns the hook's off
"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
./bin/majordomus-mcp --provider codex --repo "$T" --help >/dev/null 2>"$T/err6" || { echo "    the launcher failed with the switch off"; cat "$T/err6"; exit 1; }
grep -q '^Shared server: not ensured: session.ensure_server_on_start is false' "$T/err6" \
  || { echo "    the switch is not named on the launcher's path:"; cat "$T/err6"; exit 1; }
[ ! -f "$lease" ] || { echo "    a server was started with the switch off"; exit 1; }

echo "    a client that is not Claude Code enters through its own start event and through the launcher, and converges on one server"
