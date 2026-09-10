# majordomus-covers: capture session
# Repository entry converges: the provider's start event ensures the shared server, the
# briefing names where it stands, a second start finds the same server, and `serve stop`
# ends it.
#
# Every step drives the shim the provider would run, with the payload the provider would
# send, in a repository `init` wrote: nothing here starts a server by hand, because the
# claim is that nobody has to. The executable is the one every Rust case drives (MAJORDOMUS_BIN
# or a build of the crate); the hook finds it the way the launcher does and never builds it.
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

# --- the start event: the episode opens, a server is ensured, the briefing names it
[ ! -f "$lease" ] || { echo "    a fresh repository has a lease before anything started"; exit 1; }
out="$(printf '{"session_id":"cc-1","hook_event_name":"SessionStart","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>"$T/err")"; code=$?
[ "$code" = 0 ] || { echo "    the start shim exited $code"; cat "$T/err"; exit 1; }
printf '%s\n' "$out" | grep -q '^Shared server: ready http://' || { echo "    the briefing does not name a ready server:"; printf '%s\n' "$out"; cat "$T/err"; cat .ai/local/state/mcp/ensure.log 2>/dev/null; exit 1; }
printf '%s\n' "$out" | grep -q 'started by this call' || { echo "    the first start did not start the server"; printf '%s\n' "$out"; exit 1; }
[ -f "$lease" ] || { echo "    no lease after the start event"; exit 1; }
url="$(printf '%s\n' "$out" | sed -n 's/^Shared server: ready \(http:[^ ]*\).*/\1/p')"
[ -n "$url" ] || { echo "    no address in the briefing line"; exit 1; }
curl -fsS "$url/api/v1/server" > "$T/status.json" || { echo "    the server does not answer its status route"; exit 1; }
grep -q '"standing": "ready"' "$T/status.json" || { echo "    the server does not call itself ready"; cat "$T/status.json"; exit 1; }
grep -q '"this_process"' "$T/status.json" || { echo "    the answering server does not report the lease it holds"; exit 1; }

# --- a second start converges to the same server: the call starts nothing
out2="$(printf '{"session_id":"cc-2","hook_event_name":"SessionStart","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out2" | grep -q "^Shared server: ready $url" || { echo "    the second start did not find the same server:"; printf '%s\n' "$out2"; exit 1; }
printf '%s\n' "$out2" | grep -q 'started by this call' && { echo "    the second start started a second server"; exit 1; }

# --- status from the command line agrees, and names this checkout among the servers
expect_exit 0 "$RB" serve status --repo "$T"
expect_grep '^standing   ready'
expect_grep 'this checkout'

# --- stop ends it: the lease goes, and the next stop has nothing to do
expect_exit 0 "$RB" serve stop --repo "$T"
expect_grep '^stopped '
[ ! -f "$lease" ] || { echo "    the lease survived the stop"; exit 1; }
curl -fsS -m 2 "$url/" >/dev/null 2>&1 && { echo "    the server still answers after stop"; exit 1; }
expect_exit 0 "$RB" serve stop --repo "$T"
expect_grep 'nothing to stop'

# --- the switch: off means the briefing says so and nothing is started
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
out3="$(printf '{"session_id":"cc-3","hook_event_name":"SessionStart","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out3" | grep -q '^Shared server: not ensured: session.ensure_server_on_start is false' || { echo "    the switch is not named:"; printf '%s\n' "$out3"; exit 1; }
[ ! -f "$lease" ] || { echo "    a server was started with the switch off"; exit 1; }

# --- and an executable that is not there is named, never built, and the event still exits 0
cp "$T/pol" .ai/repo/policy.yaml
sed 's/^  ensure_server_on_start: false /  ensure_server_on_start: true /' .ai/repo/policy.yaml > "$T/pol2" && cp "$T/pol2" .ai/repo/policy.yaml
out4="$(printf '{"session_id":"cc-4","hook_event_name":"SessionStart","source":"startup"}' | MAJORDOMUS_BIN="$T/nowhere" ./.claude/hooks/majordomus-session-start 2>/dev/null)"; code=$?
[ "$code" = 0 ] || { echo "    a missing executable failed the start event ($code)"; exit 1; }
printf '%s\n' "$out4" | grep -q '^Shared server: not ensured: the executable is not built' || { echo "    a missing executable is not named:"; printf '%s\n' "$out4"; exit 1; }
[ ! -f "$lease" ] || { echo "    a server appeared without an executable"; exit 1; }

echo "    entering through the provider's start event converges on one ready server, names it, and stop ends it"
