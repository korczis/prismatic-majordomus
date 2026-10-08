# majordomus-covers: none
# majordomus-timeout: 300
# A mesh declaration that enables Bonjour is held by the server on every platform (ADR 0120).
#
# The Bonjour provider asks the operating system's DNS-SD service — mDNSResponder on macOS,
# avahi on Linux — and a machine that has neither is not broken: the provider reports
# `unavailable` with the reason, the mesh runs on, and `mesh doctor` warns without failing.
# This case cannot choose which machine it runs on, and the provider has no switch a test
# could flip (the mesh has no environment variables, docs/MESH.md), so it asserts what holds
# on both: a server whose declaration enables Bonjour names the provider in `mesh status`
# as running or unavailable and never as failed-and-fatal, `mesh doctor` carries one
# `bonjour` line that says which, with the reason when it is unavailable, and exits 0
# either way; a declaration without the block starts no such provider; and an interval the
# presence window cannot hold is refused at the index, before any server reads it.
#
# What is on the wire, where the platform has the service: one instance of a service type
# made for this run (`_mj1002-<pid>._tcp`, never `_majordomus._tcp`), on a loopback port,
# for the seconds the fixture server lives — so no running Majordomus hears the case, and
# the case hears none. Multicast is off, there is no hub and no seed, and the identity
# lives under $T. The split, the reassembly and the verification of the envelope are the
# crate's (src/mesh/bonjour.rs); this is the operator's path.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
STATE="$T/state"; mkdir -p "$STATE"
export XDG_STATE_HOME="$STATE"
SERVICE="_mj1002-$(($$ % 100000))._tcp"

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A && git commit -qm base

SRV=""
trap '[ -z "$SRV" ] || kill "$SRV" 2>/dev/null || true' EXIT
url_of() { jq -r '.url // empty' "$T/.ai/local/state/mcp/server.json" 2>/dev/null; }
serve() {
  rm -f "$T/.ai/local/state/mcp/server.json"
  ( cd "$T" && exec "$RB" serve --port 0 --idle 120 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
  SRV=$!
  local i=0
  while [ -z "$(url_of)" ]; do
    kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before it listened:"; cat "$T/serve.log"; return 1; }
    i=$((i + 1)); [ "$i" -lt 300 ] || { echo "    the server has not listened in 60 s:"; cat "$T/serve.log"; return 1; }
    sleep 0.2
  done
}
unserve() { [ -z "$SRV" ] || { kill "$SRV" 2>/dev/null || true; wait "$SRV" 2>/dev/null || true; SRV=""; }; }
declare_mesh() { # the bonjour block, or nothing
  mkdir -p .ai/repo/mesh
  { printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\n'
    printf '%b' "$1"
    printf 'trust:\n  policy: deny_unknown\n'
  } > .ai/repo/mesh/majordomus.yaml
  git add .ai/repo/mesh/majordomus.yaml && git commit -qm "mesh: $2"
}

# ---------------------------------------------------------------- 1. declared: the server names the provider
declare_mesh "bonjour:\n  enabled: true\n  service: $SERVICE\n  interval_seconds: 5\n" "bonjour on"
serve || exit 1
# The provider says `starting` until its thread has asked the platform to browse; wait that out.
i=0
while :; do
  "$RB" mesh status --repo "$T" --format json > "$T/status.json" 2> "$T/status.err" \
    || { echo "    mesh status did not answer:"; cat "$T/status.err"; exit 1; }
  jq -e '[.providers[] | select(.id == "bonjour") | .detail // "" | startswith("starting")] == [true]' "$T/status.json" >/dev/null || break
  i=$((i + 1)); [ "$i" -lt 100 ] || { echo "    the provider is still starting after 20 s:"; jq '.providers' "$T/status.json"; exit 1; }
  sleep 0.2
done
state="$(jq -r '[.providers[] | select(.id == "bonjour")] | if length == 1 then .[0].state else "listed \(length) time(s)" end' "$T/status.json")"
case "$state" in
  running|unavailable) ;;
  *) echo "    a declared bonjour provider is '$state', neither running nor unavailable:"; jq '.providers' "$T/status.json"; cat "$T/serve.log"; exit 1 ;;
