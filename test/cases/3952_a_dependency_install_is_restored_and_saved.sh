# majordomus-covers: capability:mesh.build_cache.ensure
# A dependency install is restored before and saved after, verified by its digest, the local
# level first and the node's store second (mesh.build_cache.ensure, I3952, ADR 0157 as amended
# on 2026-10-22, rule project.cache-is-invisible v2).
#
# A disposable repository tracking a package-lock.json, a mix.lock and a Cargo.lock; its
# toolchains are stubs on PATH, so every key is the same on every machine. Two "nodes" are two
# HOMEs and two clones, sharing one stub S3 store (a python3 http.server, started here):
#
#   cold, node 1          the install runs, its tree is saved to the local level and the store
#   again                 the key is in place: nothing runs
#   warm                  restored from the local level, the install is not run
#   bypassed              MAJORDOMUS_BUILD_CACHE=off: no level is asked, the install runs
#   equivalence           cold, warm and bypassed leave the same file list and contents
#                         (the marker naming the key is the one file left out)
#   node 2                what node 1 saved is restored from the store; the install never runs
#   a corrupt archive     its SHA-256 does not match: discarded, and the install runs
#   an escaping link      refused, nothing of it lands, and the install runs
#   a failing install     its exit code is ensure's, and nothing is saved
#   mix                   a hit runs `mix deps.get` anyway
#   cargo build dir       a cold one is restored without incremental state, CACHEDIR.TAG kept
#   no lockfile           the command runs uncached and says why
#   the reaper            scripts/reap-orphans measures the local level and reduces it
#
# Nothing outside the case's own directory is written: HOME, CARGO_HOME and the store are
# the case's. Skips without cargo/MAJORDOMUS_BIN, python3 or jq.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
command -v python3 >/dev/null 2>&1 || skip "python3 is not installed: the stub store needs it"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3952.XXXXXX")"; S="$(cd "$S" && pwd -P)"
mkdir -p "$S/bin" "$S/store" "$S/h1" "$S/h2"
STORE_PID=""
trap '[ -z "$STORE_PID" ] || kill "$STORE_PID" 2>/dev/null || true; rm -rf "$S"' EXIT

# ---------------------------------------------------------------- toolchain stubs
printf '#!/bin/sh\necho v24.0.0\n' > "$S/bin/node"
printf '#!/bin/sh\necho 11.0.0\n' > "$S/bin/npm"
printf '#!/bin/sh\necho 1.19.0\n' > "$S/bin/elixir"
printf '#!/bin/sh\nprintf 28\n' > "$S/bin/erl"
printf '#!/bin/sh\nprintf "host: stub-host\\nrelease: 1.98.1\\ncommit-hash: unknown\\n"\n' > "$S/bin/rustc"
chmod +x "$S/bin/"*
PATH="$S/bin:$PATH"; export PATH

# ---------------------------------------------------------------- the stub store
cat > "$S/store.py" <<'PY'
import http.server, os, sys
root = sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def path_of(self):
        rel = self.path.lstrip("/").replace("/", "%2F")
        return os.path.join(root, rel)
    def answer(self, code, body=b""):
        self.send_response(code)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)
    def do_GET(self):
        if self.path.startswith("/minio/health"):
            return self.answer(200)
        if not self.headers.get("authorization", "").startswith("AWS4-HMAC-SHA256 "):
            return self.answer(403)
        p = self.path_of()
        if not os.path.isfile(p):
            return self.answer(404)
        with open(p, "rb") as f:
            self.answer(200, f.read())
    do_HEAD = do_GET
    def do_PUT(self):
        n = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(n)
        with open(self.path_of(), "wb") as f:
            f.write(body)
        self.answer(200)
srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
with open(sys.argv[2], "w") as f:
    f.write(str(srv.server_address[1]))
