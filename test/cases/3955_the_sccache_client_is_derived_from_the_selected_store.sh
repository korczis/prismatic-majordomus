# majordomus-timeout: 240
# majordomus-covers: capability:mesh.build_cache.env
# The sccache client is derived from the selected store, never typed: the local disk is the
# first level and the store the second, only when the store is healthy when the sccache server
# starts, and the credential is named by the path of its file (mesh.build_cache.env, I2673,
# ADR 0157). A stub store answers the readiness route; no real store is ever contacted, and no
# sccache server of this machine is touched (a throwaway one runs on a private port).
#
#   no node file / store down       local-only: no variable, the reason says which condition
#   the store answers, a 0600 file  the nine variables, the file's PATH and no key anywhere
#   a file open to others           local-only
#   an sccache without the chain    local-only, and the throwaway server it started is stopped
#   a store stopped after           local-only again: health is read at every start
#   `eval` of the text              sets the variables in a shell
#   over MCP                        the same answer as the command line
#   the shipped wrapper             asks only when no server listens; a down store, a missing
#                                   majordomus and SCCACHE_MESH_OFF leave the compile running
#   the installed sccache           its multi-level support is measured, not assumed
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, or no python3 to stub the store.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
command -v python3 >/dev/null 2>&1 || skip "no python3 to stub the store"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3955.XXXXXX")"
PIDS=""
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$S"; }
trap cleanup EXIT
REPO="$PWD"
"$MJ" init >/dev/null
# a directory that is no repository: the answer is a fact of the machine
mkdir -p "$S/cwd" "$S/xdg/majordomus" "$S/xdg/mesh-build-cache"
cd "$S/cwd" || exit 1

SECRET='wJalrXUtnFEMI-NOT-A-REAL-KEY-7MDENG'
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }

# ---------------------------------------------------------------- the stubs
cat > "$S/store.py" <<'PY'
import http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200 if self.path == "/ready" else 404)
        self.send_header("Content-Length", "0")
        self.end_headers()
    def log_message(self, *a):
        pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
PORT="$(free_port)"
python3 "$S/store.py" "$PORT" & STORE=$!; PIDS="$STORE"
i=0; until curl -fsS -m 1 "http://127.0.0.1:$PORT/ready" >/dev/null 2>&1; do
  i=$((i + 1)); [ "$i" -lt 50 ] || { echo "    the stub store did not come up"; exit 1; }
  sleep 0.1
done
DEAD_PORT="$(free_port)"

node_file() { # port -> path
  printf '{"provider":{"provider":"minio","scope":"host","endpoint":"http://127.0.0.1:%s","bucket":"the-bucket","ready_path":"/ready"}}\n' "$1" > "$S/xdg/majordomus/build-cache.json"
}
CRED="$S/xdg/mesh-build-cache/credentials"
printf '[mesh-build-cache]\naws_access_key_id = AKIDEXAMPLE\naws_secret_access_key = %s\n' "$SECRET" > "$CRED"
chmod 600 "$CRED"

# An sccache that knows the chain refuses an unknown level by name; one that does not, starts.
# Both record what they were asked, and any other call is the compiler wrapper's exec.
cat > "$S/sccache-capable" <<EOF
#!/bin/sh
case "\$1" in
  --version) echo 'sccache 0.18.0' ;;
  --start-server)
    echo "\$SCCACHE_SERVER_PORT \${SCCACHE_BUCKET:-unset}" >> "$S/capable.starts"
    echo "sccache: error: Server startup failed: Unknown cache level: '\$SCCACHE_MULTILEVEL_CHAIN'" >&2
    exit 2 ;;
  *) env > "$S/exec.env"; echo "\$*" > "$S/exec.args" ;;
esac
EOF
cat > "$S/sccache-incapable" <<EOF
#!/bin/sh
case "\$1" in
  --version) echo 'sccache 0.9.0' ;;
  --start-server) echo 'sccache: Listening on address 127.0.0.1:1' ;;
  --stop-server) : > "$S/incapable.stopped" ;;
esac
EOF
chmod +x "$S/sccache-capable" "$S/sccache-incapable"

env_json() { # sccache [extra args] -> the report
  sc="$1"; shift
  XDG_CONFIG_HOME="$S/xdg" "$RB" mesh build-cache env --sccache "$sc" --format json "$@" 2>"$S/err"
}
mode_of() { printf '%s' "$1" | jq -r '.mode'; }

