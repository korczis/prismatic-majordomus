# majordomus-timeout: 240
# majordomus-covers: capability:mesh.status
# A hosted hub keeps one key from a secret, advertises its URL, and shows the mesh only to
# those it trusts (I3826, ADR 0220 decisions 2 to 5).
#
# A hosted dyno's disk is gone at every boot: a server that minted its node key there would be
# a new, untrusted node after each restart. So a hub started with `--node-seed-env VAR` takes
# its key from that secret alone, and `--advertise URL` makes it advertise the public URL
# instead of the platform's internal interface addresses. What this case holds, through real
# servers on loopback:
#
#   1. two servers started with the same seed are the same node, across a restart;
#   2. with the variable named but unset, the mesh stays off and says which variable, and no
#      key is written to the state directory;
#   3. the advertised endpoint is the URL given, not an interface address;
#   4. the seed itself appears in no log and no answer.
#
# What a remote caller may not read (the mesh's capabilities and pages refuse a caller beyond
# loopback) and what a stranger learns from registration (the hub alone) are proved by the
# crate's unit tests: http::router's a_remote_caller_reads_the_repository_and_never_the_mesh
# and mesh::manager's a_stranger_learns_this_hub_and_never_who_trusts_it — a case on
# loopback cannot be a remote caller.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

SEED="$(printf '5e' ; printf '%062d' 7)"   # 64 hex characters, a fixture key
R="$T/repo"; mkdir -p "$R"
(
  cd "$R" || exit 1
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  git add -A && git commit -qm base
) || { echo "    the fixture repository could not be made"; exit 1; }

# The declaration lists the seed's own public key, so the hub's mesh opens (deny_unknown).
declare_mesh() { # allowed-key
  mkdir -p "$R/.ai/repo/mesh"
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\nbroadcast:\n  mode: disabled\ntrust:\n  policy: deny_unknown\n  allow:\n    - %s\n' "$1" > "$R/.ai/repo/mesh/majordomus.yaml"
  git -C "$R" add .ai/repo/mesh/majordomus.yaml && git -C "$R" commit -qm "the mesh" >/dev/null
}

PIDS=""
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; }
trap cleanup EXIT
# serve NAME STATE [env assignments...] — a server of the fixture with the hub flags
serve() {
  local name="$1" state="$2"; shift 2
  rm -f "$R/.ai/local/state/mcp/server.json"
  ( cd "$R" && env "$@" XDG_STATE_HOME="$state" "$RB" serve --port 0 --idle 0 --discovery filesystem \
      --node-seed-env HUB_SEED --advertise https://hub.example </dev/null >"$T/$name.log" 2>&1 ) &
  PIDS="$PIDS $!"
  local i=0
  while [ $i -lt 120 ]; do
    URL="$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null || true)"
    [ -n "$URL" ] && curl -fsS --max-time 2 "$URL/api/v1/live" >/dev/null 2>&1 && return 0
    sleep 0.5; i=$((i + 1))
  done
  echo "    the $name server did not answer:"; sed 's/^/      /' "$T/$name.log"; exit 1
}
# a stopped server's status is the signal's, never the case's (the runner runs `bash -eu`)
stop_last() {
  local p="${PIDS##* }"
  kill "$p" 2>/dev/null || true
  wait "$p" 2>/dev/null || true
  PIDS="${PIDS% *}"
}
status() { curl -fsS --max-time 10 "$URL/api/v1/mesh" 2>/dev/null || true; }
# a key no seed here has; letters keep YAML from reading it as a number
PLACEHOLDER="$(printf 'ab%.0s' $(seq 32))"

# ---------------------------------------------------------------- 2. no secret, no mesh, no key
declare_mesh "$PLACEHOLDER"
serve unset "$T/state-unset"
st="$(status)"
printf '%s' "$st" | jq -e '(.active // false) == false' >/dev/null \
  || { echo "    the mesh opened without its seed: $st"; exit 1; }
printf '%s' "$st" | grep -q 'HUB_SEED is not set' \
  || { echo "    the closed mesh does not name the variable it needs: $st"; exit 1; }
[ ! -e "$T/state-unset/majordomus/node.json" ] \
  || { echo "    a hosted hub without its secret minted a key of its own"; exit 1; }
stop_last

# ---------------------------------------------------------------- 1. one seed, one node, every boot
# the seed's public key: an unlisted seeded hub stays closed and names the key it would need
serve probe "$T/state-probe" HUB_SEED="$SEED"
probe="$(status)"
SEED_PUB="$(printf '%s' "$probe" | jq -r '.reason // empty' | grep -oE '[0-9a-f]{64}' | head -n 1 || true)"
stop_last
[ -n "$SEED_PUB" ] || { echo "    the unlisted seeded hub does not name its key: $probe"; exit 1; }
declare_mesh "$SEED_PUB"
serve first "$T/state-a" HUB_SEED="$SEED"
first="$(status | jq -r '.identity.node_id // empty')"
st="$(status)"
stop_last
serve second "$T/state-b" HUB_SEED="$SEED"
second="$(status | jq -r '.identity.node_id // empty')"
st2="$(status)"
[ -n "$first" ] && [ "$first" = "$second" ] \
  || { echo "    one seed gave nodes '$first' and '$second'"; exit 1; }
printf '%s' "$st2" | jq -e --arg pk "$SEED_PUB" '.identity.public_key == $pk' >/dev/null \
  || { echo "    the hub's key is not the seed's: $st2"; exit 1; }
[ ! -e "$T/state-a/majordomus/node.json" ] && [ ! -e "$T/state-b/majordomus/node.json" ] \
  || { echo "    the seeded key was written to the state directory"; exit 1; }

# ---------------------------------------------------------------- 3. the advertised endpoint
printf '%s' "$st2" | jq -e '(.active // false) == true' >/dev/null \
  || { echo "    the seeded hub's mesh is not open though its key is listed: $st2"; exit 1; }
printf '%s' "$st2" | jq -e '.advertised == ["https://hub.example"]' >/dev/null \
  || { echo "    the hub does not advertise exactly the URL it was given: $(printf '%s' "$st2" | jq -c '.advertised')"; exit 1; }
stop_last

# ---------------------------------------------------------------- 4. the secret stays secret
for f in "$T"/*.log; do
  ! grep -qF "$SEED" "$f" || { echo "    the seed appears in ${f##*/}"; exit 1; }
done
for answer in "$st" "$st2"; do
  case "$answer" in *"$SEED"*) echo "    the seed appears in a status answer"; exit 1 ;; esac
done
echo "    one seed is one node across restarts, an unset secret closes the mesh, the URL is advertised, and the seed is never printed"