esac
jq -e '.active == true' "$T/status.json" >/dev/null \
  || { echo "    the mesh is not active beside a bonjour provider that is $state:"; cat "$T/status.json"; exit 1; }
jq -e '[.providers[] | select(.id == "bonjour") | .detail // "" | length > 0] == [true]' "$T/status.json" >/dev/null \
  || { echo "    the provider is $state and says nothing about it:"; jq '.providers' "$T/status.json"; exit 1; }
expect_exit 0 "$RB" mesh status --repo "$T"
expect_grep "^provider   bonjour $state sent "

# the doctor: one line, the server's, and the verdict holds on both platforms
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    declaration  majordomus: enabled=true, multicast=false, broadcast=false, bonjour=true, '
expect_grep '^verdict    every check holds'
if [ "$state" = running ]; then
  expect_grep "^ok    bonjour      declared and started: (majordomus-[0-9a-f]{8}-[0-9a-f]{8}\\.|browsing )$SERVICE\\.local\\. "
else
  expect_grep '^WARN  bonjour      declared and unavailable on this platform — .'
  expect_grep '^      impact      .*multicast, rendezvous and seeds are unaffected'
  expect_grep '^      remedy      .*avahi'
fi
"$RB" mesh doctor --repo "$T" --format json > "$T/doctor.json" 2> "$T/doctor.err" \
  || { echo "    mesh doctor --format json failed beside a bonjour provider that is $state:"; cat "$T/doctor.err"; exit 1; }
jq -e --arg state "$state" '
  .ok == true
  and ([.checks[] | select(.check == "bonjour")] | length == 1)
  and (.checks[] | select(.check == "bonjour") | .ok and ((.warning // false) == ($state == "unavailable")))' \
  "$T/doctor.json" >/dev/null || { echo "    the JSON report does not carry the bonjour verdict for '$state':"; jq '.checks' "$T/doctor.json"; exit 1; }
# the instance name discloses nothing but hex: no host name, no user, no path
if [ "$state" = running ]; then
  detail="$(jq -r '.providers[] | select(.id == "bonjour") | .detail' "$T/status.json")"
  case "$detail" in
    *"$(hostname -s 2>/dev/null || echo no-host-name)"*|*"$T"*|*"${USER:-no-user-name}"*)
      echo "    the provider's subject carries a host name, a user name or a path: $detail"; exit 1 ;;
  esac
fi
unserve

# with no server, the line is this machine's own answer, and still not a failure
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^(ok  |WARN)  bonjour      declared(;| and unavailable on this platform —) '

# ---------------------------------------------------------------- 2. not declared: no such provider
declare_mesh "" "bonjour not declared"
serve || exit 1
"$RB" mesh status --repo "$T" --format json > "$T/status.json" 2> "$T/status.err" \
  || { echo "    mesh status did not answer:"; cat "$T/status.err"; exit 1; }
jq -e '.active == true and ([.providers[] | select(.id == "bonjour")] | length == 0)' "$T/status.json" >/dev/null \
  || { echo "    a declaration without a bonjour block started the provider:"; jq '.providers' "$T/status.json"; exit 1; }
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_grep '^ok    bonjour      not declared'
unserve

# ---------------------------------------------------------------- 3. an interval the presence window cannot hold
declare_mesh "bonjour:\n  enabled: true\n  interval_seconds: 61\n" "bonjour too slow"
expect_exit 0 "$RB" mesh doctor --repo "$T"
expect_no_grep 'bonjour=true'
echo "    a declared Bonjour provider is $state here, named by status and doctor, and fails nothing"
