# majordomus-timeout: 420
# majordomus-covers: capability:self.update
# The server of an enrolled node follows the stable release by itself, by consent, and hands
# over in order (ADR 0161, ADR 0235; I2721). Against a complete local release served over a
# real HTTP connection, as case 2720 serves it: an older release is installed by install.sh,
# a fixture repository declares a mesh that admits the node's key, and the installed tool's own
# server runs the follower with a two-second interval.
#
# What is proved, through the real server and the real installer:
#   1. over a checkout with uncommitted changes the follower defers, installs nothing, and its
#      record and `self update --check` say why;
#   2. once the checkout is clean it installs the stable release within a few intervals,
#      reads it back from the new launcher, and hands the server over through the new
#      launcher's `serve ensure` — the server still answers and its mesh is still open;
#   3. a release whose archive the installer's verification refuses is installed nowhere, is
#      recorded as failed, and is not tried again.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
PIDS=""
# the servers first, then the release's own HTTP server, and only then a wait: a bare `wait`
# would wait on the fixture's HTTP server too
cleanup() {
  # and whatever server the lease names, should a hand-over have started one of its own
  lp="$(jq -r '.pid // empty' "${R:-/nonexistent}/.ai/local/state/mcp/server.json" 2>/dev/null || true)"
  [ -n "$lp" ] && kill "$lp" 2>/dev/null || true
  for p in $PIDS; do kill "$p" 2>/dev/null || true; done
  stop_http
  for p in $PIDS; do wait "$p" 2>/dev/null || true; done
}
trap cleanup EXIT INT TERM HUP

