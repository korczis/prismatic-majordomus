# majordomus-covers: capture
# majordomus-negative: capture
# The session lifecycle of a client that has no provider hooks at all (ADR 0043), driven end
# to end over the real transport, and the provider capability matrix the whole thing is
# declared by.
#
# Two things were untrue on 2026-09-11 and both are asserted against here.
#
#   1. `capture status` reported one of the six providers this distribution declares. The
#      loop was over the providers with a line in the prompt adapter table, so the state
#      `unsupported` was unreachable for any provider a person could name, and a reader
#      asking about Codex got silence — which reads as no, and had become wrong: Codex CLI
#      and Gemini CLI had both shipped SessionStart, SessionEnd and a pre-model prompt hook.
#
#   2. Episode continuity was Claude Code's alone. Every other client — anything that speaks
#      MCP and fires no hooks — had no boundary drawn below it at all.
#
# Every assertion about the lifecycle drives `bin/majordomus-mcp`, the launcher a client
# configuration names, with real JSON-RPC frames on its stdin, for the reason cases 54 and
# 111 give: a lifecycle that only works when a test calls the library proves nothing about
# the client somebody actually closed.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -n "${MAJORDOMUS_BIN:-}" ] || command -v cargo >/dev/null 2>&1 || { echo "    skip: no cargo and no MAJORDOMUS_BIN"; exit 0; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
LAUNCHER="$ROOT/bin/majordomus-mcp"
[ -x "$LAUNCHER" ] || { echo "    bin/majordomus-mcp is missing or not executable"; exit 1; }

# The suite may itself be running inside a provider session — this repository is developed in
# one — and a case that drives named episodes must not silently be inside a third.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

S="$(mktemp -d "${TMPDIR:-/tmp}/mj200.XXXXXX")"; trap 'rm -rf "$S"; exec 3>&- 2>/dev/null' EXIT
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }

# THE TRAP THIS CASE FELL INTO FIRST. `bin/majordomus-mcp` starts a server over the
# disposable repository, and the episode driver resolves the shell tool from *that* root:
# `<root>/bin/majordomus`, then `.majordomus/bin/majordomus`, then the path. A disposable
# repository has neither of the first two, so without this line the driver ran whichever
# `majordomus` the developer's PATH happened to put first — in this repository, the primary
# checkout's — and the case measured another tree's lib/. The same class as the
# MAJORDOMUS_SHARE trap, reached through PATH instead.
PATH="$ROOT/bin:$PATH"; export PATH

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "case 200 fixture" >/dev/null 2>&1 || true

OPEN=.ai/local/state/sessions-open
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
hello() { printf '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"%s","version":"0"}}' "$1"; }
call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }

# ---------------------------------------------------------------- the capability matrix
# THE FIRST REGRESSION. Every provider the declaration names is reported, for both aspects,
# with the capability it declares and the citation behind it.
"$MJ" --json capture status > "$S/status.json" 2>"$S/status.err" \
  || { echo "    capture status failed"; cat "$S/status.err"; exit 1; }
declared="$(sed -n 's/^  \([a-z][a-z0-9-]*\):$/\1/p' "$ROOT/share/providers.yaml" | sort -u)"
must "the declaration names no providers; this case would assert nothing" [ -n "$declared" ]
for p in $declared; do
  for a in prompt session; do
    jq -e --arg p "$p" --arg a "$a" \
      '[.providers[] | select(.provider == $p and .aspect == $a)] | length == 1' \
      "$S/status.json" >/dev/null \
      || { echo "    capture status does not report $p:$a — the regression that started ADR 0043"; jq -r '.providers[] | "\(.provider) \(.aspect) \(.state)"' "$S/status.json" | sed 's/^/    | /'; exit 1; }
    # Every cell carries its citation. A capability with no evidence cannot be re-verified
    # and rots in whichever direction the world moves; that is the rule
    # project.a-provider-capability-cites-its-evidence, and this is it holding.
    jq -e --arg p "$p" --arg a "$a" \
      '[.providers[] | select(.provider == $p and .aspect == $a) | .evidence | select(length > 24)] | length == 1' \
      "$S/status.json" >/dev/null \
      || { echo "    $p:$a carries no citation for its declared capability"; exit 1; }
  done
