# majordomus-covers: none
# The shared server must not serve a frozen repository.
#
# It used to. The server built one index and one git state when it started and held them
# for its whole life, so a session that committed and then asked the server what HEAD was
# got the HEAD the server had read hours earlier — the same stale answer from the HTTP API,
# from MCP over HTTP and from the Cockpit, with nothing in any of them saying the picture
# was old. Only restarting the process fixed it, and a long-lived session has no reason to
# suspect it needs to.
#
# What is asserted here, with no restart anywhere in it: HEAD is A; a commit happens; the
# same server answers B, through the HTTP API and through an MCP session opened *before*
# the commit — because a session that outlives a commit is the case that was broken. The
# object committed in it is in the index too, which is the second half of the defect: the
# head is not the only thing that was frozen.
#
# `--port 0` throughout: the default port belongs to whatever already serves this machine,
# and this case must never touch it. What it starts, it stops by the pid its own lease
# records; nothing here matches a process by name.
#
# The sibling case 115 is about a different drift — a lease that names another checkout's
# server, or a server of another version. That is the server's own state. This is the
# repository's.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: curl not installed"; exit 0; }

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=warn; export MAJORDOMUS_LOG
lease=.ai/local/state/mcp/server.json
cleanup() {
  if [ -f "$lease" ]; then
    p="$(jq -r '.pid // empty' "$lease" 2>/dev/null || true)"
    case "$p" in ''|*[!0-9]*) ;; *) kill "$p" 2>/dev/null || true ;; esac
  fi
}
trap cleanup EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm base

"$RB" serve ensure --port 0 --idle 120 --wait 40 --format json > "$T/ensure.json" 2> "$T/ensure.err" \
  || { echo "    the server did not come up:"; cat "$T/ensure.json" "$T/ensure.err"; exit 1; }
url="$(jq -r '.url' "$T/ensure.json")"
[ -n "$url" ] && [ "$url" != null ] || { echo "    no address"; cat "$T/ensure.json"; exit 1; }
pid="$(jq -r '.pid // empty' "$lease")"

head_over_http() { curl -fsS "$url/api/v1/repository" | jq -r '.repository.git.head'; }
objects_over_http() { curl -fsS "$url/api/v1/repository" | jq -r '.objects'; }

# ---------------------------------------------------------------- an MCP session, opened now
# It is opened before the commit on purpose: a session that outlives a commit is exactly the
# client the frozen picture lied to, and one opened afterwards would prove nothing.
mcp() {   # mcp ID METHOD PARAMS [SESSION-HEADER-ARGS...]
  local id="$1" method="$2" params="$3"; shift 3
  curl -fsS -X POST "$url/mcp" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    "$@" -d "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"$method\",\"params\":$params}"
}
mcp 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case150","version":"0"}}' \
  -D "$T/mcp.headers" > "$T/mcp.init" || { echo "    MCP initialize failed"; cat "$T/mcp.init"; exit 1; }
sid="$(tr -d '\r' < "$T/mcp.headers" | sed -n 's/^[Mm]cp-[Ss]ession-[Ii]d: //p' | head -n 1)"
[ -n "$sid" ] || { echo "    the MCP endpoint opened no session:"; cat "$T/mcp.headers"; exit 1; }
head_over_mcp() {
  mcp 9 tools/call "{\"name\":\"majordomus_repository\",\"arguments\":{}}" -H "mcp-session-id: $sid" \
    | tr -d '\r' | sed -e 's/^data: //' | jq -rs '[.[] | select(type == "object")][0]
        | (.result.structuredContent // (.result.content[0].text | fromjson)).repository.git.head'
}

# ---------------------------------------------------------------- A, before anything moved
a="$(git rev-parse HEAD)"
[ "$(head_over_http)" = "$a" ] || { echo "    the server did not start on HEAD $a: $(head_over_http)"; exit 1; }
[ "$(head_over_mcp)" = "$a" ] || { echo "    MCP did not start on HEAD $a: $(head_over_mcp)"; exit 1; }
objects_a="$(objects_over_http)"
[ "$objects_a" -gt 0 ] 2>/dev/null || { echo "    the index holds nothing to begin with: $objects_a"; exit 1; }

# ---------------------------------------------------------------- a commit, and no restart
# a declared object, so that the index is asked to change and not only the head. It is a
# copy of one the layer already carries, with its identity changed: nothing here needs to
# know what a rule's schema is, only that this file is one more object than there was.
sample="$(ls .ai/repo/rules/project/*.md 2>/dev/null | head -n 1)"
[ -n "$sample" ] || { echo "    the installed layer carries no project rule to copy"; exit 1; }
sed 's/^id: .*/id: case150.the-server-sees-the-current-tree/' "$sample" \
  > .ai/repo/rules/project/case150-drift.md
git add -A >/dev/null && git commit -qm "a commit made while the server runs" >/dev/null
b="$(git rev-parse HEAD)"
[ "$a" != "$b" ] || { echo "    the commit did not move HEAD"; exit 1; }

# the same process: nothing was restarted, and the lease still records the pid it did
[ "$(jq -r '.pid // empty' "$lease")" = "$pid" ] \
  || { echo "    the server was replaced; this case proves nothing"; exit 1; }

got="$(head_over_http)"
[ "$got" = "$b" ] || {
  echo "    the HTTP API still serves the repository as it was:"
  echo "      HEAD is      $b"
  echo "      the API says $got"
  exit 1
}
got="$(head_over_mcp)"
[ "$got" = "$b" ] || {
  echo "    the MCP session opened before the commit still serves the old repository:"
  echo "      HEAD is      $b"
  echo "      MCP says     $got"
  exit 1
}
objects_b="$(objects_over_http)"
[ "$objects_b" -gt "$objects_a" ] 2>/dev/null || {
  echo "    the index did not follow the commit: $objects_a object(s) before, $objects_b after"
  exit 1
}

# ---------------------------------------------------------------- and again, so it is not once
git commit -q --allow-empty -m "a second commit" >/dev/null
c="$(git rev-parse HEAD)"
got="$(head_over_http)"
[ "$got" = "$c" ] || { echo "    the second commit was not followed: HEAD $c, the API says $got"; exit 1; }

# ---------------------------------------------------------------- the peer is still a peer
# A reload is not a restart: the session that was attached before the commits is still on
# the board, with the identity it initialised under.
peers="$(mcp 10 tools/call '{"name":"majordomus_peers","arguments":{}}' -H "mcp-session-id: $sid" \
  | tr -d '\r' | sed -e 's/^data: //' | jq -rs '[.[] | select(type == "object")][0]
      | (.result.structuredContent // (.result.content[0].text | fromjson)) | tostring')"
printf '%s' "$peers" | grep -q 'case150' \
  || { echo "    the peer board lost the session across the reload:"; echo "$peers"; exit 1; }

expect_exit 0 "$RB" serve stop
expect_grep '^stopped '

echo "    a commit moves the server: $a -> $b -> $c over HTTP and over an MCP session that never restarted"
