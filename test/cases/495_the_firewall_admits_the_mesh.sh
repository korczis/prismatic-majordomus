# majordomus-timeout: 300
# claims: mesh-firewall-is-derived
# The host firewall is a derived requirement of the mesh with a verdict, not a thing an
# operator remembers.
#
# On 2026-10-08 lundra, a declared hub, ran ufw with default deny and no rule for the hub
# port or the multicast group: every registration and every advertisement from the LAN was
# dropped at the kernel while `mesh doctor` said every check held, because every check it
# ran was a fact of the process and none was a fact of the host. What this case holds:
#
#   1. over this repository's committed, enabled declaration, `mesh.firewall` derives the
#      multicast group's port as a rule, names a backend it knows (or `none`), reads a
#      five-minute window of the kernel log when it can, and renders one command per rule
#      and source network, each carrying the `majordomus mesh` comment;
#   2. `mesh doctor` carries a `firewall` check, and a failed one names its impact and
#      remedy;
#   3. a repository with no declaration needs nothing: an empty plan, `ok`, exit 0;
#   4. a declaration that makes this machine a hub derives a TCP rule for the hub port from
#      the private network the declaration names, and only that network;
#   5. `mesh firewall apply` refuses without root — exit 10, nothing run — so the suite can
#      never change a developer's firewall;
#   6. the MCP tool `majordomus_mesh_firewall` answers the same report.
#
# Offline and bounded: the fixture declarations disable multicast and name a loopback or
# this machine's own address as the hub, nothing is sent anywhere, and nothing is applied.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# ---------------------------------------------------------------- 1. the committed declaration
( cd "$ROOT" && XDG_STATE_HOME="$T/state" "$RB" run mesh.firewall --format json ) \
  > "$T/fw.run.json" 2> "$T/fw.err" || { cat "$T/fw.err"; exit 1; }
jq '.output' "$T/fw.run.json" > "$T/fw.json"
jq -e '.plan.rules | any(.role == "multicast" and .protocol == "udp" and .port == 7741 and .destination == "239.255.77.77" and .sources == [])' "$T/fw.json" >/dev/null \
  || { echo "    the committed declaration's multicast group is not a rule of the plan:"; jq '.plan' "$T/fw.json"; exit 1; }
jq -e '.plan.networks | length >= 1' "$T/fw.json" >/dev/null \
  || { echo "    the plan derived no private network from the declared hubs:"; jq '.plan' "$T/fw.json"; exit 1; }
backend="$(jq -r '.backend' "$T/fw.json")"
case "$backend" in ufw|nftables|application_firewall|none) ;; *) echo "    the backend is not a word this case knows: $backend"; exit 1 ;; esac
jq -e '.platform | length > 0' "$T/fw.json" >/dev/null || { echo "    no platform"; exit 1; }
jq -e '.window_seconds == 300' "$T/fw.json" >/dev/null \
  || { echo "    the kernel-log window is not five minutes: $(jq '.window_seconds' "$T/fw.json")"; exit 1; }
state="$(jq -r '.observation.state' "$T/fw.json")"
case "$state" in no_backend|inactive|present|missing|unobservable) ;; *) echo "    the observation state is not one this case knows: $state"; exit 1 ;; esac
# the verdict is a sentence, and `ok` agrees with it: a missing rule or a logged drop fails
jq -e '.verdict | length > 20' "$T/fw.json" >/dev/null || { echo "    no verdict"; exit 1; }
dropped="$(jq '[.blocked_recently // {} | .[]] | add // 0' "$T/fw.json")"
if [ "$state" = missing ] || [ "$dropped" -gt 0 ]; then
  jq -e '.ok == false' "$T/fw.json" >/dev/null || { echo "    a missing rule or a logged drop must fail the report"; exit 1; }
else
  jq -e '.ok == true' "$T/fw.json" >/dev/null || { echo "    nothing missing and nothing dropped, yet the report fails: $(jq -r .verdict "$T/fw.json")"; exit 1; }