# ---------------------------------------------------------------- 1. nothing offered
rm -f "$S/xdg/majordomus/build-cache.json"
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = local-only ] || { echo "    a node with no store is not local-only: $r"; exit 1; }
printf '%s' "$r" | jq -e '(.variables | length) == 0 and (.reason | test("no node offers"))' >/dev/null \
  || { echo "    local-only carries a variable or no reason: $r"; exit 1; }
text="$(XDG_CONFIG_HOME="$S/xdg" "$RB" mesh build-cache env --sccache "$S/sccache-capable")"
case "$text" in "# build cache: local-only: "*) ;; *) echo "    no local-only comment: $text"; exit 1 ;; esac
printf '%s' "$text" | grep -q '^export ' && { echo "    a local-only script exports: $text"; exit 1; }
echo "    with no store offered the client is local-only, with the reason, and exports nothing"

# ---------------------------------------------------------------- 2. a healthy store
node_file "$PORT"
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = mesh ] || { echo "    a healthy store is not the second level: $r"; cat "$S/err"; exit 1; }
want='["SCCACHE_MULTILEVEL_CHAIN","SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY","SCCACHE_BUCKET","SCCACHE_ENDPOINT","SCCACHE_REGION","SCCACHE_S3_USE_SSL","SCCACHE_S3_KEY_PREFIX","AWS_SHARED_CREDENTIALS_FILE","AWS_PROFILE"]'
printf '%s' "$r" | jq -e --argjson want "$want" '[.variables[].name] == $want' >/dev/null \
  || { echo "    the variables are not the derived nine: $r"; exit 1; }
val() { printf '%s' "$r" | jq -r --arg n "$1" '.variables[] | select(.name == $n) | .value'; }
[ "$(val SCCACHE_MULTILEVEL_CHAIN)" = "disk,s3" ] || { echo "    the chain is not disk,s3"; exit 1; }
[ "$(val SCCACHE_BUCKET)" = the-bucket ] || { echo "    the bucket is not the store's"; exit 1; }
[ "$(val SCCACHE_ENDPOINT)" = "http://127.0.0.1:$PORT" ] || { echo "    the endpoint is not the store's"; exit 1; }
[ "$(val SCCACHE_S3_USE_SSL)" = false ] || { echo "    an http endpoint is not cleartext"; exit 1; }
[ "$(val AWS_PROFILE)" = mesh-build-cache ] || { echo "    the profile is not mesh-build-cache"; exit 1; }
[ "$(val AWS_SHARED_CREDENTIALS_FILE)" = "$CRED" ] || { echo "    the credential is not the file path"; exit 1; }
if command -v rustc >/dev/null 2>&1; then
  [ "$(val SCCACHE_S3_KEY_PREFIX)" = "rust/$(rustc -vV | sed -n 's/^host: //p')/" ] \
    || { echo "    the key prefix is not rustc's host triple: $(val SCCACHE_S3_KEY_PREFIX)"; exit 1; }
fi
text="$(XDG_CONFIG_HOME="$S/xdg" "$RB" mesh build-cache env --sccache "$S/sccache-capable")"
for said in "$r" "$text"; do
  case "$said" in
    *"$SECRET"*|*AKIDEXAMPLE*|*AWS_SECRET_ACCESS_KEY*|*AWS_ACCESS_KEY_ID*)
      echo "    a key is in the answer"; exit 1 ;;
  esac
done
echo "    a healthy store with a 0600 credential file is the second level: the file's path, no key"

# ---------------------------------------------------------------- 3. eval reads it
got="$(sh -c 'eval "$1"; printf "%s %s %s" "$SCCACHE_BUCKET" "$AWS_PROFILE" "$SCCACHE_MULTILEVEL_CHAIN"' sh "$text")"
[ "$got" = "the-bucket mesh-build-cache disk,s3" ] || { echo "    eval of the text set: $got"; exit 1; }
echo "    eval of the text sets the variables in a shell"

# ---------------------------------------------------------------- 4. the credential is judged
chmod 644 "$CRED"
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = local-only ] && printf '%s' "$r" | jq -e '.reason | test("must be 0600")' >/dev/null \
  || { echo "    a credential open to others was accepted: $r"; exit 1; }
chmod 600 "$CRED"
mv "$CRED" "$CRED.away"
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = local-only ] && printf '%s' "$r" | jq -e '.reason | test("no credential file")' >/dev/null \
  || { echo "    a missing credential was accepted: $r"; exit 1; }
mv "$CRED.away" "$CRED"
echo "    a credential file open to others, or absent, leaves the store out"

