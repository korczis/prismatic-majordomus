# majordomus-timeout: 300
# The mesh is on in this repository, with least privilege, and it works as declared.
#
# The tool's default is off: the skeleton a new repository starts from ships no declaration
# (claim mesh-off-by-default; step 1 holds the skeleton to that). This repository
# decided otherwise and committed .ai/repo/mesh/majordomus.yaml enabled. What this case holds:
#
#   1. the committed declaration, as the executable reads it: enabled, multicast on the local
#      segment only (TTL 1), no broadcast, rendezvous hubs only at private or tailnet
#      addresses, no seed, cooperation at its defaults, trust `deny_unknown` with an
#      allowlist of public keys and nothing wider;
#   2. `mesh doctor` holds over this repository on a machine with no identity yet;
#   3. two runtimes of one repository on one machine — two worktrees, one node key, which is
#      this repository's everyday shape — run from that declaration, observe each other and
#      link, although their key is not on the allowlist: a machine's own key is trusted as
#      itself, and nothing else is;
#   4. a runtime of the same repository under another key (another machine that nobody
#      listed) is observed and refused `untrusted` when it dials;
#   5. `mesh doctor` tells that machine its key is not listed, and how to list it.
#
# Offline and bounded: the fixture runtimes take the committed declaration and change only
# its transport — multicast off, and the declared hubs replaced by a loopback rendezvous
# endpoint (and, for the stranger, a seed) — so nothing is sent to the declared group, which
# a developer's own servers listen on, nothing is sent to the owner's hubs, and nothing
# depends on a network interface. The multicast path itself is proved by the crate's suite
# and test/mesh-lab/run. Every identity lives under $T.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
DECL="$ROOT/.ai/repo/mesh/majordomus.yaml"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

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

# ---------------------------------------------------------------- 1. the committed declaration
expect_file "$DECL"
# This repository's decision is not the tool's default: the skeleton ships no declaration.
skeleton_mesh="$(find "$ROOT/share/skeleton" -path '*/repo/mesh/*' -name '*.yaml' 2>/dev/null)"
[ -z "$skeleton_mesh" ] || { echo "    the skeleton ships a mesh declaration, so every new repository would inherit this one's: $skeleton_mesh"; exit 1; }
EMPTY="$T/no-identity"; mkdir -p "$EMPTY"
( cd "$ROOT" && XDG_STATE_HOME="$EMPTY" "$RB" mesh doctor --repo "$ROOT" --format json ) \
  > "$T/doctor.json" 2> "$T/doctor.err" || { cat "$T/doctor.err"; exit 1; }
decl="$(jq -r '.checks[] | select(.check == "declaration") | .detail' "$T/doctor.json")"
case "$decl" in
  "majordomus: enabled=true, multicast=true, broadcast=false, rendezvous endpoints="*", trust=deny_unknown ("*" allowed key(s)), cooperation=true (heartbeat 5s, expiry 30s, 0 seed(s))") ;;
  *) echo "    the committed declaration is not the reviewed one (enabled, local multicast, no broadcast,"
     echo "    no seed, deny_unknown, cooperation at its defaults); the executable reads:"
     echo "    $decl"; exit 1 ;;
esac
allowed="$(printf '%s' "$decl" | sed -n 's/.*trust=deny_unknown (\([0-9]*\) allowed.*/\1/p')"
[ "${allowed:-0}" -ge 1 ] || { echo "    the declaration trusts no key: deny_unknown with an empty allowlist lists no machine"; exit 1; }
# The rest as the executable's index reads the committed object, comments stripped: the TTL,
# which the self-check does not print, the hubs and the allowlist.
( cd "$ROOT" && "$RB" run objects.get --input '{"uri":"majordomus://mesh-declaration/majordomus"}' --format json ) \
  > "$T/object.json" 2> "$T/object.err" || { cat "$T/object.err"; exit 1; }
jq -e '.output.metadata.enabled == true' "$T/object.json" >/dev/null \
  || { echo "    the index does not read the committed declaration as enabled:"; jq '.output' "$T/object.json"; exit 1; }
ttl="$(jq -r '.output.metadata.multicast.ttl // "absent"' "$T/object.json")"
[ "$ttl" = 1 ] || { echo "    multicast.ttl is '$ttl', not 1: announcements would leave the local segment"; exit 1; }
# Nothing is encrypted, so a hub is on the owner's private network or tailnet or it is not
# declared: RFC 1918 or the 100.64.0.0/10 range an overlay hands out, never a public address.
public_hubs="$(jq -r '.output.metadata.rendezvous.endpoints // [] | .[]
  | select(test("^http://(10\\.|192\\.168\\.|172\\.(1[6-9]|2[0-9]|3[01])\\.|100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.)[0-9.]+:[0-9]+$") | not)' "$T/object.json")"
