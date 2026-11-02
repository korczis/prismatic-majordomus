# majordomus-timeout: 240
# majordomus-covers: capability:mesh.build_cache.env
# Every local build of this repository goes through the mesh build cache, and a gate refuses a
# new bypass (I3953, ADR 0157). The cargo choke point (lib/rust_bin.sh) starts the sccache
# server with the store as its second level, and asks the store only then. Stand-ins only: an
# sccache that listens on a private port and records how it was started, and a store that
# answers the readiness route and counts the questions. No real store, no real sccache server.
#
#   store healthy, no server    the server is started with the store (disk,s3), the credential
#                               by file path, and cargo is pointed at sccache; the store is
#                               asked once
#   a server already running    nothing is started and the store is not asked, however many
#                               compilations follow
#   store down                  the build succeeds, local-only, with one line saying why
#   MAJORDOMUS_BUILD_CACHE=off  nothing is asked, nothing started
#   a cold node                 no majordomus executable: the build is what it was, one line
#   the gate                    refuses `npm ci` and `cargo build` around the choke point, passes
#                               a line that declares why, and this repository passes
#
# Skips itself when there is no python3 to stub the store.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
mj_runs python3 || skip "$(mj_runs_why python3)"
command -v nc >/dev/null 2>&1 || skip "no nc to see whether a server listens"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3953.XXXXXX")"
cleanup() {
  for f in "$S"/listener.*.pid "$S"/store.pid; do
    [ -f "$f" ] && kill "$(cat "$f")" 2>/dev/null || true
  done
  rm -rf "$S"
}
trap cleanup EXIT
mkdir -p "$S/cwd" "$S/xdg/majordomus" "$S/xdg/mesh-build-cache" "$S/bin"
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }
SECRET='wJalrXUtnFEMI-NOT-A-REAL-KEY-3953'

# ---------------------------------------------------------------- the stand-ins
cat > "$S/store.py" <<'PY'
import http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/ready":
            with open(sys.argv[2], "a") as f:
                f.write("ask\n")
        self.send_response(200 if self.path == "/ready" else 404)
        self.send_header("Content-Length", "0")
        self.end_headers()
    def log_message(self, *a):
        pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
cat > "$S/listener.py" <<'PY'
import socket, sys, time
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen(5); time.sleep(120)
PY
# An sccache that refuses an unknown chain level by name (so it is measured as multi-level),
# and otherwise "starts" a server: it listens on its port and records how it was started.
cat > "$S/bin/sccache" <<SCCACHE_STUB
#!/bin/sh
case "\$1" in
  --version) echo 'sccache 0.18.0' ;;
  --stop-server) [ -f "$S/listener.\$SCCACHE_SERVER_PORT.pid" ] && kill "\$(cat "$S/listener.\$SCCACHE_SERVER_PORT.pid")" 2>/dev/null; exit 0 ;;
  --start-server)
    case "\${SCCACHE_MULTILEVEL_CHAIN:-}" in
      disk,s3|"") ;;
      *) echo "sccache: error: Unknown cache level: '\$SCCACHE_MULTILEVEL_CHAIN'" >&2; exit 2 ;;
    esac
    echo "\${SCCACHE_MULTILEVEL_CHAIN:-local} \${SCCACHE_BUCKET:-unset}" >> "$S/starts"
    env | grep -E '^(SCCACHE|AWS)' > "$S/start.env"
    python3 "$S/listener.py" "\$SCCACHE_SERVER_PORT" >/dev/null 2>&1 &
    echo \$! > "$S/listener.\$SCCACHE_SERVER_PORT.pid"
    sleep 0.3 ;;
esac
SCCACHE_STUB
chmod +x "$S/bin/sccache"

PORT="$(free_port)"; DEAD="$(free_port)"; SP="$(free_port)"
python3 "$S/store.py" "$PORT" "$S/asks" >/dev/null 2>&1 & echo $! > "$S/store.pid"
i=0; until curl -fsS -m 1 "http://127.0.0.1:$PORT/ready" >/dev/null 2>&1; do
  i=$((i + 1)); [ "$i" -lt 50 ] || { echo "    the stub store did not come up"; exit 1; }
  sleep 0.1