fi
case "$backend" in
  ufw|nftables)
    n="$(jq '.commands | length' "$T/fw.json")"
    [ "$n" -ge 1 ] || { echo "    $backend renders no command for a plan with rules"; exit 1; }
    jq -e '.commands | all(.argv[-1] | startswith("majordomus mesh"))' "$T/fw.json" >/dev/null \
      || { echo "    a rendered command does not carry the majordomus mesh comment:"; jq '.commands' "$T/fw.json"; exit 1; }
    jq -e '.commands | all(.line | test("^(ufw allow in|nft add rule inet filter input) "))' "$T/fw.json" >/dev/null \
      || { echo "    a rendered command is not an inbound allow:"; jq '.commands[].line' "$T/fw.json"; exit 1; }
    ;;
  none)
    jq -e '.commands == [] and .observation.state == "no_backend"' "$T/fw.json" >/dev/null \
      || { echo "    no backend, yet commands or an observation:"; jq '{commands, observation}' "$T/fw.json"; exit 1; }
    ;;
esac
[ -e "$T/state/majordomus/node.json" ] && { echo "    the firewall report created an identity; it must only read"; exit 1; }

# ---------------------------------------------------------------- 2. the doctor carries it
( cd "$ROOT" && XDG_STATE_HOME="$T/state" "$RB" run mesh.doctor --format json ) \
  > "$T/doctor.run.json" 2> "$T/doctor.err" || { cat "$T/doctor.err"; exit 1; }
jq -e '.output.checks | map(.check) | index("firewall") != null' "$T/doctor.run.json" >/dev/null \
  || { echo "    mesh doctor runs no firewall check: $(jq -c '.output.checks | map(.check)' "$T/doctor.run.json")"; exit 1; }
jq -e '.output.checks | map(.check) | index("firewall") > index("link") and index("firewall") < index("runtime")' "$T/doctor.run.json" >/dev/null \
  || { echo "    the firewall check is not between the process's checks and the server's verdict"; exit 1; }
jq -e '.output.checks[] | select(.check == "firewall") | .ok or (.impact != null and .remediation != null and (.remediation | test("mesh firewall")))' "$T/doctor.run.json" >/dev/null \
  || { echo "    a failed firewall check names no impact or no remedy:"; jq '.output.checks[] | select(.check == "firewall")' "$T/doctor.run.json"; exit 1; }

# ---------------------------------------------------------------- 3. no declaration, nothing needed
R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit 1
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  git add -A && git commit -qm base
) || { echo "    the fixture repository could not be made"; exit 1; }
rc=0; ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall --format json ) > "$T/none.json" 2> "$T/none.err" || rc=$?
[ "$rc" = 0 ] || { echo "    mesh firewall exited $rc over a repository with no declaration"; cat "$T/none.err"; exit 1; }
jq -e '.ok == true and .plan.rules == [] and .commands == []' "$T/none.json" >/dev/null \
  || { echo "    no declaration, yet the plan needs something:"; jq '{ok, plan, commands}' "$T/none.json"; exit 1; }
# the text rendering is a report, not JSON
( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall ) > "$T/none.txt" 2>/dev/null || { echo "    the text rendering failed"; exit 1; }
expect_grep '^backend ' "$T/none.txt"
expect_grep '^verdict ' "$T/none.txt"

# ---------------------------------------------------------------- 4. this machine as a hub
addr="$(jq -r '.plan.local_addresses[0] // empty' "$T/fw.json")"
if [ -z "$addr" ]; then
  echo "    (this machine has no non-loopback IPv4 address; the hub step is held by the crate's unit tests instead)"
else
  mkdir -p "$R/.ai/repo/mesh"
  cat > "$R/.ai/repo/mesh/majordomus.yaml" <<EOF
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: true
multicast:
  enabled: false
broadcast:
  mode: disabled
rendezvous:
  endpoints:
    - http://$addr:8791
  interval_seconds: 30
