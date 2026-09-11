# majordomus-exclusive: measures the wall time of entering a repository against a policy budget; a busy pool would add the noise it measures
# majordomus-covers: none
#
# Entering the repository brings its runtime up (ADR 0043), and does it without becoming
# something a person waits for.
#
# The doctrine this proves was changed deliberately. Until ADR 0043 the rule read "entry by
# a shell reports and does not serve", and what that cost was the whole requirement: a person
# who opened a terminal here got a banner and no runtime — no Cockpit, no API, no peer board,
# so no way for the other workers on the same machine to see that they were not alone — and
# the remedy was a command to remember, which is the thing the design is against.
#
# What a script can decide about that is here. That it converges on *one* runtime when
# several processes enter at once is test/cases/191, which runs real concurrent processes;
# that the entry file and the gate still agree is test/cases/118; that the agent's door
# converges is test/cases/108.
#
#   0. --no-runtime enters without starting anything, and still exports the environment
#   1. a cold entry brings a runtime up and RETURNS, rather than waiting for it to bind —
#      which is why its budget is barely larger than a warm entry's: a fork and an exec
#   2. a warm entry starts nothing, and repeated entries change no file at all: no restart,
#      no episode, no ledger line, no rewritten state
#   3. both stay inside the budget the policy declares, which is a key like any other
#   4. the switch and MAJORDOMUS_RUNTIME=off each stop it, and say nothing when they do
#   5. a stale executable is named and ensures nothing: a server started from stale code
#      answers with a tree that is no longer there
#   6. a server this entry starts inherits none of the caller's file descriptors, so a
#      caller that waits on a pipe of its own is not held for the server's whole life
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT
lease=.ai/local/state/mcp/server.json

# The budgets are read from the policy, never defaulted here: a case that carried its own
# number would be a second declaration of the budget, and the two would drift.
budget_of() { sed -n "s/^    $1: *\([0-9][0-9]*\).*/\1/p" .ai/repo/policy.yaml | head -n 1; }
WARM_BUDGET="$(budget_of enter_ms)"
COLD_BUDGET="$(budget_of enter_cold_ms)"
[ -n "$WARM_BUDGET" ] || { echo "    the policy declares no benchmark.budget.enter_ms"; exit 1; }
[ -n "$COLD_BUDGET" ] || { echo "    the policy declares no benchmark.budget.enter_cold_ms"; exit 1; }

# Milliseconds, without bash 5's EPOCHREALTIME (macOS ships bash 3.2) and without a
# subprocess per reading that would be most of what is measured on a short run.
now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
enter() { "$RB" env enter --repo "$T" --shell direnv --no-banner --no-bridge "$@"; }
# With one variable in the environment of the executable itself. `VAR=x enter` would not do
# it: a variable assignment prefixed to a *function* call is a shell variable for the
# duration of the function and is not exported to what the function runs, so the executable
# would never see it and every assertion below would pass for the wrong reason.
enter_env() { local v="$1"; shift; env "$v" "$RB" env enter --repo "$T" --shell direnv --no-banner --no-bridge "$@"; }

# A cold entry returns *before* the lease exists. That is the claim, not an accident: the
# server it started writes its own lease as it elects, a moment after the entry has handed
# the shell its environment and gone. So every assertion about a lease appearing waits for
# it, bounded, and every assertion about one NOT appearing waits long enough that a spawn
# would have shown (project.every-wait-is-bounded — a wait without a predicate and a bound
# is a hang with a nice name).
await_lease() {
  local i=0
  while [ "$i" -lt 100 ]; do [ -f "$lease" ] && return 0; i=$((i+1)); sleep 0.1; done
  return 1
}
no_lease_appears() {   # what, after a settle long enough for a spawn to have written one
  sleep 2
  [ ! -f "$lease" ] && return 0
  echo "    $1"; cat "$lease"; return 1
}

