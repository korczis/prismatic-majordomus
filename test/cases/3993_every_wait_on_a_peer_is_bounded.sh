# majordomus-covers: none
# majordomus-timeout: 300
# Every wait of the executable on another session or runtime is bounded, and says what it
# waited for when the bound is reached (I3993). The registry is mesh::waits::PEER_WAITS; its
# unit tests hold each entry to the source line it names and to docs/MESH.md. Here, end to end:
#
#   1. under the default declaration `mesh doctor`'s peer_waits check holds, counting every wait
#   2. a declaration that sets cooperation.session_ttl_seconds to 0 turns the progress rule
#      off: the check fails naming mesh.session.progress, and the doctor exits 10
#   3. a follow that waits for a signal another session produces stops at its bound, exits 10
#      and names the wait, the signal it waited for and where it goes now
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MAJORDOMUS_MONITOR_FOR

R="$T/repo"
mkdir -p "$R" "$T/state"
declare_mesh() { # extra cooperation line, or nothing
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n'
    printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: case-3993\n'
    [ -z "${1:-}" ] || printf '  %s\n' "$1"
  } > "$R/.ai/repo/mesh/majordomus.yaml"
  git -C "$R" add -A && git -C "$R" commit -qm "the mesh declaration${1:+: $1}"
}
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
)
declare_mesh
cd "$R" || exit

# the doctor exits 10 on any failed check, and on this machine others may fail: only the answer is read
waits() {
  { XDG_STATE_HOME="$T/state" "$RB" mesh doctor --format json 2>"$T/doctor.err" || true; } \
    | jq -c '.checks[] | select(.check == "peer_waits")'
}

# ---------------------------------------------------------------- 1. every wait is bounded under the defaults
CHECK="$(waits)"
[ -n "$CHECK" ] || { echo "    the doctor has no peer_waits check"; cat "$T/doctor.err"; exit 1; }
printf '%s' "$CHECK" | jq -e '.ok == true' >/dev/null || { echo "    peer_waits fails under the defaults: $CHECK"; exit 1; }
printf '%s' "$CHECK" | jq -e '.detail | test("^[0-9]+ peer wait\\(s\\) of the executable, each bounded: [0-9]+ answered at once, the longest 14400 s")' >/dev/null \
  || { echo "    the verdict does not count the waits and their longest bound: $CHECK"; exit 1; }

# ---------------------------------------------------------------- 2. a declared ttl of 0 leaves a wait unbounded, and the doctor refuses it
declare_mesh 'session_ttl_seconds: 0'
CHECK="$(waits)"
printf '%s' "$CHECK" | jq -e '.ok == false' >/dev/null || { echo "    peer_waits holds with the progress rule off: $CHECK"; exit 1; }
printf '%s' "$CHECK" | jq -e '.detail | startswith("mesh.session.progress (") and endswith("cooperation.session_ttl_seconds is 0, which turns it off")' >/dev/null \
  || { echo "    the failure does not name the unbounded wait and why: $CHECK"; exit 1; }
printf '%s' "$CHECK" | jq -e '.remediation | contains("cooperation.session_ttl_seconds")' >/dev/null \
  || { echo "    the failure gives no remedy: $CHECK"; exit 1; }
rc=0
XDG_STATE_HOME="$T/state" "$RB" mesh doctor >/dev/null 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    the doctor exited $rc with an unbounded peer wait, not 10"; exit 1; }

# ---------------------------------------------------------------- 3. a follow that reaches its bound names what it waited for
expect_exit 10 "$RB" monitor --follow --until merged --match '#885' --sources mesh --timeout 1 --interval 1
"$RB" monitor --follow --until merged --match '#885' --sources mesh --timeout 1 --interval 1 >/dev/null 2>"$T/monitor.err" || true
expect_grep 'monitor: --timeout reached before --count or --until was met' "$T/monitor.err"
expect_grep 'monitor: peer wait monitor.until reached its bound after [0-9]* s waiting for a signal from the stream' "$T/monitor.err"
expect_grep "held by whoever was to produce a .merged. signal matching '#885' on the mesh stream; report: the follow exits 10" "$T/monitor.err"