trust:
  policy: deny_unknown
EOF
  git -C "$R" add .ai/repo/mesh/majordomus.yaml && git -C "$R" commit -qm "a hub declaration"
  ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall --format json ) > "$T/hub.json" 2> "$T/hub.err" || true
  jq -e --arg a "$addr" '.plan.hub_ports == [8791] and (.plan.rules | length == 1) and .plan.rules[0].role == "hub" and .plan.rules[0].protocol == "tcp" and .plan.rules[0].port == 8791' "$T/hub.json" >/dev/null \
    || { echo "    a declaration naming this machine ($addr) as a hub does not derive the hub rule:"; cat "$T/hub.err"; jq '.plan' "$T/hub.json"; exit 1; }
  # the sources are the private range the declared address is in, and nothing wider
  net="$(jq -r '.plan.rules[0].sources | join(",")' "$T/hub.json")"
  case "$addr" in
    10.*)      want="10.0.0.0/8" ;;
    192.168.*) want="192.168.$(printf '%s' "$addr" | cut -d. -f3).0/24" ;;
    172.*)     want="172.16.0.0/12" ;;
    100.*)     want="100.64.0.0/10" ;;
    *)         want="" ;;
  esac
  [ "$net" = "$want" ] || { echo "    the hub rule admits '$net', expected '$want' for $addr"; exit 1; }
  # a server beyond loopback is dialed on its port too, once, and not twice when it is the hub's
  ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall --port 8741 --format json ) > "$T/hub2.json" 2>/dev/null || true
  jq -e '[.plan.rules[] | select(.role == "link")] | length == 1 and .[0].port == 8741' "$T/hub2.json" >/dev/null \
    || { echo "    --port 8741 derives no link rule:"; jq '.plan.rules' "$T/hub2.json"; exit 1; }
  ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall --port 8791 --format json ) > "$T/hub3.json" 2>/dev/null || true
  jq -e '.plan.rules | length == 1' "$T/hub3.json" >/dev/null \
    || { echo "    the hub port given as the server port is admitted twice:"; jq '.plan.rules' "$T/hub3.json"; exit 1; }
fi

# ---------------------------------------------------------------- 5. apply refuses without root
if [ "$(id -u)" = 0 ]; then
  echo "    (running as root; the refusal without root is held by the crate's unit tests instead)"
else
  rc=0; ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall apply --format json ) > "$T/apply.json" 2> "$T/apply.err" || rc=$?
  [ "$rc" = 10 ] || { echo "    mesh firewall apply exited $rc without root, expected 10"; cat "$T/apply.err"; cat "$T/apply.json"; exit 1; }
  jq -e '.ok == false and .applied == [] and (.refused | length > 0)' "$T/apply.json" >/dev/null \
    || { echo "    apply did not refuse cleanly:"; cat "$T/apply.json"; exit 1; }
  ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mesh firewall apply ) > "$T/apply.txt" 2>/dev/null || true
  expect_grep '^refused ' "$T/apply.txt"
fi

# ---------------------------------------------------------------- 6. the MCP tool
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case495","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_mesh_firewall","arguments":{}}'
} > "$T/mcp.in"
rc=0; ( cd "$R" && XDG_STATE_HOME="$T/state" "$RB" mcp --standalone < "$T/mcp.in" ) > "$T/mcp.out" 2> "$T/mcp.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the MCP server exited $rc"; cat "$T/mcp.err"; exit 1; }
sed -n 2p "$T/mcp.out" | jq -e '.id == 2 and (.result.isError // false | not)' >/dev/null \
  || { echo "    majordomus_mesh_firewall did not answer:"; cat "$T/mcp.out"; exit 1; }
sed -n 2p "$T/mcp.out" | jq -r '.result.content[0].text' | jq -e '.backend and .plan and .verdict' >/dev/null \
  || { echo "    the MCP answer is not the firewall report:"; sed -n 2p "$T/mcp.out"; exit 1; }