# --- 0. one entry with the runtime off, which does two things at once
#
# It proves that --no-runtime is the refusal it claims to be — nothing is started and the
# environment is exported anyway — and it warms the fast resolver's cache, so that the cold
# entry measured next is measuring what ADR 0043 added and not what was always there.
#
# The first resolution of a checkout nobody has entered yet costs about a second and a half
# on a loaded machine, all of it in `environment::resolve` writing its cache for the first
# time. That cost is the same before this decision and after; the runtime half contributes
# nothing to it. A budget over it would be a budget over the resolver wearing entry's name,
# which is why the policy says so and why this line is here rather than a larger number in
# the policy.
[ ! -f "$lease" ] || { echo "    a fresh repository has a lease before anything entered it"; exit 1; }
out="$(enter --no-runtime 2>"$T/norun.err")" || { echo "    entry --no-runtime failed"; cat "$T/norun.err"; exit 1; }
no_lease_appears "--no-runtime started a server" || exit 1
printf '%s\n' "$out" | grep -qE '^export MAJORDOMUS_ROOT=' || { echo "    --no-runtime cost the environment too"; exit 1; }

# --- 1. cold: nothing is serving, and entering starts one without waiting for it
t0="$(now_ms)"; out="$(enter 2>"$T/cold.err")"; code=$?; cold=$(( $(now_ms) - t0 ))
[ "$code" = 0 ] || { echo "    a cold entry exited $code; entering a directory must not fail"; cat "$T/cold.err"; exit 1; }
await_lease || { echo "    a cold entry left no lease; nothing was started"; cat "$T/cold.err"; exit 1; }
grep -q 'a server is starting' "$T/cold.err" || { echo "    a cold entry said nothing about the runtime it started:"; cat "$T/cold.err"; exit 1; }
# It returned before the server was ready — and, usually, before the server had even written
# its lease. That is the claim, and what holds it is the elapsed time against the cold budget,
# which is barely larger than a warm entry's: a wait for a bind could not fit inside it.
printf '%s\n' "$out" | grep -qE '^export MAJORDOMUS_ROOT=' || { echo "    a cold entry exported no environment:"; printf '%s\n' "$out"; exit 1; }
echo "    cold entry: ${cold} ms of ${COLD_BUDGET} ms"
[ "$cold" -le "$COLD_BUDGET" ] || { echo "    a cold entry took ${cold} ms, over the budget of ${COLD_BUDGET} ms (policy benchmark.budget.enter_cold_ms)"; exit 1; }

# wait for the server the cold entry started, so that what follows measures a warm entry.
# Bounded, with a predicate, and a recovery path that says what it saw: an unbounded wait
# here would be a hang with a nice name (project.every-wait-is-bounded).
ready=0
for _ in $(seq 1 100); do
  "$RB" serve status --repo "$T" --checkouts this 2>/dev/null | grep -q '^standing   ready' && { ready=1; break; }
  sleep 0.2
done
[ "$ready" = 1 ] || { echo "    the server a cold entry started never became ready:"; "$RB" serve status --repo "$T" --checkouts this 2>&1; cat .ai/local/state/mcp/server.log 2>/dev/null; exit 1; }
url="$("$RB" serve status --repo "$T" --checkouts this 2>/dev/null | sed -n 's/^this  *\(http:[^ ]*\).*/\1/p')"
[ -n "$url" ] || { echo "    a ready server published no address"; exit 1; }

# --- 2. warm: nothing is started, nothing is rewritten, and the address is exported
#
# The whole local state is fingerprinted before and after. "It did not restart the server"
# is easy to assert and easy to satisfy accidentally; "it changed no file" is the claim that
# actually covers a fake episode, a ledger line, a rewritten cache and a touched lease at
# once, and it is the one a person repeating `cd .. && cd repo` would notice breaking.
# LC_ALL=C, so that the order does not depend on the machine (project.canonical-order):
# this is compared against itself across five entries, and a locale that collates differently
# between two readings would report a change nobody made.
fingerprint() { find .ai/local -type f -exec ls -ld {} \; 2>/dev/null | LC_ALL=C sort; }
before="$(fingerprint)"
pid_before="$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")"