# ---------------------------------------------------------------- 5. no chain in the sccache
r="$(env_json "$S/sccache-incapable")"
[ "$(mode_of "$r")" = local-only ] && printf '%s' "$r" | jq -e '.sccache.multilevel == false' >/dev/null \
  || { echo "    an sccache without the chain was given the store: $r"; exit 1; }
[ -f "$S/incapable.stopped" ] || { echo "    the throwaway server was left running"; exit 1; }
echo "    an sccache that ignores the chain is measured so, gets no store, and its throwaway server is stopped"

# ---------------------------------------------------------------- 6. a store that is not there
before="$(wc -l < "$S/capable.starts" | tr -d ' ')"
node_file "$DEAD_PORT"
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = local-only ] && printf '%s' "$r" | jq -e '(.variables | length) == 0 and ([.passed_over[].why] | any(test("unhealthy")))' >/dev/null \
  || { echo "    a store that is down was given to the server: $r"; exit 1; }
[ "$(wc -l < "$S/capable.starts" | tr -d ' ')" = "$before" ] \
  || { echo "    a server was started to measure a store that is not there"; exit 1; }
node_file "$PORT"
kill "$STORE"; wait "$STORE" 2>/dev/null || true
r="$(env_json "$S/sccache-capable")"
[ "$(mode_of "$r")" = local-only ] || { echo "    a store stopped after being up is still the second level: $r"; exit 1; }
echo "    a store that is down, or stopped since, is left out of the chain: health is read at every start"
python3 "$S/store.py" "$PORT" & STORE=$!; PIDS="$STORE"
i=0; until curl -fsS -m 1 "http://127.0.0.1:$PORT/ready" >/dev/null 2>&1; do
  i=$((i + 1)); [ "$i" -lt 50 ] || { echo "    the stub store did not come back"; exit 1; }
  sleep 0.1
done

# ---------------------------------------------------------------- 7. MCP agrees
args="$(jq -cn --arg s "$S/sccache-capable" --arg n "$S/xdg/majordomus/build-cache.json" --arg c "$CRED" '{sccache:$s,node_file:$n,credentials:$c}')"
mcp="$( { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3955","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_mesh_build_cache_env","arguments":%s}}\n' "$args"
} | "$RB" mcp --standalone --repo "$REPO" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty')"
[ -n "$mcp" ] || { echo "    the tool call answered nothing"; cat "$S/mcp.err"; exit 1; }
[ "$(printf '%s' "$mcp" | jq -c '[.mode, [.variables[].name]]')" = "$(env_json "$S/sccache-capable" | jq -c '[.mode, [.variables[].name]]')" ] \
  || { echo "    MCP and the command line disagree: $mcp"; exit 1; }
case "$mcp" in *"$SECRET"*) echo "    a key is in the MCP answer"; exit 1 ;; esac
echo "    a tools/call over MCP answers the same configuration"

# ---------------------------------------------------------------- 8. the shipped wrapper
W="$ROOT/share/libexec/sccache-mesh"
[ -x "$W" ] || { echo "    share/libexec/sccache-mesh is not executable"; exit 1; }
sh -n "$W" || { echo "    the wrapper does not parse"; exit 1; }
SP="$(free_port)"
run_wrapper() { # [majordomus] [SCCACHE_MESH_OFF value]: the compiler's arguments must pass through
  rm -f "$S/exec.env" "$S/exec.args"
  env XDG_CONFIG_HOME="$S/xdg" SCCACHE_SERVER_PORT="$SP" SCCACHE_MESH_SCCACHE="$S/sccache-capable" \
    SCCACHE_MESH_OFF="${2:-}" MAJORDOMUS_BIN="${1:-$RB}" "$W" rustc -vV >/dev/null 2>&1
}
run_wrapper || { echo "    the wrapper failed with a store up"; exit 1; }
grep -q '^SCCACHE_BUCKET=the-bucket$' "$S/exec.env" || { echo "    no server running, store healthy: the store did not reach sccache"; exit 1; }
grep -q "^AWS_SHARED_CREDENTIALS_FILE=$CRED\$" "$S/exec.env" || { echo "    the credential path did not reach sccache"; exit 1; }
grep -q '^SCCACHE_MULTILEVEL_CHAIN=disk,s3$' "$S/exec.env" || { echo "    the chain did not reach sccache"; exit 1; }
grep -qF "$SECRET" "$S/exec.env" && { echo "    a key reached the compiler's environment"; exit 1; }
[ "$(cat "$S/exec.args")" = "rustc -vV" ] || { echo "    the compiler's arguments did not pass through: $(cat "$S/exec.args")"; exit 1; }
echo "    the wrapper starts sccache with the derived store, the credential by path, the arguments intact"

