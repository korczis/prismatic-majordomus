# majordomus-timeout: 180
# majordomus-covers: capability:mesh.doctor
# The mesh client reaches an https endpoint over TLS and refuses cleartext to a public address
# (I3823, ADR 0220 decision 1).
#
# Until this, the client stripped `http://` and wrote to a bare socket: an `https://` hub could
# not be reached at all, and a public `http://` one would have carried the journal in
# cleartext. Now an `https://host[:port]` hub or seed — by host name or address — is reached
# over TLS with its certificate verified, and an `http://` one only when every address it
# resolves to is loopback, private, link-local or an overlay's. What this case holds, through
# the declaration and the self-check that judges it without dialing anything:
#
#   1. a committed declaration naming https hubs and seeds by host name and by address, and
#      http hubs and seeds at private, tailnet and loopback addresses, is accepted, and
#      `mesh doctor`'s `transport` check counts each over TLS or in cleartext and holds;
#   2. a public http hub or seed is a shape the declaration accepts and an address the client
#      refuses: the `transport` check fails naming the address and the correction (use
#      https://), and names none of the private endpoints beside it;
#   3. any other scheme, and a seed without one, is refused by the declaration itself.
#
# The TLS round trip itself — a verified certificate answered, an unknown authority and a
# certificate for another name refused naming the endpoint, a public http endpoint refused
# before a socket opens — is proved by the crate's unit tests against a local rustls server
# with a certificate authority minted in memory (src/mcp/bridge.rs). Offline: no hub declared
# here is dialed, an https endpoint is not even resolved, and the only names resolved are
# literal addresses and localhost.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A && git commit -qm base

# declare ENDPOINTS SEEDS — commit a declaration with these YAML list items (one per line),
# because a working tree's widening of the trunk's copy is narrowed out (trust-root)
declare_mesh() {
  mkdir -p .ai/repo/mesh
  {
    printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n'
    printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\nbroadcast:\n  mode: disabled\n'
    printf 'rendezvous:\n  endpoints:\n'
    printf '%s\n' "$1" | sed 's/^/    - /'
    printf 'cooperation:\n  seeds:\n'
    printf '%s\n' "$2" | sed 's/^/    - /'
    printf 'trust:\n  policy: deny_unknown\n'
  } > .ai/repo/mesh/majordomus.yaml
  git add .ai/repo/mesh/majordomus.yaml && git commit -qm "a mesh declaration"
}

# doctor NAME — the in-process self-check over this repository, its report in $T/NAME.json
doctor() {
  local rc=0
  XDG_STATE_HOME="$T/state" "$RB" run mesh.doctor --format json > "$T/$1.run.json" 2> "$T/$1.err" || rc=$?
  jq -e '.output.checks' "$T/$1.run.json" >/dev/null 2>&1 \
    || { echo "    mesh.doctor answered no report (exit $rc):"; cat "$T/$1.err" "$T/$1.run.json"; exit 1; }
  jq '.output' "$T/$1.run.json" > "$T/$1.json"
}
check_of() { jq -c --arg c "$2" '.checks[] | select(.check == $c)' "$T/$1.json"; }

# ---------------------------------------------------------------- 1. https and private http are held
declare_mesh "https://hub.example
https://203.0.113.5:8443
http://192.168.100.30:8791
http://100.92.246.32:8791
http://127.0.0.1:8791" "https://seed.example:8742
http://10.0.0.12:8741"
doctor held
decl="$(check_of held declaration)"
printf '%s' "$decl" | jq -e '.ok and (.detail | contains("rendezvous endpoints=5")) and (.detail | contains("2 seed(s)"))' >/dev/null \
  || { echo "    the declaration with https hubs and seeds by host name is not accepted: $decl"; exit 1; }
transport="$(check_of held transport)"
[ -n "$transport" ] || { echo "    mesh doctor runs no transport check: $(jq -c '[.checks[].check]' "$T/held.json")"; exit 1; }
printf '%s' "$transport" | jq -e '.ok and .detail == "3 endpoint(s) over TLS, 4 in cleartext to loopback, private or overlay addresses"' >/dev/null \
  || { echo "    the transport check does not hold over https and private http endpoints: $transport"; exit 1; }
jq -e '[.checks[].check] | index("transport") == index("declaration") + 1' "$T/held.json" >/dev/null \
  || { echo "    the transport check does not follow the declaration it judges: $(jq -c '[.checks[].check]' "$T/held.json")"; exit 1; }

# ---------------------------------------------------------------- 2. public cleartext is refused
declare_mesh "http://192.168.100.30:8791
http://203.0.113.5:8791
https://hub.example" "http://198.51.100.7:8742"
doctor public
decl="$(check_of public declaration)"
printf '%s' "$decl" | jq -e '.ok' >/dev/null \
  || { echo "    a public http hub is an address the client refuses, not a malformed declaration: $decl"; exit 1; }
transport="$(check_of public transport)"
printf '%s' "$transport" | jq -e '.ok | not' >/dev/null \
  || { echo "    a public http hub passes the transport check: $transport"; exit 1; }
for named in \
  'http://203.0.113.5:8791 was not dialed: 203.0.113.5 is a public address' \
  'use https://203.0.113.5:8791' \
  'http://198.51.100.7:8742 was not dialed: 198.51.100.7 is a public address' \
  'use https://198.51.100.7:8742'; do
  printf '%s' "$transport" | jq -e --arg n "$named" '.detail | contains($n)' >/dev/null \
    || { echo "    the refusal does not say '$named': $transport"; exit 1; }
done
printf '%s' "$transport" | jq -e '(.detail | contains("192.168.100.30") | not) and (.detail | contains("hub.example") | not)' >/dev/null \
  || { echo "    the refusal names an endpoint the client would dial: $transport"; exit 1; }
printf '%s' "$transport" | jq -e '(.remediation | contains("https://")) and (.impact | length > 0)' >/dev/null \
  || { echo "    the refusal carries no impact or no correction: $transport"; exit 1; }
jq -e '.ok == false' "$T/public.json" >/dev/null \
  || { echo "    the self-check holds over a declaration whose hub the client refuses"; exit 1; }

# ---------------------------------------------------------------- 3. other schemes are refused outright
declare_mesh "ws://hub.example:80" "http://10.0.0.12:8741"
doctor scheme
decl="$(check_of scheme declaration)"
printf '%s' "$decl" | jq -e '(.ok | not) and (.detail | contains("rendezvous endpoint '"'"'ws://hub.example:80'"'"' is not https://host[:port] or http://host:port"))' >/dev/null \
  || { echo "    a ws:// hub is not refused by the declaration: $decl"; exit 1; }
declare_mesh "http://10.0.0.2:8791" "10.0.0.12:8741"
doctor bare
decl="$(check_of bare declaration)"
printf '%s' "$decl" | jq -e '(.ok | not) and (.detail | contains("cooperation seed '"'"'10.0.0.12:8741'"'"' is not https://host[:port] or http://host:port"))' >/dev/null \
  || { echo "    a seed without a scheme is not refused by the declaration: $decl"; exit 1; }
