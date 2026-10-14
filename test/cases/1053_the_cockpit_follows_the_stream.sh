# majordomus-timeout: 300
# The Cockpit follows the stream (ADR 0133, I2404): /cockpit/monitor renders the first page
# from monitor.stream with what names the reader (?for=) in its own card, carries the cursor
# its script resumes from and the script itself, and the overview and the mesh page link it.
# The script's behaviour is the page's enhancement; what is proven here is what a browser
# without JavaScript gets, which is the whole page.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_MONITOR_FOR

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-1053\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
# GitHub is the network: a gh that is not there is the answer this case expects
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
cd "$R" || exit

URL="$(jq -r '.url' "$R/.ai/local/state/mcp/server.json")"
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train"
expect_exit 0 "$RB" mesh review request "1b, read PR #885 before it lands" --session s1

# ---------------------------------------------------------------- 1. the page, for a reader
page() { curl -fsS -H 'Accept: text/html' "$URL$1"; }
HTML="$(page '/cockpit/monitor?for=1b')" || { echo "    /cockpit/monitor did not answer"; exit 1; }
for want in '<title>Monitor' 'For you \(1\)' 'data-mj-monitor="/api/v1/monitor\?limit=200&amp;for=1b"' \
  'data-mj-monitor-cursor="m' 'asks anyone to review' 'data-mj-for-you="yes"' 'monitor\.js' \
  'majordomus monitor --follow'; do
  grep -qE "$want" <<<"$HTML" || { echo "    /cockpit/monitor lacks /$want/"; echo "$HTML" | head -80; exit 1; }
done

# ---------------------------------------------------------------- 2. without a reader it says how to become one
grep -q 'add ?for=&lt;your names&gt;' <<<"$(page /cockpit/monitor)" \
  || grep -q 'add ?for=<your names>' <<<"$(page /cockpit/monitor)" \
  || { echo "    the page does not say how to be addressed"; exit 1; }

# ---------------------------------------------------------------- 3. the overview and the mesh page link it
grep -q 'href="/cockpit/monitor"' <<<"$(page /cockpit)" || { echo "    the overview does not link the monitor"; exit 1; }
grep -q 'href="/cockpit/monitor"' <<<"$(page /cockpit/mesh)" || { echo "    the mesh page does not link the monitor"; exit 1; }

# ---------------------------------------------------------------- 4. the script the page names is served
curl -fsS "$URL/cockpit/assets/monitor.js" | grep -q "data-mj-monitor" || { echo "    monitor.js is not served"; exit 1; }