# the provisioned copy (I3997) is the same file: from another path it gets the same configuration
mkdir -p "$S/installed"; cp "$W" "$S/installed/sccache-mesh"; chmod +x "$S/installed/sccache-mesh"
cp "$S/exec.env" "$S/exec.env.shipped"
rm -f "$S/exec.env"
env XDG_CONFIG_HOME="$S/xdg" SCCACHE_SERVER_PORT="$SP" SCCACHE_MESH_SCCACHE="$S/sccache-capable" \
  MAJORDOMUS_BIN="$RB" "$S/installed/sccache-mesh" rustc -vV >/dev/null 2>&1
for v in SCCACHE_BUCKET SCCACHE_ENDPOINT SCCACHE_S3_USE_SSL SCCACHE_S3_KEY_PREFIX SCCACHE_MULTILEVEL_CHAIN AWS_SHARED_CREDENTIALS_FILE; do
  [ "$(grep "^$v=" "$S/exec.env")" = "$(grep "^$v=" "$S/exec.env.shipped")" ] \
    && [ -n "$(grep "^$v=" "$S/exec.env")" ] || { echo "    the installed copy and the shipped wrapper disagree on $v"; exit 1; }
done
[ ! -e "$ROOT/share/mesh-cache-provider/sccache-mesh" ] || { echo "    a second wrapper source exists"; exit 1; }
echo "    both entry points get identical configuration from mesh.build_cache.env"

# a server already listening: nothing is asked of majordomus
cat > "$S/asked-mj" <<EOF
#!/bin/sh
echo "\$*" >> "$S/asked.log"
exit 1
EOF
chmod +x "$S/asked-mj"
python3 -c 'import socket,sys,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen(5); time.sleep(60)' "$SP" & LISTENER=$!
PIDS="$PIDS $LISTENER"
sleep 0.5
if command -v nc >/dev/null 2>&1; then
  run_wrapper "$S/asked-mj"
  [ ! -f "$S/asked.log" ] || { echo "    majordomus was asked while a server was running: $(cat "$S/asked.log")"; exit 1; }
  [ -f "$S/exec.args" ] && ! grep -q '^SCCACHE_BUCKET=' "$S/exec.env" \
    || { echo "    a running server was handed the store again"; exit 1; }
  echo "    with a server already listening the wrapper asks nothing and runs the compiler"
fi
kill "$LISTENER" 2>/dev/null

# no majordomus, SCCACHE_MESH_OFF, a down store: the compile still runs, local-only
run_wrapper "$S/no-such-majordomus" && [ -f "$S/exec.args" ] && ! grep -q '^SCCACHE_BUCKET=' "$S/exec.env" \
  || { echo "    a missing majordomus stopped or changed the compile"; exit 1; }
run_wrapper "$RB" 1 && ! grep -q '^SCCACHE_BUCKET=' "$S/exec.env" \
  || { echo "    SCCACHE_MESH_OFF did not leave the build local-only"; exit 1; }
node_file "$DEAD_PORT"
run_wrapper && [ -f "$S/exec.args" ] && ! grep -q '^SCCACHE_BUCKET=' "$S/exec.env" \
  || { echo "    a down store changed or stopped the compile"; exit 1; }
node_file "$PORT"
echo "    a missing majordomus, SCCACHE_MESH_OFF and a down store each leave the compile running, local-only"

# ---------------------------------------------------------------- 9. the installed sccache
if command -v sccache >/dev/null 2>&1; then
  r="$(env_json sccache)"
  printf '%s' "$r" | jq -e '.sccache.found and (.sccache.version | startswith("sccache")) and (.sccache.multilevel != null)' >/dev/null \
    || { echo "    the installed sccache's multi-level support was not measured: $r"; exit 1; }
  printf '%s' "$r" | jq -e '(.mode == "mesh") == (.sccache.multilevel == true)' >/dev/null \
    || { echo "    the mode does not follow the measurement: $r"; exit 1; }
  echo "    the installed $(printf '%s' "$r" | jq -r '.sccache.version') was measured: multilevel=$(printf '%s' "$r" | jq -r '.sccache.multilevel')"
else
  echo "    no sccache installed here: the real multi-level measurement is not made by this run"
fi