warm_total=0
for i in 1 2 3 4 5; do
  t0="$(now_ms)"; out="$(enter 2>"$T/warm.err")" || { echo "    a warm entry failed:"; cat "$T/warm.err"; exit 1; }
  warm=$(( $(now_ms) - t0 )); warm_total=$(( warm_total + warm ))
  [ -s "$T/warm.err" ] && { echo "    a warm entry said something; entering a healthy repository is silent:"; cat "$T/warm.err"; exit 1; }
done
warm=$(( warm_total / 5 ))
echo "    warm entry: ${warm} ms of ${WARM_BUDGET} ms (mean of 5)"
[ "$warm" -le "$WARM_BUDGET" ] || { echo "    a warm entry took ${warm} ms, over the budget of ${WARM_BUDGET} ms (policy benchmark.budget.enter_ms)"; exit 1; }

printf '%s\n' "$out" | grep -q "export MAJORDOMUS_URL='$url'" || { echo "    a warm entry did not export the address of the server it found:"; printf '%s\n' "$out"; exit 1; }
pid_after="$(sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lease")"
[ "$pid_before" = "$pid_after" ] || { echo "    five warm entries restarted the server ($pid_before -> $pid_after)"; exit 1; }
after="$(fingerprint)"
[ "$before" = "$after" ] || { echo "    five warm entries changed the local state:"; diff <(printf '%s\n' "$before") <(printf '%s\n' "$after") | sed 's/^/    /'; exit 1; }

# --- 3. the switch stops it, and says nothing when it does
"$RB" serve stop --repo "$T" >/dev/null 2>&1
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
out="$(enter 2>"$T/off.err")" || { echo "    entry failed with the switch off"; cat "$T/off.err"; exit 1; }
no_lease_appears "a server was started with session.ensure_server_on_start false" || exit 1
[ -s "$T/off.err" ] && { echo "    entry with the switch off said something:"; cat "$T/off.err"; exit 1; }
printf '%s\n' "$out" | grep -qE '^export MAJORDOMUS_ROOT=' || { echo "    the switch being off cost the environment too"; exit 1; }
reset_policy; "$MJ" update >/dev/null

# --- 4. MAJORDOMUS_RUNTIME=off is the same refusal for one person's shell; only that value
[ ! -f "$lease" ] || { echo "    a lease survived the policy reset"; exit 1; }
enter_env MAJORDOMUS_RUNTIME=off >/dev/null 2>"$T/envoff.err" || { echo "    entry failed with MAJORDOMUS_RUNTIME=off"; cat "$T/envoff.err"; exit 1; }
no_lease_appears "MAJORDOMUS_RUNTIME=off started a server anyway" || exit 1
# A typo must not silently turn the runtime off: anything that is not exactly `off` is auto.
enter_env MAJORDOMUS_RUNTIME=of >/dev/null 2>"$T/typo.err" || { echo "    entry failed with a misspelt MAJORDOMUS_RUNTIME"; cat "$T/typo.err"; exit 1; }
await_lease || { echo "    MAJORDOMUS_RUNTIME=of was read as off; a typo in a shell profile must not stop a repository coming up"; exit 1; }
"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true
rm -f "$lease"

