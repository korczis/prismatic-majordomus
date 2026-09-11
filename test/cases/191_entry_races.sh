# majordomus-exclusive: starts and races real servers; a busy pool changes what it measures
# majordomus-covers: none
# majordomus-timeout: 900
#
# Entry converges on ONE runtime, whatever else is happening at the same moment.
#
# ADR 0043 let a shell entering the repository bring the runtime up, and the cost it accepted
# is that a `cd` can now start a process. What makes that safe is not the entry path — it is
# the election, one `O_EXCL` create of the lease file, which was already there and which
# `serve ensure` already used. This case is the proof that entry reaches it: that ten shells
# entering a cold checkout at the same instant end with one authoritative server and not two,
# and that every shape of broken lease a previous server can leave behind converges as well.
#
# What is proved elsewhere and is deliberately not repeated: that a SIGKILLed server leaves a
# stale lease the next client takes over is test/cases/113; that two worktrees each carry
# their own server is test/cases/114; that the election survives a storm of MCP clients is
# apps/majordomus-cli/tests/mcp_shared.rs and test/cases/90. What is new here is the entry
# path as the thing doing the racing, and the row nothing covered: a port taken by something
# that is not a server of ours.
#
# Nothing in this case kills anything by pattern. Several sessions share this machine and run
# servers of their own; a `pkill -f` here would take theirs. Every process it ends is ended by
# a pid it read out of its own checkout's lease, or by a pid whose command line names this
# case's own temporary directory, which no other process on the machine can carry.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v python3 >/dev/null 2>&1 || { echo "    skip: python3 not installed"; exit 0; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=error; export MAJORDOMUS_LOG
lease=.ai/local/state/mcp/server.json
TT="$(cd "$T" && pwd -P)"

# Every server whose command line names THIS case's repository, and nothing else on the
# machine. The `--repo <path>` that `spawn_server` passes is what makes them findable, and
# the path is a mktemp directory this run owns.
our_servers() { ps -ax -o pid=,command= 2>/dev/null | grep -F -- "serve --repo $TT" | grep -v grep || true; }
our_server_count() { our_servers | grep -c . | tr -d ' '; }
stop_ours() {
  "$RB" serve stop --repo "$T" >/dev/null 2>&1 || true
  local p
  for p in $(our_servers | awk '{print $1}'); do
    case "$p" in ''|*[!0-9]*) continue ;; esac
    [ "$p" = "$$" ] && continue
    kill "$p" 2>/dev/null || true
  done
  rm -f "$lease"
}
trap stop_ours EXIT

enter() { "$RB" env enter --repo "$T" --shell direnv --no-banner --no-bridge; }

# Bounded, with a predicate and a recovery path (project.every-wait-is-bounded): wait until
# this checkout's server is ready, or say what stood instead.
await_ready() {
  local i=0
  while [ "$i" -lt 150 ]; do
    "$RB" serve status --repo "$T" --checkouts this 2>/dev/null | grep -q '^standing   ready' && return 0
    i=$((i+1)); sleep 0.2
  done
  echo "    no server became ready:"; "$RB" serve status --repo "$T" --checkouts this 2>&1 | sed 's/^/    /'
  sed 's/^/    | /' .ai/local/state/mcp/server.log 2>/dev/null | tail -n 20
  return 1
}

# The one assertion every row ends with: one lease, one ready server, one process.
converged_on_one() {
  local what="$1"
  await_ready || { echo "    ($what)"; return 1; }
  [ -f "$lease" ] || { echo "    $what: no lease after convergence"; return 1; }
  local n; n="$(our_server_count)"
  [ "$n" = 1 ] || { echo "    $what: $n server processes for one checkout, not 1:"; our_servers | sed 's/^/    | /'; return 1; }
  local pid; pid="$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")"
  kill -0 "$pid" 2>/dev/null || { echo "    $what: the lease names pid $pid, which is not a live process"; return 1; }
  our_servers | awk '{print $1}' | grep -qx "$pid" || { echo "    $what: the one live server is not the one the lease names ($pid):"; our_servers | sed 's/^/    | /'; return 1; }
  return 0
}

# ---------------------------------------------------------------- 1. two terminals at once
stop_ours
( enter >/dev/null 2>&1 ) & a=$!
( enter >/dev/null 2>&1 ) & b=$!
wait "$a"; ra=$?; wait "$b"; rb=$?
[ "$ra" = 0 ] && [ "$rb" = 0 ] || { echo "    two simultaneous entries exited $ra/$rb; entering a directory must not fail"; exit 1; }
converged_on_one "two at once" || exit 1
url1="$("$RB" serve status --repo "$T" --checkouts this 2>/dev/null | sed -n 's/^this  *\(http:[^ ]*\).*/\1/p')"
echo "    two at once: one server at $url1"