done
: > "$S/asks"
node_file() { printf '{"provider":{"provider":"minio","scope":"host","endpoint":"http://127.0.0.1:%s","bucket":"the-bucket","ready_path":"/ready"}}\n' "$1" > "$S/xdg/majordomus/build-cache.json"; }
CRED="$S/xdg/mesh-build-cache/credentials"
printf '[mesh-build-cache]\naws_access_key_id = AKIDEXAMPLE\naws_secret_access_key = %s\n' "$SECRET" > "$CRED"
chmod 600 "$CRED"
asks() { wc -l < "$S/asks" | tr -d ' '; }
starts() { if [ -f "$S/starts" ]; then wc -l < "$S/starts" | tr -d ' '; else echo 0; fi; }

# one compilation's worth of the choke point: a fresh process that has not decided yet
choke() { # [env assignments...] -> prints the RUSTC_WRAPPER state; stderr to $S/err
  ( cd "$S/cwd" && env -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER -u MJ_BUILD_CACHE_DONE \
      PATH="$S/bin:$PATH" XDG_CONFIG_HOME="$S/xdg" SCCACHE_SERVER_PORT="$SP" MAJORDOMUS_BIN="${MJBIN:-$RB}" "$@" \
      sh -c ". '$ROOT/lib/rust_bin.sh'; mj_build_cache_start '$ROOT' && echo \"wrapper=\${RUSTC_WRAPPER-unset}\"" 2>"$S/err" )
}

# ---------------------------------------------------------------- 1. a healthy store
node_file "$PORT"
out="$(choke)" || { echo "    the choke point failed with a healthy store"; cat "$S/err"; exit 1; }
[ "$out" = "wrapper=sccache" ] || { echo "    cargo was not pointed at sccache: $out"; cat "$S/err"; exit 1; }
[ "$(starts)" = 1 ] && grep -qx 'disk,s3 the-bucket' "$S/starts" \
  || { echo "    the server was not started with the store as its second level: $(cat "$S/starts" 2>/dev/null)"; exit 1; }
grep -qx "AWS_SHARED_CREDENTIALS_FILE=$CRED" "$S/start.env" || { echo "    the credential did not reach the server by file path"; exit 1; }
grep -qF "$SECRET" "$S/start.env" && { echo "    a key reached the server's environment"; exit 1; }
[ "$(asks)" = 1 ] || { echo "    the store was asked $(asks) time(s) to start one server, not once"; exit 1; }
echo "    a healthy store becomes the server's second level, the credential by path, the store asked once"

# ---------------------------------------------------------------- 2. never per compilation
for _ in 1 2 3 4 5; do
  out="$(choke)" || { echo "    a compilation failed with a server running"; exit 1; }
  [ "$out" = "wrapper=sccache" ] || { echo "    a running server was not used: $out"; exit 1; }
done
[ "$(starts)" = 1 ] || { echo "    a server was started again while one ran"; exit 1; }
[ "$(asks)" = 1 ] || { echo "    the store was asked again by a compilation: $(asks)"; exit 1; }
echo "    five more compilations start nothing and ask the store nothing"

# ---------------------------------------------------------------- 3. the store is down
kill "$(cat "$S/listener.$SP.pid")"; sleep 0.3
node_file "$DEAD"
before_starts="$(starts)"
out="$(choke)" || { echo "    a down store failed the build"; cat "$S/err"; exit 1; }
[ "$out" = "wrapper=unset" ] || { echo "    local-only build was pointed at sccache: $out"; exit 1; }
[ "$(starts)" = "$before_starts" ] || { echo "    a server was started with a store that is down"; exit 1; }
[ "$(wc -l < "$S/err" | tr -d ' ')" = 1 ] && grep -q 'build cache: local-only: ' "$S/err" \
  || { echo "    the down store left no single recorded line: $(cat "$S/err")"; exit 1; }
echo "    a store that is down leaves a local-only build that succeeds, with one line: $(sed 's/^majordomus: //' "$S/err")"