done

# HONESTY. `unsupported` is a statement about the provider and is reserved for one that
# cannot do the thing. Codex and Gemini document complete hook systems this distribution
# ships no adapter for, and the word for that is `unadapted` — a gap in this tool, named as
# one. Reporting them `unsupported` would be this tool making a false claim about somebody
# else's product, which is exactly what it was doing by omission.
for p in codex gemini; do
  for a in prompt session; do
    state="$(jq -r --arg p "$p" --arg a "$a" '.providers[] | select(.provider == $p and .aspect == $a) | .state' "$S/status.json")"
    must "$p:$a is '$state'; a provider that documents the event and has no adapter here is 'unadapted', never 'unsupported'" \
      [ "$state" = unadapted ]
  done
done
# ... and a provider that genuinely cannot do it says so, with the search behind the claim.
must "bb:session is not unsupported" \
  [ "$(jq -r '.providers[] | select(.provider == "bb" and .aspect == "session") | .state' "$S/status.json")" = unsupported ]

# The gate that refuses a declaration without its citation, and the proof that it can fail:
# a check nobody has ever seen fail is a check nobody knows the meaning of.
expect_exit 0 "$ROOT/scripts/provider-evidence-check"
# The negative, over a COPY of the distribution and never the repository's own declaration:
# a case that edited the tree it is checking would leave it broken on any crash, and a
# concurrent case would read the damage as its own. The gate reads
# ${MAJORDOMUS_SHARE:-<root>/share} for exactly that reason.
cp -R "$ROOT/share" "$S/share"
awk '/^      prompts_evidence: / && !seen { sub(/:.*/, ": \"\""); seen = 1 } { print }' \
  "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
rc=0; MAJORDOMUS_SHARE="$S/share" "$ROOT/scripts/provider-evidence-check" >"$S/gate.out" 2>&1 || rc=$?
must "the evidence gate passed a declaration with an empty citation (exit $rc)" [ "$rc" = 10 ]
grep -qF 'prompts_evidence is empty' "$S/gate.out" \
  || { echo "    the gate failed for some other reason than the missing citation:"; sed 's/^/    | /' "$S/gate.out"; exit 1; }

# ---------------------------------------------------------------- initialize opens nothing
# A client that reads the layer is a peer and not a worker with a record. This is also what
# keeps case 90's guarantee true — serving changes the repository not at all — and it is why
# `attach` is a call the client makes rather than something `initialize` does behind it.
before="$(git status --porcelain; git ls-files -s | shasum -a 256)"
{ req 1 initialize "$(hello case200-reader)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_episodes '{}'
} > "$S/reader.in"
rc=0; "$LAUNCHER" --http-port 0 < "$S/reader.in" > "$S/reader.out" 2> "$S/reader.err" || rc=$?
must "the reader session exited $rc" [ "$rc" = 0 ]
sed -n 2p "$S/reader.out" | jq -e '.result.structuredContent.open == 0 and (.result.structuredContent.mine | not)' >/dev/null \
  || { echo "    initialize alone opened an episode"; sed -n 2p "$S/reader.out"; exit 1; }
must "reading the layer opened an episode in the store" [ ! -d "$OPEN" ] || [ -z "$(ls -A "$OPEN" 2>/dev/null)" ]
after="$(git status --porcelain; git ls-files -s | shasum -a 256)"
must "a client that only read the layer changed the repository" [ "$before" = "$after" ]