# ---------------------------------------------------------------- 2. a tenth enters while one serves
# The row that matters most for a machine several people work on: entering a repository whose
# runtime is already up must be a read, not a restart, however many do it at once.
pid_before="$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")"
i=0; pids=""
while [ "$i" -lt 10 ]; do ( enter >/dev/null 2>&1 ) & pids="$pids $!"; i=$((i+1)); done
bad=0; for p in $pids; do wait "$p" || bad=$((bad+1)); done
[ "$bad" = 0 ] || { echo "    $bad of ten simultaneous warm entries failed"; exit 1; }
converged_on_one "ten warm at once" || exit 1
[ "$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")" = "$pid_before" ] \
  || { echo "    ten simultaneous warm entries restarted the server"; exit 1; }
echo "    ten warm at once: the same server, never restarted"

# ---------------------------------------------------------------- 3. ten enter a cold checkout at once
# The expensive row. Each of the ten may find `absent` before any of them has written a lease
# and start a server of its own; the election lets one bind and the rest exit 0 within
# milliseconds. What must be true when the dust settles is one lease and one process.
stop_ours
i=0; pids=""
while [ "$i" -lt 10 ]; do ( enter >/dev/null 2>&1 ) & pids="$pids $!"; i=$((i+1)); done
bad=0; for p in $pids; do wait "$p" || bad=$((bad+1)); done
[ "$bad" = 0 ] || { echo "    $bad of ten simultaneous cold entries failed"; exit 1; }
await_ready || exit 1
# the losers exit on their own; give them a bounded moment to, then require exactly one
i=0; while [ "$i" -lt 100 ] && [ "$(our_server_count)" != 1 ]; do i=$((i+1)); sleep 0.2; done
converged_on_one "ten cold at once" || exit 1
echo "    ten cold at once: one lease, one listening server, the rest stood down"

# ---------------------------------------------------------------- 4a. two agents at once
# Two provider start events reaching the election in the same instant, which is what happens
# when somebody opens two Claude Code sessions in one checkout. Both must find or make the
# same one server, and the second must not start a second.
stop_ours
"$MJ" capture install >/dev/null 2>&1 || true
if [ -x ./.claude/hooks/majordomus-session-start ]; then
  PATH="$(dirname "$MJ"):$PATH"; export PATH
  MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
  ( printf '{"session_id":"agent-a","hook_event_name":"SessionStart","source":"startup"}' \
      | ./.claude/hooks/majordomus-session-start >"$T/agent-a.out" 2>/dev/null ) & a=$!
  ( printf '{"session_id":"agent-b","hook_event_name":"SessionStart","source":"startup"}' \
      | ./.claude/hooks/majordomus-session-start >"$T/agent-b.out" 2>/dev/null ) & b=$!
  wait "$a" || true; wait "$b" || true
  converged_on_one "two agents at once" || exit 1
  # exactly one of the two says it started the server; the other found it
  started="$(grep -l 'started by this call' "$T/agent-a.out" "$T/agent-b.out" 2>/dev/null | wc -l | tr -d ' ')"
  [ "$started" -le 1 ] || { echo "    both start events claim to have started the server:"; cat "$T/agent-a.out" "$T/agent-b.out"; exit 1; }
  echo "    two agents at once: one server, and at most one of them started it"
else
  echo "    (no session shim installed here; the two-agent race is covered by case 108)"
fi

# ---------------------------------------------------------------- 4b. an agent and a shell at once
# The cross-door race ADR 0043 created: the provider's start event and a shell entry reaching
# the same election from two different programs. Both must find or make the same one server.
stop_ours
"$MJ" capture install >/dev/null 2>&1 || true
if [ -x ./.claude/hooks/majordomus-session-start ]; then
  ( printf '{"session_id":"race-a","hook_event_name":"SessionStart","source":"startup"}' \
      | MAJORDOMUS_BIN="$RB" PATH="$(dirname "$MJ"):$PATH" ./.claude/hooks/majordomus-session-start >/dev/null 2>&1 ) & a=$!
  ( enter >/dev/null 2>&1 ) & b=$!
  wait "$a" || true; wait "$b" || { echo "    the shell entry failed while an agent was starting"; exit 1; }
  converged_on_one "an agent and a shell at once" || exit 1
  echo "    an agent and a shell at once: one server, reached through one election"
else
  echo "    (no session shim installed here; the cross-door race is covered by case 108)"
fi

# ---------------------------------------------------------------- 5. a lease naming a dead pid
stop_ours
dead=99999
while kill -0 "$dead" 2>/dev/null; do dead=$((dead - 1)); done
mkdir -p "$(dirname "$lease")"
printf '{"schema":"majordomus-mcp-lease/v1","pid":%s,"token":"t","root":"%s","url":"http://127.0.0.1:1","started_at":"2026-01-01T00:00:00Z"}\n' \
  "$dead" "$TT" > "$lease"
enter >/dev/null 2>"$T/dead.err" || { echo "    entry failed on a lease with a dead pid"; cat "$T/dead.err"; exit 1; }
converged_on_one "a dead pid in the lease" || exit 1
[ "$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")" != "$dead" ] || { echo "    the dead pid's lease was never taken over"; exit 1; }
echo "    a lease naming a dead pid: taken over, one server"

