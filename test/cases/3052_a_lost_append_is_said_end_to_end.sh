# majordomus-covers: none
# majordomus-timeout: 300
# A mesh event the disk did not take is never acknowledged as durable, and a journal larger
# than the reload bound is named by `mesh doctor` (I2706), through the command line and a
# running server rather than the journal's own unit tests.
#
#   1. `mesh claim` on a journal file that takes appends answers `durability: synced`, and
#      its text says nothing more than `written`;
#   2. the journal's place is taken by a directory — no write reaches it, for any user, root
#      included, which is the fault the crate's own test injects: the next `mesh claim`
#      still exits 0 (the event is held and replicated), answers `durability: not_durable`,
#      and its text says NOT DURABLE and names `mesh doctor`; `mesh doctor` then fails its
#      `journal` check with exit 10, saying the events were not acknowledged as durable;
#   3. a journal file of the reload bound plus 4096 bytes, there before the server starts:
#      the reload reads up to the bound and `mesh doctor` fails its `journal` check naming
#      the 4096 bytes left unread beyond the 67108864-byte bound.
#
# The bound (MAX_RELOAD_BYTES, 64 MiB) is a constant, not a setting, so the oversized file is
# made here: blank lines, which a reload skips, so the only finding is the unread tail. One
# repository, one node identity under $T, one server on loopback at a time, stopped by the
# pid this case started. Nothing is dialed: no seed, no hub, multicast off.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_HTTP_HOST

PID=""
trap '[ -n "$PID" ] && kill "$PID" 2>/dev/null; true' EXIT

R="$T/repo"; STATE="$T/state"
mkdir -p "$R" "$STATE"
(
  cd "$R" || exit 1
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p lib && for f in a b c d e; do echo "$f" > "lib/$f"; done
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3052\n' >> .ai/repo/mesh/majordomus.yaml
  # committed on the trunk: trust on first use is honoured only from the trunk's copy
  git add -A && git commit -qm "base, with the mesh declared"
) || { echo "    the fixture repository could not be made"; exit 1; }
JOURNAL="$R/.ai/local/state/mesh/journal.jsonl"

serve() {
  ( cd "$R" && XDG_STATE_HOME="$STATE" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
  PID=$!
  local i=0
  while [ "$i" -lt 150 ]; do
    jq -e --argjson pid "$PID" '.pid == $pid and (.url // "") != ""' "$R/.ai/local/state/mcp/server.json" >/dev/null 2>&1 && return 0
    i=$((i + 1)); sleep 0.2
  done
  echo "    the server did not publish an address"; cat "$T/serve.log"; return 1
}
stop() { kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; PID=""; }

# mj ARGS... — the command line from the fixture, stdout alone (the discovery log is stderr)
mj() { ( cd "$R" && XDG_STATE_HOME="$STATE" "$RB" "$@" 2>"$T/mj.err" ); }

# poll N CMD... — run CMD every 0.2s until it succeeds, at most N tries.
poll() {
  local n="$1" i=0; shift
  while [ "$i" -lt "$n" ]; do
    "$@" >/dev/null 2>&1 && return 0
    i=$((i + 1)); sleep 0.2
  done
  printf '    gave up waiting for: %s\n' "$*"
  return 1
}

# ---------------------------------------------------------------- 1. a write the disk took
serve || exit 1
cooperating() { mj mesh claim lib/a --session s0 --format json | jq -e '.key' >/dev/null; }
poll 150 cooperating || { echo "    the server never cooperated:"; cat "$T/mj.err" "$T/serve.log"; exit 1; }
[ -f "$JOURNAL" ] || { echo "    the running server keeps no journal at $JOURNAL"; exit 1; }

mj mesh claim lib/c --session s1 --format json > "$T/synced.json" \
  || { echo "    a claim on a writable journal failed:"; cat "$T/synced.json" "$T/mj.err"; exit 1; }
jq -e '.durability == "synced"' "$T/synced.json" >/dev/null \
  || { echo "    a claim the disk took is not answered synced:"; cat "$T/synced.json"; exit 1; }
expect_exit 0 mj mesh claim lib/d --session s2
expect_grep '^written '
expect_no_grep 'NOT DURABLE'
# the control: before the fault, the journal check holds
expect_exit 0 mj mesh doctor
expect_grep '^ok +journal'

# ---------------------------------------------------------------- 2. a write the disk refused
rm -f "$JOURNAL"
mkdir "$JOURNAL"
mj mesh claim lib/b --session s3 --format json > "$T/lost.json" \
  || { echo "    a claim whose append failed was refused outright; it is held and replicated:"; cat "$T/mj.err"; exit 1; }
jq -e '.durability == "not_durable"' "$T/lost.json" >/dev/null \
  || { echo "    a claim the disk refused is not answered not_durable:"; cat "$T/lost.json"; exit 1; }
jq -e '(.key // "") != "" and .lamport > 0' "$T/lost.json" >/dev/null \
  || { echo "    the not-durable claim carries no key or stamp:"; cat "$T/lost.json"; exit 1; }
expect_exit 0 mj mesh claim lib/e --session s4
expect_grep '^written '
expect_grep 'NOT DURABLE'
expect_grep 'mesh doctor'
# held and offered: the claim is in the state this runtime serves
mj mesh state --full --format json > "$T/state.json"
jq -e --arg k "$(jq -r '.key' "$T/lost.json")" '[.state.claims[] | select(.key == $k or .id == $k)] | length >= 1' "$T/state.json" >/dev/null \
  || { echo "    the not-durable claim is not held:"; jq '.state.claims' "$T/state.json"; exit 1; }

expect_exit 10 mj mesh doctor
expect_grep '^FAIL  journal'
expect_grep '[1-9][0-9]* event\(s\) did not reach the journal file and were not acknowledged as durable'
stop

# ---------------------------------------------------------------- 3. a journal beyond the bound
rm -rf "$JOURNAL"
BOUND=67108864
head -c "$((BOUND + 4096))" /dev/zero | tr '\0' '\n' > "$JOURNAL"
[ "$(wc -c < "$JOURNAL" | tr -d ' ')" = "$((BOUND + 4096))" ] || { echo "    the oversized journal was not written"; exit 1; }
serve || exit 1
poll 150 cooperating || { echo "    the server never cooperated over the oversized journal:"; cat "$T/mj.err" "$T/serve.log"; exit 1; }
expect_exit 10 mj mesh doctor
expect_grep '^FAIL  journal'
expect_grep "4096 byte\\(s\\) of it lie beyond the $BOUND-byte reload bound and were left on disk unread"
mj mesh doctor --format json > "$T/doctor.json" || true
jq -e '[.checks[] | select(.check == "journal" and (.ok | not))] | length == 1' "$T/doctor.json" >/dev/null \
  || { echo "    mesh doctor's json does not fail the journal check:"; cat "$T/doctor.json"; exit 1; }
stop
exit 0