# ---------------------------------------------------------------- attach, activity, close
# The whole lifecycle in one connection: attach names the work, the episode reaches the
# repository's own store through the same command a provider hook runs, the client's traffic
# is its heartbeat, and a deliberate detach closes it into a record.
EP=thread-200-alpha
{ req 1 initialize "$(hello case200-worker)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_session_attach "{\"external_id\":\"$EP\"}"
  call 3 majordomus_episodes '{}'
  call 4 majordomus_session_detach '{}'
  call 5 majordomus_episodes '{}'
} > "$S/worker.in"
rc=0; "$LAUNCHER" --http-port 0 < "$S/worker.in" > "$S/worker.out" 2> "$S/worker.err" || rc=$?
must "the worker session exited $rc" [ "$rc" = 0 ]

sed -n 2p "$S/worker.out" | jq -e --arg e "$EP" \
  '.result.structuredContent.resumed == false
   and .result.structuredContent.episode.external_id == $e
   and .result.structuredContent.episode.state == "open"
   and .result.structuredContent.episode.provider == "generic"
   and (.result.structuredContent.reattach_grace_seconds > 0)' >/dev/null \
  || { echo "    attach did not open an episode"; sed -n 2p "$S/worker.out"; exit 1; }
# The episode reached the repository, through `capture session` and not through a second
# writer of the record: the field carries what that command said, verbatim.
sed -n 2p "$S/worker.out" | jq -e '.result.structuredContent.episode.repository | test("opened|kept")' >/dev/null \
  || { echo "    the repository episode was not opened; the board recorded:"; sed -n 2p "$S/worker.out" | jq -r '.result.structuredContent.episode.repository' | sed 's/^/    | /'; exit 1; }

sed -n 3p "$S/worker.out" | jq -e --arg e "$EP" '.result.structuredContent.mine.external_id == $e and .result.structuredContent.open == 1' >/dev/null \
  || { echo "    the connection does not hold the episode it attached"; sed -n 3p "$S/worker.out"; exit 1; }
sed -n 5p "$S/worker.out" | jq -e '.result.structuredContent.open == 0 and .result.structuredContent.detached == 0' >/dev/null \
  || { echo "    detach did not close the episode"; sed -n 5p "$S/worker.out"; exit 1; }

# A deliberate detach closes the episode into an immutable record, exactly as a provider's
# own end event does. `capture status` said `connection: verified` before this ran; this is
# what makes that word true.
must "a deliberate detach left the episode open in the store" [ ! -f "$OPEN/$EP.yaml" ]
records="$(find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' ')"
must "the closed episode wrote no session record" [ "$records" -ge 1 ]

# ---------------------------------------------------------------- crash and recovery
# THE SECOND REGRESSION, and the reason a peer id cannot be the episode's identity. A client
# that is killed comes back under a *different* peer id and must find *its own* episode.
#
# The server is killed with its client here, which is the honest simulation: a stdio client
# and its server die together, so recovery cannot come from the board's memory. It comes
# from the repository's store, through `session start --if-open keep`, which is what makes an
# episode survive the process that drew it.
EP2=thread-200-crash
{ req 1 initialize "$(hello case200-crasher)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_session_attach "{\"external_id\":\"$EP2\"}"
} > "$S/crash.in"
mkfifo "$S/hold"
# stdin stays open, so the launcher does not reach its own shutdown path: this process is
# killed where it stands, which is what a crash is.
( cat "$S/crash.in"; cat "$S/hold" ) | "$LAUNCHER" --http-port 0 > "$S/crash.out" 2> "$S/crash.err" &
crashed=$!
i=0; until [ "$(wc -l < "$S/crash.out" 2>/dev/null | tr -d ' ')" -ge 2 ]; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the crashing client never attached"; cat "$S/crash.err"; exit 1; }
  sleep 0.1
done
must "the crashing client's episode is not in the store" [ -f "$OPEN/$EP2.yaml" ]
opened="$(sed -n 's/^session_id: //p' "$OPEN/$EP2.yaml" | head -n 1)"
must "the episode in the store carries no session id" [ -n "$opened" ]
# kill -9 the whole client: no end event, no close_all, nothing tidied. Exactly the episode
# this subsystem exists for — the one that ended in a crash.
kill -9 "$crashed" 2>/dev/null || true
exec 3> "$S/hold"; exec 3>&-
wait "$crashed" 2>/dev/null || true
must "the crash closed the episode; a crashed episode must stay open to be resumed" [ -f "$OPEN/$EP2.yaml" ]

# ... and the client comes back. A new process, a new server, a new peer id, the same
# external identity — and the same episode, not a second one.
{ req 1 initialize "$(hello case200-crasher)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_session_attach "{\"external_id\":\"$EP2\"}"
  call 3 majordomus_session_detach '{}'
} > "$S/recover.in"
rc=0; "$LAUNCHER" --http-port 0 < "$S/recover.in" > "$S/recover.out" 2> "$S/recover.err" || rc=$?
must "the recovering session exited $rc" [ "$rc" = 0 ]
sed -n 2p "$S/recover.out" | jq -e '.result.structuredContent.episode.repository | test("kept|already open")' >/dev/null \
  || { echo "    the reconnecting client did not recover its own episode:"; sed -n 2p "$S/recover.out" | jq -r '.result.structuredContent.episode.repository' | sed 's/^/    | /'; exit 1; }
must "recovery opened a second episode instead of resuming the first" \
  [ ! -f "$OPEN/$EP2.yaml" ]

# ---------------------------------------------------------------- within one server
# Inside one server the board holds the association, and a detach that is not deliberate
# leaves the episode recoverable rather than closing it. Two clients, so that the server
# outlives the one that leaves — the shape case 90 established.
EP3=thread-200-reattach
mkfifo "$S/in1"
"$LAUNCHER" --http-port 0 < "$S/in1" > "$S/out1.txt" 2> "$S/err1.txt" & holder=$!
exec 3> "$S/in1"
i=0; until grep -q 'listening on http://' "$S/err1.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the holding client's server never listened"; cat "$S/err1.txt"; exit 1; }
  sleep 0.1
done
req 1 initialize "$(hello case200-holder)" >&3

{ req 1 initialize "$(hello case200-first-visit)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_session_attach "{\"external_id\":\"$EP3\"}"
} > "$S/visit1.in"
rc=0; "$LAUNCHER" < "$S/visit1.in" > "$S/visit1.out" 2> "$S/visit1.err" || rc=$?
must "the first visit exited $rc" [ "$rc" = 0 ]
grep -q 'bridging this stdio session to it' "$S/visit1.err" || { echo "    the first visit did not bridge to the holder's server"; cat "$S/visit1.err"; exit 1; }

# It has gone. The episode is detached and waiting, not closed: a dropped connection is the
# most ordinary thing that happens to work in progress and is not the end of it.
{ req 1 initialize "$(hello case200-second-visit)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  call 2 majordomus_episodes '{}'
  call 3 majordomus_session_attach "{\"external_id\":\"$EP3\"}"
  call 4 majordomus_session_detach '{}'
} > "$S/visit2.in"
rc=0; "$LAUNCHER" < "$S/visit2.in" > "$S/visit2.out" 2> "$S/visit2.err" || rc=$?
must "the second visit exited $rc" [ "$rc" = 0 ]
sed -n 2p "$S/visit2.out" | jq -e --arg e "$EP3" \
  '[.result.structuredContent.episodes[] | select(.external_id == $e and .state == "detached")] | length == 1' >/dev/null \
  || { echo "    a lost connection closed the episode instead of detaching it"; sed -n 2p "$S/visit2.out"; exit 1; }
# the same episode, a different connection: attachments went up and opened_at did not move
sed -n 3p "$S/visit2.out" | jq -e --arg e "$EP3" \
  '.result.structuredContent.resumed == true
   and .result.structuredContent.episode.external_id == $e
   and .result.structuredContent.episode.attachments == 2' >/dev/null \
  || { echo "    the reconnecting client got a new episode rather than its own"; sed -n 3p "$S/visit2.out"; exit 1; }

exec 3>&-
wait "$holder" 2>/dev/null || true