REAL_VERSION="$("$ROOT/scripts/release-version")"; VERSION="${REAL_VERSION%%-*}"
fixture() {
  MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" "$@" \
    > "$T/fixture.log" 2>&1 \
    || { echo "    the fixture release could not be built ($*)"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
}
fixture
cp "$FIX/releases/latest.json" "$T/latest.good"
cp "$ROOT/site/static/install.sh" "$FIX/install.sh"
older="0.0.9"
fixture --version "$older"

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
BIN="$HOMEDIR/.local/bin"; PREFIX="$HOMEDIR/.local/share/majordomus"
STATE="$PREFIX/self-update.json"
release_env() {
  env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN -u MAJORDOMUS_LAUNCHER -u MAJORDOMUS_SELF_UPDATE \
    HOME="$HOMEDIR" MAJORDOMUS_NO_BUILD=1 \
    MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 "$@"
}
release_env sh "$ROOT/site/static/install.sh" --version "v$older" > "$T/install.log" 2>&1 \
  || { echo "    the older release did not install"; tail -20 "$T/install.log"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $older" ] || { echo "    the older release is not what runs"; exit 1; }
cp "$T/latest.good" "$FIX/releases/latest.json"

# --- a fixture repository whose mesh admits this node's key ----------------------------------
SEED="$(printf '5e' ; printf '%062d' 21)"
R="$T/repo"; mkdir -p "$R"
(
  cd "$R" || exit 1
  git init -q .
  git config user.email t@example.com
  git config user.name t
  release_env "$BIN/majordomus" init >/dev/null 2>&1
  printf 'a tracked file\n' > notes.txt
  git add -A && git commit -qm base
) || { echo "    the fixture repository could not be made"; exit 1; }
declare_mesh() { # allowed-key
  mkdir -p "$R/.ai/repo/mesh"
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntailscale:\n  enabled: false\nbroadcast:\n  mode: disabled\ntrust:\n  policy: deny_unknown\n  allow:\n    - %s\n' "$1" > "$R/.ai/repo/mesh/majordomus.yaml"
  git -C "$R" add .ai/repo/mesh/majordomus.yaml && git -C "$R" commit -qm "the mesh" >/dev/null
}
URL=""
serve() { # name
  rm -f "$R/.ai/local/state/mcp/server.json"
  # exec, so that the pid recorded is the server's own and a kill reaches it
  ( cd "$R" && exec env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN -u MAJORDOMUS_LAUNCHER -u MAJORDOMUS_SELF_UPDATE \
      HOME="$HOMEDIR" MAJORDOMUS_NO_BUILD=1 MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      HUB_SEED="$SEED" XDG_STATE_HOME="$T/state" MAJORDOMUS_SELF_UPDATE_INTERVAL=2 \
      "$BIN/majordomus" serve --port 0 --idle 3600 --discovery filesystem --node-seed-env HUB_SEED \
      </dev/null >"$T/$1.log" 2>&1 ) &
  PIDS="$PIDS $!"
  local i=0
  while [ $i -lt 240 ]; do
    URL="$(jq -r '.url // empty' "${R:-/nonexistent}/.ai/local/state/mcp/server.json" 2>/dev/null || true)"
    [ -n "$URL" ] && curl -fsS --max-time 2 "$URL/api/v1/live" >/dev/null 2>&1 && return 0
    sleep 0.5; i=$((i + 1))
  done
  echo "    the $1 server did not answer:"; sed 's/^/      /' "$T/$1.log"; exit 1
}
stop_last() { local p="${PIDS##* }"; kill "$p" 2>/dev/null || true; wait "$p" 2>/dev/null || true; PIDS="${PIDS% *}"; }
mesh() { curl -fsS --max-time 10 "$URL/api/v1/mesh" 2>/dev/null || true; }
# wait_state JQ-EXPRESSION SECONDS — until the follower's record satisfies it
wait_state() {
  local i=0
  while [ $i -lt $(( $2 * 2 )) ]; do
    [ -f "$STATE" ] && jq -e "$1" "$STATE" >/dev/null 2>&1 && return 0
    sleep 0.5; i=$((i + 1))
  done
  echo "    the follower's record never satisfied $1:"; cat "$STATE" 2>/dev/null | sed 's/^/      /'
  echo "    the server's log:"; tail -30 "$T/follow.log" | sed 's/^/      /'; exit 1
}

PLACEHOLDER="$(printf 'ab%.0s' $(seq 32))"
declare_mesh "$PLACEHOLDER"
serve probe
PUB="$(mesh | jq -r '.reason // empty' | grep -oE '[0-9a-f]{64}' | head -n 1 || true)"
stop_last
[ -n "$PUB" ] || { echo "    the unlisted node does not name its key: $(mesh)"; exit 1; }
declare_mesh "$PUB"
rm -f "$STATE"

# --- 1. a dirty checkout defers, and nothing is installed ------------------------------------
printf 'an edit in progress\n' >> "$R/notes.txt"
serve follow
mesh | jq -e '(.active // false) == true' >/dev/null || { echo "    the enrolled node's mesh is not open: $(mesh)"; exit 1; }
wait_state '.step == "hold" and (.reason | contains("uncommitted"))' 60
[ "$("$BIN/majordomus" version)" = "majordomus $older" ] || { echo "    the follower upgraded over a dirty checkout"; exit 1; }
[ ! -d "$PREFIX/versions/$VERSION" ] || { echo "    a deferred round installed v$VERSION"; exit 1; }
out="$(cd "$R" && release_env MAJORDOMUS_SELF_UPDATE_INTERVAL=2 "$BIN/majordomus" self update --check 2>&1 || true)"
for want in "follower     stable every 2s, dirty checkout defers" "last round   hold"; do
  printf '%s\n' "$out" | grep -qF "$want" || { echo "    self update --check did not say '$want': $out"; exit 1; }
done

# --- 2. a clean checkout is upgraded and handed over -----------------------------------------
git -C "$R" checkout -q notes.txt
wait_state ".installed == \"$VERSION\" and (.handover // \"\" | contains(\"serve ensure\"))" 120
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    the follower did not install v$VERSION"; exit 1; }
expect_file "$PREFIX/versions/$VERSION/libexec/majordomus-cli"
curl -fsS --max-time 5 "$URL/api/v1/live" >/dev/null 2>&1 || { echo "    no server answers after the hand-over"; exit 1; }
mesh | jq -e '(.active // false) == true' >/dev/null || { echo "    the mesh closed across the hand-over: $(mesh)"; exit 1; }
[ ! -e "$PREFIX/self-update.lock" ] || { echo "    the apply lock outlived the round"; exit 1; }

# --- 3. what the installer's verification refuses is installed nowhere, and not retried ------
bad="9.9.9"
fixture --version "$bad"
found=0
for f in "$FIX"/*"$bad"*; do
  case "$f" in *.json|*.sh) continue ;; esac
  [ -f "$f" ] && { printf 'tampered' >> "$f"; found=1; }
done
[ "$found" -eq 1 ] || { echo "    the fixture of v$bad carries no archive to tamper with: $(ls "$FIX")"; exit 1; }
wait_state ".failed | index(\"$bad\") != null" 120
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a refused archive moved the installation"; exit 1; }
[ ! -d "$PREFIX/versions/$bad" ] || { echo "    a refused archive left a version tree"; exit 1; }
wait_state ".step == \"hold\" and (.reason | contains(\"failed to install here\"))" 30
echo "    a dirty checkout defers, a clean one is upgraded and handed over, and a refused archive is recorded and never retried"
