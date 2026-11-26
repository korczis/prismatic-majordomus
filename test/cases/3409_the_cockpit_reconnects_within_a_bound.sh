# majordomus-covers: none
# The Cockpit's one WebSocket client reconnects within a bound (I1017): share/cockpit/socket.js
# retries with a growing, capped, jittered wait and stops after MAX_ATTEMPTS failures in a
# row, reading offline, so a tab left open on a dead server is no request storm. A connection
# that opens and is dropped before the server speaks the protocol does not start the backoff
# over; one that receives `stream.ready` does. The suite runs against a fake WebSocket and a
# fake clock, with no server: scripts/lib/cockpit-socket.test.mjs.
. "$ROOT/test/lib.sh"
mj_runs node || {
  [ -z "${CI:-}" ] || { echo "    node is required on CI: the reconnection suite cannot run ($(mj_runs_why node))"; exit 1; }
  skip "$(mj_runs_why node)"; }
out="$(cd "$ROOT" && node --test scripts/lib/cockpit-socket.test.mjs 2>&1)" \
  || { printf '%s\n' "$out" | grep -E '^not ok|# (pass|fail)|Error|expected|actual' | head -40; exit 1; }
printf '%s\n' "$out" | grep -qE '^# fail 0$' || { printf '%s\n' "$out" | tail -20; exit 1; }
pass="$(printf '%s\n' "$out" | sed -n 's/^# pass \([0-9]*\)$/\1/p')"
# a suite that ran nothing passes nothing: the count is a measured floor
[ "${pass:-0}" -ge 4 ] || { echo "    the reconnection suite ran ${pass:-0} test(s), fewer than the 4 it holds"; exit 1; }
echo "    ok: $pass reconnection test(s) pass"
