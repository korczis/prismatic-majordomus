# majordomus-covers: none
# The interface a local server binds is the machine's to name.
#
# A server on a person's machine binds loopback, and case 85 holds that default. What was
# missing is a way for one machine to say otherwise that survives the server: on 2026-10-07
# a server was restarted by hand with `--host 0.0.0.0` so that a second machine could reach
# it, and `serve status` went on reporting `desired 127.0.0.1` — the next session start, or
# the next idle stop, would have put it back on loopback without a word, because
# `serve ensure` starts a server with no host at all and no caller could give it one.
#
# A tracked setting is the wrong place: this repository is public, and a clone would listen
# on whatever network it woke up in. A flag is the wrong place too: the hook, `serve ensure`
# and every client's `majordomus mcp` each start a server, and each would have to repeat it.
# `MAJORDOMUS_HTTP_HOST` is inherited by all of them and set by none.
#
# What is held here: without the variable the bind is loopback; with it, `serve`, the server
# `serve ensure` starts and the one `majordomus mcp` elects all bind what it names; the
# command line still wins; a blank value is no value; the undeclared-bind warning is not
# silenced by it; `server.status` calls desired the address a bind would take; and a value
# that cannot be bound fails naming the address.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj992.XXXXXX")"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST
lease=.ai/local/state/mcp/server.json
pid=""
cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  if [ -f "$lease" ]; then
    p="$(jq -r '.pid // empty' "$lease" 2>/dev/null || true)"
    case "$p" in ''|*[!0-9]*) ;; *) kill "$p" 2>/dev/null || true ;; esac
  fi
  rm -rf "$S"
}
trap cleanup EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm base >/dev/null

# Start `serve` with the arguments given, wait for it to say where it listens, stop it, and
# leave its log in $S/$1.log. The environment is the caller's.
served() {   # name [serve arguments...]
  local name="$1" n=0; shift
  "$RB" serve --port 0 --idle 60 "$@" </dev/null >"$S/$name.log" 2>&1 &
  pid=$!
  while [ $n -lt 150 ] && ! grep -q 'shared server listening' "$S/$name.log" 2>/dev/null; do
    n=$((n+1)); sleep 0.1
  done
  kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; pid=""
  if ! grep -q 'shared server listening' "$S/$name.log"; then
    echo "    serve ($name) never listened:"; cat "$S/$name.log"; exit 1
  fi
}

# ---------------------------------------------------------------- nothing said: loopback
served default
if ! grep -q 'listening on http://127\.0\.0\.1:' "$S/default.log"; then
  echo "    with no variable and no flag the bind was not loopback:"; cat "$S/default.log"; exit 1
fi
if grep -q 'MAJORDOMUS_HTTP_HOST' "$S/default.log"; then
  echo "    the variable was named in a run that did not set it"; exit 1
fi

# ---------------------------------------------------------------- the machine says: all
MAJORDOMUS_HTTP_HOST=0.0.0.0 served named
if ! grep -q 'listening on http://0\.0\.0\.0:' "$S/named.log"; then
  echo "    the variable named 0.0.0.0 and the bind did not follow:"; cat "$S/named.log"; exit 1
fi
# it says where the address came from: a bind nobody can trace is the one that surprises
if ! grep -q 'MAJORDOMUS_HTTP_HOST names it' "$S/named.log"; then
  echo "    the bind did not say the variable named it:"; cat "$S/named.log"; exit 1
fi
# and the warning stands: the variable chooses the interface, it does not declare the
# exposure the way a deployment object does
if ! grep -q 'bind 127.0.0.1 unless that is intended' "$S/named.log"; then
  echo "    the variable silenced the undeclared-bind warning:"; cat "$S/named.log"; exit 1
fi

# ---------------------------------------------------------------- the command line wins
MAJORDOMUS_HTTP_HOST=0.0.0.0 served flag --host 127.0.0.1
if ! grep -q 'listening on http://127\.0\.0\.1:' "$S/flag.log"; then
  echo "    --host did not win over the variable:"; cat "$S/flag.log"; exit 1