# ---------------------------------------------------------------- 6. a live process that is not our server
# The nastier half of the same shape: the address in the lease answers, so a liveness check
# that only opened a socket would call it healthy. The probe asks whether it answers *for this
# checkout*, which is why this converges instead of attaching to a stranger.
stop_ours
python3 - "$T/port" <<'PY' &
import http.server, socketserver, sys, threading
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.end_headers(); self.wfile.write(b"not a majordomus server")
    def log_message(self, *a): pass
s = socketserver.TCPServer(("127.0.0.1", 0), H)
open(sys.argv[1], "w").write(str(s.server_address[1]))
s.serve_forever()
PY
stranger=$!
i=0; while [ "$i" -lt 100 ] && [ ! -s "$T/port" ]; do i=$((i+1)); sleep 0.1; done
if [ -s "$T/port" ]; then
  sport="$(cat "$T/port")"
  mkdir -p "$(dirname "$lease")"
  printf '{"schema":"majordomus-mcp-lease/v1","pid":%s,"token":"t","root":"%s","url":"http://127.0.0.1:%s","started_at":"2026-01-01T00:00:00Z"}\n' \
    "$stranger" "$TT" "$sport" > "$lease"
  enter >/dev/null 2>"$T/unhealthy.err" || { echo "    entry failed on a lease naming a live stranger"; cat "$T/unhealthy.err"; exit 1; }
  converged_on_one "a live pid that is not our server" || exit 1
  our_url="$("$RB" serve status --repo "$T" --checkouts this 2>/dev/null | sed -n 's/^this  *\(http:[^ ]*\).*/\1/p')"
  [ "$our_url" != "http://127.0.0.1:$sport" ] || { echo "    entry attached to a process that is not a server of this checkout"; exit 1; }
  kill "$stranger" 2>/dev/null || true
  echo "    a lease naming a live stranger: not attached to, taken over"
else
  kill "$stranger" 2>/dev/null || true
  echo "    (the stranger never bound; skipping that row)"
fi

# ---------------------------------------------------------------- 7. a server that never binds
# A crash during startup, seen from the lease it left: a document with no address, older than
# the bind grace. The mtime is moved rather than the case sleeping for the grace, because a
# case that waits fifteen seconds to learn one thing is a case somebody eventually deletes.
stop_ours
mkdir -p "$(dirname "$lease")"
printf '{"schema":"majordomus-mcp-lease/v1","pid":%s,"token":"t","root":"%s","url":null,"started_at":"2026-01-01T00:00:00Z"}\n' \
  "$dead" "$TT" > "$lease"
python3 -c 'import os,sys,time; t=time.time()-600; os.utime(sys.argv[1],(t,t))' "$lease"
enter >/dev/null 2>"$T/abandoned.err" || { echo "    entry failed on an abandoned lease"; cat "$T/abandoned.err"; exit 1; }
converged_on_one "a server that never bound" || exit 1
echo "    an abandoned lease with no address: taken over, one server"

# ---------------------------------------------------------------- 8. a port taken by a stranger
# Entry asks for one port and accepts another; nothing about that is entry's own decision —
# `spawn_server` passes `--fallback`, and this is the same `converge` the entry path calls,
# reached through the command line so that the taken port can be named.
stop_ours
python3 - "$T/port2" <<'PY' &
import socket, sys, time
s = socket.socket(); s.bind(("127.0.0.1", 0)); s.listen(16)
open(sys.argv[1], "w").write(str(s.getsockname()[1]))
time.sleep(300)
PY
squatter=$!
i=0; while [ "$i" -lt 100 ] && [ ! -s "$T/port2" ]; do i=$((i+1)); sleep 0.1; done
if [ -s "$T/port2" ]; then
  taken="$(cat "$T/port2")"
  "$RB" serve ensure --repo "$T" --port "$taken" --idle 60 --wait 40 > "$T/taken.out" 2>&1 \
    || { echo "    a taken port stopped the convergence:"; cat "$T/taken.out"; kill "$squatter" 2>/dev/null; exit 1; }
  grep -q '^ready ' "$T/taken.out" || { echo "    a taken port left no ready server:"; cat "$T/taken.out"; kill "$squatter" 2>/dev/null; exit 1; }
  grep -q ":$taken " "$T/taken.out" && { echo "    the server bound a port something else holds"; kill "$squatter" 2>/dev/null; exit 1; }
  converged_on_one "a taken port" || { kill "$squatter" 2>/dev/null; exit 1; }
  kill "$squatter" 2>/dev/null || true
  echo "    a port held by a stranger: a free one bound instead, one server"
else
  kill "$squatter" 2>/dev/null || true
  echo "    (the squatter never bound; skipping that row)"
fi

# ---------------------------------------------------------------- 9. and the repository is untouched
stop_ours
[ -z "$(git status --porcelain -- .ai/repo lib)" ] || { echo "    racing entries wrote into the repository:"; git status --porcelain; exit 1; }

echo "    every entry race converges on exactly one authoritative server, and never on two"