srv.serve_forever()
PY
python3 -I "$S/store.py" "$S/store" "$S/port" &
STORE_PID=$!
i=0; while [ ! -s "$S/port" ] && [ "$i" -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
[ -s "$S/port" ] || { echo "    the stub store did not start"; exit 1; }
URL="http://127.0.0.1:$(cat "$S/port")"
for h in h1 h2; do
  mkdir -p "$S/$h/.config/majordomus" "$S/$h/.config/mesh-build-cache"
  printf '{"provider":{"provider":"minio","scope":"host","endpoint":"%s","bucket":"cache"}}\n' "$URL" \
    > "$S/$h/.config/majordomus/build-cache.json"
  printf '[mesh-build-cache]\naws_access_key_id = node-%s\naws_secret_access_key = stub-secret\n' "$h" \
    > "$S/$h/.config/mesh-build-cache/credentials"
  chmod 600 "$S/$h/.config/mesh-build-cache/credentials"
done

# ---------------------------------------------------------------- the repository
"$MJ" init >/dev/null
mkdir -p site app .cargo
printf '{"lockfileVersion":3}\n' > site/package-lock.json
printf '%%{}\n' > app/mix.lock
printf 'version = 4\n' > Cargo.lock
printf '[workspace]\n' > Cargo.toml
printf '[build]\nbuild-dir = "{cargo-cache-home}/build-shared/fixture"\n' > .cargo/config.toml
printf 'node_modules/\ndeps/\n' > .gitignore
git add -A >/dev/null && git commit -qm fixture
git update-ref refs/remotes/origin/master HEAD
N1="$PWD"
git clone -q "$N1" "$S/n2" || { echo "    the second node's clone failed"; exit 1; }
N2="$S/n2"

# ensure <home> <repo> <kind> [args...] -- the JSON answer in $OUT, the exit code in $RC
ensure() {
  h="$1"; repo="$2"; shift 2
  RC=0
  OUT="$(cd "$repo" && env -u XDG_CACHE_HOME HOME="$S/$h" CARGO_HOME="$S/$h/.cargo" \
    MAJORDOMUS_MIN_FREE_MB=0 "$RB" mesh build-cache ensure --format json "$@" 2>"$S/err")" || RC=$?
}
field() { printf '%s' "$OUT" | jq -r "$1"; }
INSTALL='mkdir -p node_modules/pkg/.bin node_modules/pkg/lib && printf "module.exports = 1\n" > node_modules/pkg/lib/index.js && printf "{\"name\":\"pkg\"}\n" > node_modules/pkg/package.json && ln -sf ../lib/index.js node_modules/pkg/.bin/pkg && echo ran >> ../ran.log'
npm_ensure() { ensure "$1" "$2" npm-modules --dir site -- sh -c "$INSTALL"; }
runs() { if [ -f "$1/ran.log" ]; then wc -l < "$1/ran.log" | tr -d ' '; else echo 0; fi; }
# every file, link and directory under a tree but the marker, with what it holds
snapshot() {
  (cd "$1" && find . -name .majordomus-build-cache-key -prune -o -print | LC_ALL=C sort | while IFS= read -r p; do
    if [ -L "$p" ]; then printf '%s -> %s\n' "$p" "$(readlink "$p")"
    elif [ -d "$p" ]; then printf '%s/\n' "$p"
    else printf '%s %s\n' "$p" "$(cksum < "$p")"; fi
  done)
}
expect_source() {
  [ "$(field .source)" = "$1" ] || { echo "    $2: source $(field .source), not $1: $OUT"; cat "$S/err"; exit 1; }
}

# ---------------------------------------------------------------- 1. cold on node 1
npm_ensure h1 "$N1"
[ "$RC" = 0 ] || { echo "    a cold ensure exited $RC: $OUT"; cat "$S/err"; exit 1; }
expect_source command "cold"
[ "$(field '.saved | join(",")')" = "local,store" ] || { echo "    the cold install was not saved to both levels: $OUT"; exit 1; }
KEY="$(field .key)"
case "$KEY" in v1/npm-modules/*/node-24.0.0-npm-11/*) ;; *) echo "    unexpected key $KEY"; exit 1 ;; esac
[ -f "$S/h1/.cache/majordomus/build-cache/$KEY.tar.gz" ] && [ -f "$S/h1/.cache/majordomus/build-cache/$KEY.sha256" ] \
  || { echo "    the local level holds no archive and digest under $KEY"; exit 1; }
[ -f "$S/store/cache%2F$(printf '%s' "$KEY" | sed 's#/#%2F#g').tar.gz" ] \
  || { echo "    the store holds no archive under $KEY: $(ls "$S/store")"; exit 1; }
[ "$(runs "$N1")" = 1 ] || { echo "    the cold install ran $(runs "$N1") times"; exit 1; }
COLD="$(snapshot "$N1/site/node_modules")"
echo "    cold: the install ran once and was saved to the local level and the store"

# ---------------------------------------------------------------- 2. in place
npm_ensure h1 "$N1"
expect_source in_place "the key in place"
[ "$(runs "$N1")" = 1 ] || { echo "    an install whose key was in place ran again"; exit 1; }
echo "    again: the key is in place and nothing runs"

# ---------------------------------------------------------------- 3. warm, 4. bypassed
rm -rf "$N1/site/node_modules"
npm_ensure h1 "$N1"
expect_source local "warm"
[ "$(runs "$N1")" = 1 ] || { echo "    a hit ran the install"; exit 1; }
WARM="$(snapshot "$N1/site/node_modules")"
rm -rf "$N1/site/node_modules"
RC=0
OUT="$(cd "$N1" && env -u XDG_CACHE_HOME HOME="$S/h1" MAJORDOMUS_BUILD_CACHE=off \
  "$RB" mesh build-cache ensure --format json npm-modules --dir site -- sh -c "$INSTALL" 2>"$S/err")" || RC=$?
expect_source bypassed "MAJORDOMUS_BUILD_CACHE=off"
[ "$(runs "$N1")" = 2 ] || { echo "    the bypassed install did not run"; exit 1; }
[ ! -e "$N1/site/node_modules/.majordomus-build-cache-key" ] || { echo "    a bypassed run wrote the marker"; exit 1; }
BYPASSED="$(snapshot "$N1/site/node_modules")"
[ "$COLD" = "$WARM" ] && [ "$COLD" = "$BYPASSED" ] || {
  echo "    cold, warm and bypassed trees differ:"; diff <(printf '%s\n' "$COLD") <(printf '%s\n' "$WARM"); diff <(printf '%s\n' "$COLD") <(printf '%s\n' "$BYPASSED"); exit 1; }
echo "    warm restores without running, bypassed runs, and the three trees are identical"

# ---------------------------------------------------------------- 5. node 2 restores from the store
npm_ensure h2 "$N2"
expect_source store "node 2"
[ "$(runs "$N2")" = 0 ] || { echo "    node 2 ran the install instead of restoring it"; exit 1; }
[ "$(snapshot "$N2/site/node_modules")" = "$COLD" ] || { echo "    node 2's tree differs from node 1's"; exit 1; }
[ -f "$S/h2/.cache/majordomus/build-cache/$KEY.tar.gz" ] || { echo "    node 2 kept no local copy"; exit 1; }
echo "    node 2 restores what node 1 saved, from the store, and keeps a local copy"

# ---------------------------------------------------------------- 6. a corrupt archive
# node 1 forgets its store for the next two, so that a discarded local archive is a miss
# rather than a restore from the store
mv "$S/h1/.config/majordomus/build-cache.json" "$S/h1-store.json"
A1="$S/h1/.cache/majordomus/build-cache/$KEY.tar.gz"
printf 'x' >> "$A1"
rm -rf "$N1/site/node_modules"
npm_ensure h1 "$N1"
expect_source command "a corrupt local archive"
field '.notes | join("\n")' | grep -q 'does not match' || { echo "    the digest mismatch was not said: $OUT"; exit 1; }
[ "$(runs "$N1")" = 3 ] || { echo "    the install did not run after a corrupt archive"; exit 1; }
[ "$(snapshot "$N1/site/node_modules")" = "$COLD" ] || { echo "    the tree after a corrupt archive differs"; exit 1; }
echo "    an archive whose SHA-256 does not match is discarded and the install runs"

# ---------------------------------------------------------------- 7. an escaping link
mkdir -p "$S/evil" && ln -s /etc "$S/evil/escape"
(cd "$S/evil" && COPYFILE_DISABLE=1 tar -czf "$A1" .)
shasum -a 256 "$A1" | awk '{ print $1 }' > "$S/h1/.cache/majordomus/build-cache/$KEY.sha256"
rm -rf "$N1/site/node_modules"
npm_ensure h1 "$N1"
expect_source command "an escaping link"
field '.notes | join("\n")' | grep -q 'outside its target' || { echo "    the escaping link was not said: $OUT"; exit 1; }
[ ! -e "$N1/site/node_modules/escape" ] || { echo "    the escaping link landed"; exit 1; }
[ "$(snapshot "$N1/site/node_modules")" = "$COLD" ] || { echo "    the tree after a refused archive differs"; exit 1; }
echo "    an archive carrying a link out of its target is refused and the install runs"

# ---------------------------------------------------------------- 8. a failing install
# a changed lockfile is a new key, so a miss
rm -rf "$N1/site/node_modules"
printf '{"lockfileVersion":3,"changed":true}\n' > "$N1/site/package-lock.json"
ensure h1 "$N1" npm-modules --dir site -- sh -c 'mkdir -p node_modules; exit 3'
[ "$RC" = 3 ] || { echo "    a failing install's exit code is not ensure's (got $RC): $OUT"; exit 1; }
[ "$(field '.saved | length')" = 0 ] || { echo "    a failing install was saved: $OUT"; exit 1; }
[ ! -e "$N1/site/node_modules/.majordomus-build-cache-key" ] || { echo "    a failing install was marked in place"; exit 1; }
git -C "$N1" checkout -q -- site/package-lock.json
echo "    a failing install's exit code is ensure's, and nothing of it is saved"

# ---------------------------------------------------------------- 9. mix runs on a hit
MIX='mkdir -p deps/dep && printf "defmodule Dep do end\n" > deps/dep/mix.exs && echo ran >> ../mix.log'
ensure h1 "$N1" mix-deps -- sh -c "$MIX"
expect_source command "mix cold"
rm -rf "$N1/app/deps"
ensure h1 "$N1" mix-deps -- sh -c "$MIX"
expect_source local "mix warm"
[ "$(wc -l < "$N1/mix.log" | tr -d ' ')" = 2 ] || { echo "    mix deps.get did not run on a hit"; exit 1; }
[ -f "$N1/app/deps/dep/mix.exs" ] || { echo "    mix deps were not restored"; exit 1; }
echo "    mix-deps: a hit is restored and the command runs anyway"

# ---------------------------------------------------------------- 10. a cold cargo build directory
mv "$S/h1-store.json" "$S/h1/.config/majordomus/build-cache.json"
BD1="$S/h1/.cargo/build-shared/fixture"
BUILD="mkdir -p '$BD1/debug/deps' '$BD1/debug/incremental/s-1' && printf dep > '$BD1/debug/deps/libdep.rlib' && printf inc > '$BD1/debug/incremental/s-1/x' && printf 'Signature: 8a477f597d28d172789f06886806bc55\n' > '$BD1/CACHEDIR.TAG' && : > '$BD1/debug/.cargo-build-lock'"
ensure h1 "$N1" cargo-build-dir -- sh -c "$BUILD"
expect_source command "cargo cold"
[ "$(field '.saved | join(",")')" = "local,store" ] || { echo "    the build directory was not saved: $OUT"; exit 1; }
ensure h2 "$N2" cargo-build-dir
expect_source store "cargo on node 2"
BD2="$S/h2/.cargo/build-shared/fixture"
[ "$(cat "$BD2/debug/deps/libdep.rlib")" = dep ] || { echo "    node 2's build directory lacks the dependency"; exit 1; }
[ ! -e "$BD2/debug/incremental" ] || { echo "    incremental state travelled"; exit 1; }
[ -f "$BD2/CACHEDIR.TAG" ] && [ -f "$BD2/debug/.cargo-build-lock" ] || { echo "    CACHEDIR.TAG or cargo's lock is missing"; exit 1; }
for left in "$S/h2/.cargo/build-shared"/.*mj-*; do
  [ ! -e "$left" ] || { echo "    a scratch directory was left beside the build directory: $left"; exit 1; }
done
ensure h2 "$N2" cargo-build-dir
field '.notes | join("\n")' | grep -q 'warm' || { echo "    a warm build directory was not left alone: $OUT"; exit 1; }
echo "    a cold build directory is restored on node 2 without incremental state, a warm one is left"

# ---------------------------------------------------------------- 11. no lockfile
mkdir -p "$N1/nowhere"
ensure h1 "$N1" npm-modules --dir nowhere -- true
[ "$RC" = 0 ] && [ "$(field .source)" = uncached ] || { echo "    an instance with no lockfile did not run uncached: $OUT"; exit 1; }
field '.notes[0]' | grep -q 'package-lock.json' || { echo "    the uncached run did not say why: $OUT"; exit 1; }
echo "    with no lockfile tracked there, the command runs uncached and says why"

# ---------------------------------------------------------------- 12. the reaper
if command -v lsof >/dev/null 2>&1; then
  out="$(cd "$N1" && MJ_ROOT="$N1" HOME="$S/h1" CARGO_HOME="$S/h1/.cargo" MAJORDOMUS_BUILD_CACHE_CAP_MB=0 \
    env -u XDG_CACHE_HOME "$ROOT/scripts/reap-orphans" --targets 2>&1)" || true
  printf '%s\n' "$out" | grep -q 'build-cache.*would reduce' || { echo "    the reaper did not measure the local level over its bound:"; printf '%s\n' "$out"; exit 1; }
  before="$(find "$S/h1/.cache/majordomus/build-cache" -name '*.tar.gz' | wc -l | tr -d ' ')"
  out="$(cd "$N1" && MJ_ROOT="$N1" HOME="$S/h1" CARGO_HOME="$S/h1/.cargo" MAJORDOMUS_BUILD_CACHE_CAP_MB=0 \
    env -u XDG_CACHE_HOME "$ROOT/scripts/reap-orphans" --reclaim 2>&1)" || { echo "    the reaper failed:"; printf '%s\n' "$out"; exit 1; }
  after="$(find "$S/h1/.cache/majordomus/build-cache" -name '*.tar.gz' | wc -l | tr -d ' ')"
  [ "$before" -gt 0 ] && [ "$after" = 0 ] || { echo "    the reaper left $after of $before archives over a bound of 0:"; printf '%s\n' "$out"; exit 1; }
  printf '%s\n' "$out" | grep -q 'build-cache.*reduced' || { echo "    the reduction was not said:"; printf '%s\n' "$out"; exit 1; }
  echo "    scripts/reap-orphans measures the local level and reduces it to its bound"
else
  echo "    (lsof absent: the reaper's half is not measured here)"
fi

# ---------------------------------------------------------------- 13. the switch is documented
grep -q 'MAJORDOMUS_BUILD_CACHE=off' "$ROOT/docs/CLI.md" || { echo "    docs/CLI.md does not document MAJORDOMUS_BUILD_CACHE=off"; exit 1; }
echo "    MAJORDOMUS_BUILD_CACHE=off is documented in docs/CLI.md"
