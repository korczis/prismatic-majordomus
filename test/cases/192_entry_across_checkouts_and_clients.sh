# majordomus-exclusive: starts servers in several checkouts at once; a busy pool changes what it measures
# majordomus-covers: none
# majordomus-timeout: 900
#
# The two rows of the entry race matrix that are not about one checkout racing itself.
#
#   A. two linked worktrees of one repository entering at the SAME moment. Each is its own
#      checkout and is owed its own server (ADR 0035); what must never happen is that they
#      end up sharing one, or that one of them ends up with two. test/cases/114 proves the
#      topology with `serve ensure` run one after the other; this is the same claim with the
#      entry path doing it, simultaneously, which is how it actually happens on a machine
#      where somebody opens two terminals.
#   B. one checkout, ten MCP clients attaching at once, on a runtime a shell entry brought
#      up. ADR 0003's election has always converged for clients; what is new since ADR 0043
#      is that the server they find may have been started by a `cd` rather than by the first
#      of them, and that a shell entering beside them must still start nothing.
#
# Nothing here is stopped by pattern: every server is ended through the lease of its own
# checkout. Several sessions share this machine and run servers of their own.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }

# The repository is nested one level down: its worktree container is by definition its
# sibling, so a repository at $T would leave a container beside $T. Paths are resolved,
# because git holds the resolved path and macOS's $TMPDIR is a symlink.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
( cd "$R" && "$MJ" init >/dev/null && "$MJ" update >/dev/null )
git -C "$R" add -A >/dev/null && git -C "$R" commit -qm base >/dev/null
trunk="$(git -C "$R" symbolic-ref --short HEAD)"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
MAJORDOMUS_LOG=error; export MAJORDOMUS_LOG

A="$R-wt/feature/alpha"; B="$R-wt/feature/beta"
trap '"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$B" >/dev/null 2>&1 || true; "$RB" serve stop --repo "$R" >/dev/null 2>&1 || true' EXIT

enter() { "$RB" env enter --repo "$1" --shell direnv --no-banner --no-bridge; }
await_ready() {   # bounded, with a predicate and something to say when it does not happen
  local repo="$1" i=0
  while [ "$i" -lt 150 ]; do
    "$RB" serve status --repo "$repo" --checkouts this 2>/dev/null | grep -q '^standing   ready' && return 0
    i=$((i+1)); sleep 0.2
  done
  echo "    no server became ready in $repo:"; "$RB" serve status --repo "$repo" --checkouts this 2>&1 | sed 's/^/    /'
  sed 's/^/    | /' "$repo/.ai/local/state/mcp/server.log" 2>/dev/null | tail -n 20
  return 1
}
url_of() { "$RB" serve status --repo "$1" --checkouts this 2>/dev/null | sed -n 's/^this  *\(http:[^ ]*\).*/\1/p'; }

# ---------------------------------------------------------------- A. two worktrees at once
"$RB" worktree create feature/alpha --repo "$R" --base "$trunk" >/dev/null 2>&1 \
  || ( cd "$R" && "$RB" worktree create feature/alpha --base "$trunk" >/dev/null )
( cd "$R" && "$RB" worktree create feature/beta --base "$trunk" >/dev/null )
[ -d "$A" ] && [ -d "$B" ] || { echo "    the worktrees are not at the derived paths"; exit 1; }

( enter "$A" >/dev/null 2>&1 ) & pa=$!
( enter "$B" >/dev/null 2>&1 ) & pb=$!
wait "$pa"; ra=$?; wait "$pb"; rb=$?
[ "$ra" = 0 ] && [ "$rb" = 0 ] || { echo "    two worktrees entering at once exited $ra/$rb"; exit 1; }
await_ready "$A" || exit 1
await_ready "$B" || exit 1
ua="$(url_of "$A")"; ub="$(url_of "$B")"
[ -n "$ua" ] && [ -n "$ub" ] || { echo "    a worktree has no address after entering ($ua / $ub)"; exit 1; }
[ "$ua" != "$ub" ] || { echo "    two worktrees ended up sharing one server at $ua"; exit 1; }
# one server each, not two, and not one between them
for w in "$A" "$B"; do
  n="$(ps -ax -o pid=,command= 2>/dev/null | grep -F -- "serve --repo $w" | grep -vc grep || true)"
  [ "$n" = 1 ] || { echo "    $w has $n server processes, not 1:"; ps -ax -o pid=,command= | grep -F -- "serve --repo $w" | grep -v grep | sed 's/^/    | /'; exit 1; }
done
# and they know they are one repository, which is what makes a peer board worth reading
"$RB" serve status --repo "$A" --format json > "$T/a.json"
"$RB" serve status --repo "$B" --format json > "$T/b.json"
[ "$(jq -r '.git.id' "$T/a.json")" = "$(jq -r '.git.id' "$T/b.json")" ] \
  || { echo "    two worktrees entering at once do not agree which repository they are"; exit 1; }