[ -z "$public_hubs" ] || { echo "    a rendezvous hub is not on a private network or an overlay: $public_hubs"; exit 1; }
# every allowlisted entry is a public key: 64 hex characters, never anything longer
bad_keys="$(jq -r '.output.metadata.trust.allow // [] | .[] | select(test("^[0-9a-f]{64}$") | not)' "$T/object.json")"
[ -z "$bad_keys" ] || { echo "    trust.allow holds something other than a public key: $bad_keys"; exit 1; }

# ---------------------------------------------------------------- 2. mesh doctor holds
jq -e '.ok == true' "$T/doctor.json" >/dev/null \
  || { echo "    mesh doctor does not hold over this repository:"; jq -r '.checks[] | select(.ok | not) | "    \(.check): \(.detail)"' "$T/doctor.json"; exit 1; }
[ -e "$EMPTY/majordomus/node.json" ] && { echo "    the self-check created an identity; it must only read"; exit 1; }

# ---------------------------------------------------------------- 3. two worktrees, one key
# The committed declaration with its transport made loopback-only: multicast off, and the
# declared hubs (the endpoint items and their comments) replaced by one loopback endpoint or
# none. Every other line is carried over as committed — trust, allowlist and cooperation
# included — and the number of lines changed is checked, so a reshaped declaration fails here
# rather than leaking a fixture's announcements onto the declared group or to a hub.
derive_declaration() { # out want-changes [rendezvous-url [seed-url]]
  awk -v rv="${3:-}" -v seed="${4:-}" '
    /^[a-z]/ { s = $1 }
    hubs && /^    / { next }
    { hubs = 0 }
    s == "multicast:" && /^  enabled:/ { print "  enabled: false"; n++; next }
    s == "rendezvous:" && /^  endpoints:/ {
      if (rv != "") { print "  endpoints:"; print "    - " rv } else print "  endpoints: []"
      hubs = 1; n++; next
    }
    s == "cooperation:" && seed != "" && !seeded { print; print "  seeds:"; print "    - " seed; seeded = 1; n++; next }
    { print }
    END {
      if (seed != "" && !seeded) { print "cooperation:"; print "  seeds:"; print "    - " seed; n++ }
      printf "%d\n", n > "/dev/stderr"
    }' "$DECL" > "$1" 2> "$T/changes"
  [ "$(cat "$T/changes")" = "$2" ] \
    || { echo "    the committed declaration no longer has the shape this case derives its fixture from ($(cat "$T/changes") of $2 transport line(s) changed)"; return 1; }
  local away
  away="$(grep -E '^ *- *http://' "$1" | grep -v -E '^ *- *http://127\.0\.0\.1:[0-9]+$' || true)"
  [ -z "$away" ] || { echo "    the fixture declaration still names an address beyond loopback: $away"; return 1; }
}

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
mkdir -p "$R/.ai/repo/mesh"
derive_declaration "$R/.ai/repo/mesh/majordomus.yaml" 2 || exit 1
git -C "$R" add .ai/repo/mesh/majordomus.yaml
git -C "$R" commit -qm "the committed mesh declaration, loopback-only"
B="$T/wt-b"; C="$T/wt-c"
git -C "$R" worktree add -q --detach "$B"
git -C "$R" worktree add -q --detach "$C"

MACHINE="$T/state-machine"; STRANGER="$T/state-stranger"
mkdir -p "$MACHINE" "$STRANGER"