# --- 6. a stale executable is named and ensures nothing
#
# Through the adapter, because that is where the staleness is decided, for both entry paths
# at once (lib/rust_bin.sh's mj_rust_stale). A copy of the checkout's own adapter, a copy of
# a real executable, and a source file newer than it.
STALE="$T/stale"; mkdir -p "$STALE/bin" "$STALE/lib" "$STALE/apps/majordomus-cli/src" "$STALE/target/debug"
cp "$ROOT/bin/majordomus-env" "$STALE/bin/"; cp "$ROOT/lib/rust_bin.sh" "$STALE/lib/"
cp "$RB" "$STALE/target/debug/majordomus"
printf 'fn main() {}\n' > "$STALE/apps/majordomus-cli/src/main.rs"
printf '[package]\nname = "x"\n' > "$STALE/apps/majordomus-cli/Cargo.toml"
touch "$STALE/apps/majordomus-cli/src/main.rs"    # newer than the executable copied above
( cd "$STALE" && MAJORDOMUS_BIN="" CARGO_TARGET_DIR="$STALE/target" \
    bin/majordomus-env enter --repo "$T" --shell direnv --no-banner --no-bridge \
    >"$T/stale.out" 2>"$T/stale.err" )
code=$?
[ "$code" = 0 ] || { echo "    a stale executable failed the entry ($code); entering a directory must not fail"; cat "$T/stale.err"; exit 1; }
grep -q 'older than the sources' "$T/stale.err" || { echo "    a stale executable was not named:"; cat "$T/stale.err"; exit 1; }
no_lease_appears "a runtime was ensured from an executable older than its sources" || exit 1
grep -qE '^export MAJORDOMUS_ROOT=' "$T/stale.out" || { echo "    a stale executable cost the environment too"; exit 1; }

# --- 7. the descriptors a caller holds are not inherited by the server it caused
#
# The row that cost 481 seconds. `Command` replaces standard input, output and error and
# nothing else, so every other descriptor the caller had open is inherited by the child and
# held for its whole idle life. direnv is where that stops being theoretical: it hands the
# file it evaluates an extra descriptor of its own, and it does not return until that pipe
# closes. On 2026-09-11 the first real `cd` into a checkout hung for eight minutes and only
# returned when the server was killed by hand; `lsof` showed the server holding the other
# end of direnv's pipe.
#
# Nothing here could see it before, because a case that captures output in a command
# substitution passes the command no descriptor above two — which is the very thing direnv
# does. So this row opens one deliberately, the way direnv does, and requires the pipe to be
# closed once the entry has returned and this shell has let go of its own end.
stop_server() { "$RB" serve stop --repo "$T" >/dev/null 2>&1 || true; rm -f "$lease"; }
stop_server
FIFO="$T/inherited.fifo"
mkfifo "$FIFO" || { echo "    could not make a fifo; skipping the descriptor row"; FIFO=""; }
if [ -n "$FIFO" ]; then
  rm -f "$T/eof"
  # a reader that touches a file when — and only when — every writer has let go
  ( cat "$FIFO" >/dev/null 2>&1; : > "$T/eof" ) &
  reader=$!
  # fd 9 open on the pipe, exactly as direnv opens its own, and inherited by what runs next
  exec 9> "$FIFO"
  enter >/dev/null 2>"$T/fd.err" || { echo "    entry failed with a descriptor open"; cat "$T/fd.err"; exit 1; }
  await_lease || { echo "    the descriptor row started no server, so it proves nothing"; exit 1; }
  exec 9>&-      # this shell's end; the only end left open now would be the server's
  # bounded, with a predicate and something to say when it does not happen
  i=0; while [ ! -f "$T/eof" ] && [ "$i" -lt 100 ]; do i=$((i+1)); sleep 0.1; done
  if [ ! -f "$T/eof" ]; then
    echo "    a server this entry started inherited a descriptor the caller was waiting on:"
    echo "    under direnv this is a shell that does not come back until the server's idle life ends"
    ps -ax -o pid=,command= 2>/dev/null | grep -F -- "serve --repo $T" | grep -v grep | sed 's/^/    | /'
    kill "$reader" 2>/dev/null || true
    stop_server
    exit 1
  fi
  kill "$reader" 2>/dev/null || true
  echo "    a descriptor the caller held: closed as soon as entry returned, not held for the server's life"
  stop_server
fi

echo "    entering brings the runtime up without waiting for it, repeated entries change nothing, both stay in budget, a stale executable is named rather than served from, and nothing the caller holds is inherited"
