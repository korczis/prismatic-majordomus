# majordomus-timeout: 300
# `majordomus monitor` answers one page of the stream from a running server (ADR 0133,
# I2400-I2402): a session that re-announces itself unchanged is counted as noise and shown
# once, a changed intent is news, a review asked of the reader is listed first as "for you",
# and a source that cannot be read is named with its reason. A bad cursor and an unknown
# source are refused with exit 10, and the page's cursor resumes with nothing repeated.
#
# One checkout, one node identity of its own, one `majordomus serve` on loopback with the
# mesh declared. The fold's every rule is unit-tested in src/monitor; this is the operator's
# path through the server and the command line.
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
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-1050\n' >> .ai/repo/mesh/majordomus.yaml
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

# ---------------------------------------------------------------- 1. a session said three times is one signal, a change is another
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train"
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train" --head abc
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train"
expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "release 0.21.0"
expect_exit 0 "$RB" mesh review request "1b, read PR #885 before it lands" --session s1

# stdout alone: expect_exit folds stderr (the server discovery's log lines) into its capture
PAGE="$("$RB" monitor --for 1b --sources mesh,executor,github --format json 2>/dev/null)" \
  || { echo "    monitor failed"; exit 1; }
count() { jq -r "$1" <<<"$PAGE"; }
[ "$(count '[.signals[] | select(.kind == "session_joined" and .session == "s1")] | length')" = 1 ] \
  || { echo "    s1 joined more or less than once"; echo "$PAGE" | head -60; exit 1; }
[ "$(count '[.signals[] | select(.kind == "session_changed")] | length')" = 1 ] \
  || { echo "    the changed intent is not one signal"; exit 1; }
[ "$(count '.suppressed >= 2')" = true ] || { echo "    the re-announcements were not folded away"; exit 1; }
[ "$(count '[.signals[] | select(.kind == "review_requested" and .for_you and .severity == "action")] | length')" = 1 ] \
  || { echo "    the review asked of 1b is not for 1b"; exit 1; }
[ "$(count '.for_you')" = 1 ] || { echo "    for_you counts more than the review"; exit 1; }
[ "$(count '.sources[] | select(.source == "executor") | .state')" = absent ] \
  || { echo "    an empty executor trail is not reported absent"; exit 1; }
[ "$(count '.sources[] | select(.source == "github") | .state')" = error ] \
  || { echo "    a failing gh is not reported"; exit 1; }
count '.sources[] | select(.source == "github") | .reason' | grep -q 'gh pr failed: gh: not here in this case' \
  || { echo "    the github reason is not gh's own"; exit 1; }

# ---------------------------------------------------------------- 2. the text page: for you first, noise counted, sources, how to go on
expect_exit 0 "$RB" monitor --for 1b --sources mesh
expect_grep '^For you \(1\)'
expect_grep '> !! 3d \(s1\) asks anyone to review .*: 1b, read PR #885'
expect_grep 'folded away as noise'
expect_grep 'sources  mesh ok · executor skipped \(not asked for\) · github skipped \(not asked for\)'
expect_grep "next     majordomus monitor --after 'm"

# ---------------------------------------------------------------- 3. the cursor resumes with nothing repeated
CURSOR="$(jq -r '.cursor' <<<"$PAGE")"
NEXT="$("$RB" monitor --after "$CURSOR" --sources mesh --format json 2>/dev/null)"
[ "$(jq '[.signals[] | select(.session == "s1")] | length' <<<"$NEXT")" = 0 ] \
  || { echo "    a resumed page repeated s1"; echo "$NEXT"; exit 1; }
expect_exit 0 "$RB" mesh session open --session s2 --worker cd --intent "the executor lane"
NEXT="$("$RB" monitor --after "$CURSOR" --sources mesh --format json 2>/dev/null)"
[ "$(jq '[.signals[] | select(.kind == "session_joined" and .session == "s2")] | length' <<<"$NEXT")" = 1 ] \
  || { echo "    the new session is not on the resumed page"; echo "$NEXT"; exit 1; }

# ---------------------------------------------------------------- 4. refusals are typed, exit 10
expect_exit 10 "$RB" monitor --after 'm12'
expect_grep 'invalid_input'
expect_exit 10 "$RB" monitor --sources mesh,slack
expect_grep 'unknown source .?"slack'