serve() { # checkout state
  ( cd "$1" && XDG_STATE_HOME="$2" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$1.serve.log" 2>&1 ) &
  PIDS="$PIDS $!"
}
url_of() { jq -r '.url // empty' "$1/.ai/local/state/mcp/server.json" 2>/dev/null; }
has_url() { [ -n "$(url_of "$1")" ]; }
mj() { local cwd="$1" state="$2"; shift 2; ( cd "$cwd" && XDG_STATE_HOME="$state" "$RB" "$@" 2>/dev/null ); }
runtime_of() { mj "$1" "$2" mesh peers --format json | jq -r '.runtime // empty'; }

serve "$R" "$MACHINE"
poll 300 has_url "$R" || { cat "$R.serve.log"; exit 1; }
URL_A="$(url_of "$R")"
derive_declaration "$B/.ai/repo/mesh/majordomus.yaml" 2 "$URL_A" || exit 1
serve "$B" "$MACHINE"
poll 300 has_url "$B" || { cat "$B.serve.log"; exit 1; }

expect_exit 0 mj "$R" "$MACHINE" mesh status
expect_grep '^mesh +active'
expect_exit 0 mj "$B" "$MACHINE" mesh status
expect_grep '^mesh +active'

RT_A="$(runtime_of "$R" "$MACHINE")"; RT_B="$(runtime_of "$B" "$MACHINE")"
[ -n "$RT_A" ] && [ -n "$RT_B" ] || { echo "    a runtime has no cooperation identity: A='$RT_A' B='$RT_B'"; cat "$R.serve.log" "$B.serve.log"; exit 1; }
[ "${RT_A%%-*}" = "${RT_B%%-*}" ] || { echo "    two worktrees of one machine are two nodes: $RT_A $RT_B"; exit 1; }
[ "$RT_A" != "$RT_B" ] || { echo "    two worktrees are one runtime: $RT_A"; exit 1; }

# each observes the other: one registry record, present, the other's runtime
observes() { # checkout runtime-id
  mj "$1" "$MACHINE" mesh nodes --format json \
    | jq -e --arg rt "${2#*-}" '[.nodes[] | select(.runtime == $rt and .presence == "present")] | length == 1'
}
poll 300 observes "$R" "$RT_B" || { echo "    A never observed B"; cat "$R.serve.log"; exit 1; }
poll 300 observes "$B" "$RT_A" || { echo "    B never observed A"; cat "$B.serve.log"; exit 1; }
# Discovery is not trust: the fixture's key is on no allowlist, and the registry says so.
expect_exit 0 mj "$R" "$MACHINE" mesh nodes --format json
jq -e --arg rt "${RT_B#*-}" '.nodes[] | select(.runtime == $rt) | .trust.state == "observed"' <<<"$LAST_OUT" >/dev/null \
  || { echo "    an unlisted key was labelled trusted by discovery:"; echo "$LAST_OUT"; exit 1; }

# and they link, trusting each other only as this machine's own key
linked() { # checkout peer-runtime-id
  mj "$1" "$MACHINE" mesh peers --format json \
    | jq -e --arg rt "$2" '[.machines[].runtimes[] | select(.runtime == $rt and .link.state == "connected")] | length == 1'
}
poll 300 linked "$R" "$RT_B" || { echo "    A never linked B:"; mj "$R" "$MACHINE" mesh peers; cat "$R.serve.log"; exit 1; }
poll 300 linked "$B" "$RT_A" || { echo "    B never linked A:"; mj "$B" "$MACHINE" mesh peers; exit 1; }
expect_exit 0 mj "$R" "$MACHINE" mesh verify
expect_grep 'cooperation verified'
expect_exit 0 mj "$B" "$MACHINE" mesh verify
expect_grep 'cooperation verified'

# ---------------------------------------------------------------- 4. another machine, unlisted
derive_declaration "$C/.ai/repo/mesh/majordomus.yaml" 3 "$URL_A" "$URL_A" || exit 1
serve "$C" "$STRANGER"
poll 300 has_url "$C" || { cat "$C.serve.log"; exit 1; }
RT_C="$(runtime_of "$C" "$STRANGER")"
[ -n "$RT_C" ] && [ "${RT_C%%-*}" != "${RT_A%%-*}" ] || { echo "    the stranger is not another node: $RT_C"; exit 1; }
refused_untrusted() {
  mj "$R" "$MACHINE" mesh peers --format json \
    | jq -e '[.refused[]? | select(.refusal.code == "untrusted")] | length >= 1'
}
poll 300 observes "$R" "$RT_C" || { echo "    A never observed the stranger"; exit 1; }
poll 300 refused_untrusted || { echo "    A did not refuse the stranger's hello as untrusted:"; mj "$R" "$MACHINE" mesh peers; cat "$C.serve.log"; exit 1; }
linked "$R" "$RT_C" >/dev/null 2>&1 && { echo "    A linked a key nobody listed"; exit 1; }
expect_exit 0 mj "$R" "$MACHINE" mesh nodes --format json
jq -e --arg rt "${RT_C#*-}" '.nodes[] | select(.runtime == $rt) | .trust.state == "observed"' <<<"$LAST_OUT" >/dev/null \
  || { echo "    the stranger was labelled trusted:"; echo "$LAST_OUT"; exit 1; }

# ---------------------------------------------------------------- 5. an unlisted machine is told
# The fixture machine now has an identity, which is on no allowlist; the self-check over this
# repository names the key and the remedy instead of passing silently.
( cd "$ROOT" && XDG_STATE_HOME="$MACHINE" "$RB" mesh doctor --repo "$ROOT" --format json ) \
  > "$T/doctor-unlisted.json" 2>/dev/null || { echo "    mesh doctor failed to answer"; exit 1; }
jq -e '.ok == false and ([.checks[] | select(.check == "trust" and (.ok | not) and (.remediation | test("trust.allow")))] | length == 1)' \
  "$T/doctor-unlisted.json" >/dev/null \
  || { echo "    mesh doctor did not tell an unlisted machine so:"; cat "$T/doctor-unlisted.json"; exit 1; }
echo "    ok: the declaration is on with deny_unknown; one machine's worktrees link, an unlisted key is refused"