jq -e --arg u "$ub" '.servers[] | select(.branch == "feature/beta") | .standing == "ready" and .lease.url == $u' "$T/a.json" >/dev/null \
  || { echo "    alpha does not see the server beta's entry started:"; cat "$T/a.json"; exit 1; }
echo "    two worktrees entering at once: one server each, distinct addresses, one repository"

# a second simultaneous pair starts nothing: entering a checkout whose runtime is up is a read
pid_a="$(jq -r '.this_process.pid' "$T/a.json")"; pid_b="$(jq -r '.this_process.pid' "$T/b.json")"
( enter "$A" >/dev/null 2>&1 ) & pa=$!
( enter "$B" >/dev/null 2>&1 ) & pb=$!
wait "$pa" || { echo "    a warm entry in alpha failed"; exit 1; }
wait "$pb" || { echo "    a warm entry in beta failed"; exit 1; }
[ "$(jq -r '.this_process.pid' <("$RB" serve status --repo "$A" --format json))" = "$pid_a" ] \
  || { echo "    entering alpha again restarted its server"; exit 1; }
[ "$(jq -r '.this_process.pid' <("$RB" serve status --repo "$B" --format json))" = "$pid_b" ] \
  || { echo "    entering beta again restarted its server"; exit 1; }
"$RB" serve stop --repo "$B" >/dev/null 2>&1 || true
echo "    entering both again: neither server restarted"

# ---------------------------------------------------------------- B. ten clients, one runtime
# The runtime under them is the one alpha's *entry* started, not one the first client made:
# that is the part ADR 0043 added, and the part that would break silently if entry and the
# MCP election ever stopped agreeing about which lease they are reading.
LAUNCHER="$ROOT/bin/majordomus-mcp"
if [ ! -x "$LAUNCHER" ]; then
  echo "    (no launcher; the ten-client row needs bin/majordomus-mcp)"
else
  S="$T/clients"; mkdir -p "$S"
  frames() {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"%s","version":"0"}}}\n' "$1"
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_peers","arguments":{}}}\n'
  }
  i=0; pids=""
  while [ "$i" -lt 10 ]; do
    frames "case192-$i" > "$S/in.$i"
    ( cd "$A" && "$LAUNCHER" < "$S/in.$i" > "$S/out.$i" 2> "$S/err.$i" ) & pids="$pids $!"
    i=$((i+1))
  done
  bad=0; for p in $pids; do wait "$p" || bad=$((bad+1)); done
  [ "$bad" = 0 ] || { echo "    $bad of ten clients failed:"; head -n 5 "$S"/err.* | sed 's/^/    | /'; exit 1; }

  # every one of them was told the same address, and it is the one entry started
  seen=""
  i=0; while [ "$i" -lt 10 ]; do
    u="$(sed -n 1p "$S/out.$i" | jq -r '.result.instructions // ""' | sed -n 's|.*\(http://127\.0\.0\.1:[0-9]*\).*|\1|p' | head -n 1)"
    [ -n "$u" ] || { echo "    client $i was told about no server:"; sed -n 1p "$S/out.$i"; exit 1; }
    case "$seen" in "") seen="$u" ;; *) [ "$u" = "$seen" ] || { echo "    two clients were told about different servers: $seen / $u"; exit 1; } ;; esac
    i=$((i+1))
  done
  [ "$seen" = "$ua" ] || { echo "    ten clients found $seen, not the server entering the worktree started ($ua)"; exit 1; }
  n="$(ps -ax -o pid=,command= 2>/dev/null | grep -F -- "serve --repo $A" | grep -vc grep || true)"
  [ "$n" = 1 ] || { echo "    ten clients left $n server processes, not 1"; exit 1; }

  # and a shell entering beside them starts nothing
  pid_now="$(jq -r '.this_process.pid' <("$RB" serve status --repo "$A" --format json))"
  enter "$A" >/dev/null 2>"$T/beside.err" || { echo "    entering beside ten clients failed"; cat "$T/beside.err"; exit 1; }
  [ -s "$T/beside.err" ] && { echo "    entering beside ten clients said something:"; cat "$T/beside.err"; exit 1; }
  [ "$(jq -r '.this_process.pid' <("$RB" serve status --repo "$A" --format json))" = "$pid_now" ] \
    || { echo "    entering beside ten clients restarted the server"; exit 1; }
  echo "    ten clients on the runtime an entry started: all told the same address, one server, and entering beside them starts nothing"
fi

"$RB" serve stop --repo "$A" >/dev/null 2>&1 || true
echo "    entry converges per checkout, not per repository, and a runtime a shell started serves every client of it"
