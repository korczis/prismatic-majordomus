# majordomus-covers: none
# A brief is bounded, and it never builds (I4390, ADR 0224).
#
# The brief is asked on every status-line tick of every session, so an owner that hangs must
# not hang it, and a status line is not where a compiler starts. Every section runs under its
# own budget and the composition under six seconds; every call of the forge is bounded inside
# its section; and the only children are git, gh and ps.
#
# What it proves, over a real server whose forge hangs:
#   1  `majordomus brief` answers within 8 s although every `gh` call sleeps 30 s
#   2  neither the server nor the command line ever starts cargo or rustc
#   3  the CI pool and the session's work read unread or partial, and say what took too long
#   4  every other section is read, none of them past its budget
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
export MAJORDOMUS_SHARE="$ROOT/share"
fail() { printf '    %s\n' "$*"; exit 1; }

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT
R="$T/repo"; mkdir -p "$R"
(
  cd "$R" || exit
  git init -q . && git config user.email t@example.com && git config user.name t
  "$MJ" init >/dev/null && "$MJ" update >/dev/null
  git add -A && git commit -qm base
)
mkdir -p "$T/bin" "$T/state"
# a forge that hangs, and compilers that record that they were started and refuse
printf '#!/bin/sh\nsleep 30\n' > "$T/bin/gh"
for c in cargo rustc; do
  printf '#!/bin/sh\necho "%s $*" >> "%s/compilers.log"\nexit 99\n' "$c" "$T" > "$T/bin/$c"
done
chmod +x "$T/bin/gh" "$T/bin/cargo" "$T/bin/rustc"
( cd "$R" && PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# ---------------------------------------------------------------- 1  bounded
t0="$(date +%s)"
json="$(cd "$R" && PATH="$T/bin:$PATH" MAJORDOMUS_PROVIDER_SESSION=case-4405 "$RB" brief --this-checkout --format json 2>/dev/null)" \
  || fail "brief failed"
took=$(( $(date +%s) - t0 ))
[ "$took" -le 8 ] || fail "the brief took $took s with a hanging forge"
[ "$(printf '%s' "$json" | jq -r .answered_by.process)" = leaseholder ] || fail "the brief was not the leaseholder's"

# ---------------------------------------------------------------- 2  never builds
[ ! -s "$T/compilers.log" ] || { cat "$T/compilers.log"; fail "a compiler was started"; }

# ---------------------------------------------------------------- 3  the slow owners say so
for s in ci work; do
  printf '%s' "$json" | jq -e --arg s "$s" '.[$s].observed.verdict != "current" and (.[$s].summary | test("took"))' >/dev/null \
    || { printf '%s' "$json" | jq --arg s "$s" '.[$s] | {state, summary, observed}'; fail "$s does not say the forge took too long"; }
done

# ---------------------------------------------------------------- 4  the rest is read
for s in session peers mesh integration machine release derive; do
  printf '%s' "$json" | jq -e --arg s "$s" '.[$s].summary | test("took >") | not' >/dev/null \
    || fail "$s ran past its budget: $(printf '%s' "$json" | jq -r --arg s "$s" '.[$s].summary')"
done
printf '%s' "$json" | jq -e '.machine.state == "read" and .session.state != "unread" and .peers.state != "unread"' >/dev/null \
  || fail "a section that asks no forge was not read: $(printf '%s' "$json" | jq -c '{machine: .machine.state, session: .session.state, peers: .peers.state}')"
exit 0