# ---------------------------------------------------------------- 4. off
node_file "$PORT"
a="$(asks)"; b="$(starts)"
out="$(choke MAJORDOMUS_BUILD_CACHE=off)" || { echo "    off failed the build"; exit 1; }
[ "$out" = "wrapper=unset" ] && [ "$(asks)" = "$a" ] && [ "$(starts)" = "$b" ] \
  || { echo "    MAJORDOMUS_BUILD_CACHE=off asked or started something"; exit 1; }
echo "    MAJORDOMUS_BUILD_CACHE=off asks nothing and starts nothing"

# a wrapper the caller named is respected, even an empty one
out="$( cd "$S/cwd" && env PATH="$S/bin:$PATH" XDG_CONFIG_HOME="$S/xdg" SCCACHE_SERVER_PORT="$SP" MAJORDOMUS_BIN="$RB" RUSTC_WRAPPER= \
  sh -c ". '$ROOT/lib/rust_bin.sh'; mj_build_cache_start '$ROOT'; echo \"wrapper=[\$RUSTC_WRAPPER]\"" 2>/dev/null)"
[ "$out" = "wrapper=[]" ] && [ "$(asks)" = "$a" ] || { echo "    a wrapper the caller named was overridden: $out"; exit 1; }
echo "    a RUSTC_WRAPPER the caller set, even an empty one, is left alone"

# ---------------------------------------------------------------- 5. a cold node
a="$(asks)"
out="$(MJBIN="$S/no-such-majordomus" choke)" || { echo "    a node without majordomus failed the build"; exit 1; }
[ "$out" = "wrapper=unset" ] && [ "$(asks)" = "$a" ] && grep -q 'no majordomus executable yet' "$S/err" \
  || { echo "    a cold node was not built as before: $out $(cat "$S/err")"; exit 1; }
echo "    a node with no majordomus yet builds as before, with one line saying so"

# ---------------------------------------------------------------- 6. the gate
G="$ROOT/scripts/ci/build-cache-check"
[ -x "$G" ] || { echo "    scripts/ci/build-cache-check is not executable"; exit 1; }
mkdir -p "$S/fx"
printf '#!/bin/sh\nset -e\nmj_cargo_bounded "$ROOT" build --quiet\nnpm ci\n' > "$S/fx/bad-npm.sh"
printf '#!/bin/sh\nset -e\ncd app && cargo build --locked\n' > "$S/fx/bad-cargo.sh"
printf '#!/bin/sh\nset -e\nmix deps.get\n' > "$S/fx/bad-mix.sh"
for f in bad-npm bad-cargo bad-mix; do
  rc=0; "$G" "$S/fx/$f.sh" >"$S/gate.out" 2>&1 || rc=$?
  [ "$rc" = 10 ] && grep -q "^BYPASS .*$f.sh" "$S/gate.out" || { echo "    the gate passed $f (exit $rc): $(cat "$S/gate.out")"; exit 1; }
done
printf '#!/bin/sh\nset -e\nnpm ci # build-cache: the lockfile is generated here, nothing to reuse\n' > "$S/fx/good-same-line.sh"
printf '#!/bin/sh\nset -e\n# build-cache: a one-off bootstrap before the executable exists\ncargo build --locked\n' > "$S/fx/good-line-before.sh"
printf '#!/bin/sh\ncargo() { mj_cargo_bounded "$ROOT" "$@"; }\ncargo build --quiet\necho "run npm ci, then cargo build"\n# cargo test is documented here\n' > "$S/fx/good-choke.sh"
for f in good-same-line good-line-before good-choke; do
  "$G" "$S/fx/$f.sh" >"$S/gate.out" 2>&1 || { echo "    the gate refused $f: $(cat "$S/gate.out")"; exit 1; }
done
"$G" >"$S/gate.out" 2>&1 || { echo "    this repository has an undeclared bypass: $(cat "$S/gate.out")"; exit 1; }
grep -q 'id: build-cache-check' "$ROOT/.ai/repo/ci/gates.yaml" || { echo "    the gate is not in gates.yaml"; exit 1; }
echo "    the gate refuses npm ci, cargo build and mix deps.get around the choke point, passes a declared reason, and the repository passes"
