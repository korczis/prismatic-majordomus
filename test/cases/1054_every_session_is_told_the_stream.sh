# majordomus-timeout: 300
# Every session is told the stream (ADR 0133, I2405): `majordomus context` carries a WHAT
# HAPPENED section from `majordomus monitor`, with what names the reader
# ($MAJORDOMUS_MONITOR_FOR) first; the session-start briefing prints it as a Monitor line
# under the mesh; and the provider bootstrap this repository renders for every agent names
# the command. With no server, the section is not written at all.
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
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-1054\n' >> .ai/repo/mesh/majordomus.yaml
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

expect_exit 0 "$RB" mesh session open --session s1 --worker 3d --intent "the runtime train"
expect_exit 0 "$RB" mesh review request "1b, read PR #885 before it lands" --session s1

# ---------------------------------------------------------------- 1. context: what happened, for the reader first
CTX="$(MAJORDOMUS_BIN="$RB" MAJORDOMUS_MONITOR_FOR=1b "$MJ" context 2>/dev/null)"
grep -q '^## WHAT HAPPENED' <<<"$CTX" || { echo "    context has no WHAT HAPPENED section"; echo "$CTX" | head -40; exit 1; }
grep -qE '^[0-9]+ signal\(s\) in the last hour, 1 for you' <<<"$CTX" || { echo "    the summary does not count the review for 1b"; echo "$CTX" | sed -n '/WHAT HAPPENED/,/^$/p'; exit 1; }
grep -q '^  for you    3d (s1) asks anyone to review' <<<"$CTX" || { echo "    the review is not listed for 1b"; exit 1; }

# ---------------------------------------------------------------- 2. the briefing prints it under the mesh
# shellcheck source=/dev/null
BRIEF="$(MJ_LIB_DIR="$ROOT/lib" MJ_ROOT="$R" bash -c '. "$MJ_LIB_DIR/common.sh" 2>/dev/null; . "$MJ_LIB_DIR/derive.sh"; mj_derive_briefing "ready http://x" "active — test" "3 signal(s) in the last hour, 1 for you"' 2>/dev/null)"
grep -q '^Mesh: active — test' <<<"$BRIEF" && grep -q '^Monitor: 3 signal(s) in the last hour, 1 for you' <<<"$BRIEF" \
  || { echo "    the briefing does not print the Monitor line under the mesh"; echo "$BRIEF" | head -30; exit 1; }

# ---------------------------------------------------------------- 3. no server, no section
kill $PIDS 2>/dev/null; wait 2>/dev/null; PIDS=""
CTX="$(MAJORDOMUS_BIN="$RB" "$MJ" context 2>/dev/null)"
grep -q '^## WHAT HAPPENED' <<<"$CTX" && { echo "    a context without a server grew a monitor section"; exit 1; }

# ---------------------------------------------------------------- 4. every agent's bootstrap names the command
for f in AGENTS.md CLAUDE.md; do
  grep -q 'monitor --follow --for' "$ROOT/$f" \
    || { echo "    $f does not name majordomus monitor --follow"; exit 1; }
done