fi

# ---------------------------------------------------------------- blank is unset
MAJORDOMUS_HTTP_HOST="  " served blank
if ! grep -q 'listening on http://127\.0\.0\.1:' "$S/blank.log"; then
  echo "    a blank variable was taken for an address:"; cat "$S/blank.log"; exit 1
fi

# ---------------------------------------------------------------- an address nothing binds
if MAJORDOMUS_HTTP_HOST=not-an-address "$RB" serve --port 0 --idle 5 </dev/null >"$S/bad.log" 2>&1; then
  echo "    an unbindable address was served:"; cat "$S/bad.log"; exit 1
fi
if ! grep -q 'cannot bind not-an-address' "$S/bad.log"; then
  echo "    the refusal did not name the address:"; cat "$S/bad.log"; exit 1
fi
if ! grep -q 'MAJORDOMUS_HTTP_HOST names it' "$S/bad.log"; then
  echo "    the refusal's log did not name the variable that supplied it:"; cat "$S/bad.log"; exit 1
fi
# a refused bind leaves no lease for the next start to wait on
if [ -f "$lease" ]; then
  echo "    a refused bind left a lease:"; cat "$lease"; exit 1
fi

# ---------------------------------------------------------------- the server ensure starts
# The one that mattered: `serve ensure` passes no host, so the only way the server it
# starts can leave loopback is by inheriting the variable.
if ! MAJORDOMUS_HTTP_HOST=0.0.0.0 "$RB" serve ensure --port 0 --idle 120 --wait 40 --format json \
    > "$S/ensure.json" 2> "$S/ensure.err"; then
  echo "    serve ensure did not converge:"; cat "$S/ensure.json" "$S/ensure.err"; exit 1
fi
url="$(jq -r '.url // empty' "$S/ensure.json")"
case "$url" in
  http://0.0.0.0:*) ;;
  *) echo "    the server ensure started did not bind what the variable named: '$url'"
     cat "$S/ensure.json"; exit 1 ;;
esac

# desired is what a bind from this environment would take, so status and bind cannot differ
MAJORDOMUS_HTTP_HOST=0.0.0.0 "$RB" serve status --format json > "$S/status-named.json" 2>/dev/null || true
got="$(jq -r '.desired.host // empty' "$S/status-named.json")"
if [ "$got" != 0.0.0.0 ]; then
  echo "    desired.host is '$got' where the variable names 0.0.0.0"; cat "$S/status-named.json"; exit 1
fi
"$RB" serve stop >/dev/null 2>&1 || true

# and with that server gone and nothing naming an interface, desired is loopback again. It
# is asked after the stop on purpose: a running server answers for its own environment, and
# the one above was started under the variable.
"$RB" serve status --format json > "$S/status-default.json" 2>/dev/null || true
got="$(jq -r '.desired.host // empty' "$S/status-default.json")"
if [ "$got" != 127.0.0.1 ]; then
  echo "    desired.host is '$got' where nothing names an interface"; cat "$S/status-default.json"; exit 1
fi

# ---------------------------------------------------------------- the server mcp elects
# A client's `majordomus mcp` is the third thing that starts a server. It runs in the
# foreground here and serves for as long as its stdin stays open: the reader below holds it
# open until the server has said where it listens, at most fifteen seconds, and then ends,
# which is the client going away. Nothing is left running if this case is interrupted.
MAJORDOMUS_HTTP_HOST=0.0.0.0 "$RB" mcp --http-port 0 >"$S/mcp.out" 2>"$S/mcp.log" < <(
  n=0
  while [ $n -lt 150 ] && ! grep -q 'shared server listening' "$S/mcp.log" 2>/dev/null; do
    n=$((n+1)); sleep 0.1
  done
) || true
if ! grep -q 'listening on http://0\.0\.0\.0:' "$S/mcp.log"; then
  echo "    the server mcp elected did not bind what the variable named:"; cat "$S/mcp.log"; exit 1
fi
