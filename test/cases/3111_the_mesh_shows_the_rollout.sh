# majordomus-timeout: 300
# The mesh shows each node's version and how far a release has rolled out (I2723, ADR 0161;
# evidence: mesh_reports_versions_test).
#
# What this case holds:
#   1. Every cooperating runtime publishes its `version` and `follow` setting (`stable`,
#      `off`, or `pinned <version>`) in its runtime card and stream marks;
#   2. `majordomus mesh peers` prints each runtime's `version` and `follow` setting (in text
#      and JSON) and reports per-release rollout progress;
#   3. `majordomus mesh doctor` fails its `rollout` check and names a node that is more than
#      one release behind the stable pointer;
#   4. When every enrolled following node reports the current stable release, the release is
#      marked `rolled_out: true` and `majordomus mesh doctor` passes.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

poll() {
  local n="$1" i=0; shift
  while [ "$i" -lt "$n" ]; do
    "$@" >/dev/null 2>&1 && return 0
    i=$((i + 1)); sleep 0.2
  done
  printf '    gave up waiting for: %s\n' "$*"
  return 1
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
  mkdir -p .ai/repo/releases
  for ver in 0.26.0 0.27.0 0.28.0; do
    cat > ".ai/repo/releases/v${ver}.yaml" <<EOF
schema: release/v1
version: ${ver}
tag: v${ver}
channel: stable
commit: 0123456789abcdef0123456789abcdef01234567
published_at: "2026-10-17T00:00:00Z"
artifacts: []
EOF
  done
  git add -A && git commit -qm "base with three stable releases"
) || { echo "    fixture repository setup failed"; exit 1; }

STATE_A="$T/state-a"
STATE_B="$T/state-b"
mkdir -p "$STATE_A" "$STATE_B"

write_decl() { # path rv_url key1 [key2]
  local rv_block="  endpoints: []"
  if [ -n "${2:-}" ]; then
    rv_block="$(printf '  endpoints:\n    - %s' "$2")"
  fi
  local allow_block='  allow: ["0000000000000000000000000000000000000000000000000000000000000000"]'
  if [ -n "${3:-}" ] && [ -n "${4:-}" ]; then
    allow_block="$(printf '  allow:\n    - "%s"\n    - "%s"' "$3" "$4")"
  elif [ -n "${3:-}" ]; then
    allow_block="$(printf '  allow:\n    - "%s"' "$3")"
  fi
  cat > "$1" <<EOF
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: true
multicast:
  enabled: false
broadcast:
  mode: disabled
rendezvous:
$rv_block
  interval_seconds: 5
trust:
  policy: deny_unknown
$allow_block
cooperation:
  enabled: true
  heartbeat_seconds: 1
  expiry_seconds: 10
EOF
}

serve() { # checkout state
  ( cd "$1" && XDG_STATE_HOME="$2" exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$1.serve.log" 2>&1 ) &
  PIDS="$PIDS $!"
}
url_of() { jq -r '.url // empty' "$1/.ai/local/state/mcp/server.json" 2>/dev/null; }
has_url() { [ -n "$(url_of "$1")" ]; }
mj() { local cwd="$1" state="$2"; shift 2; ( cd "$cwd" && XDG_STATE_HOME="$state" "$RB" "$@" 2>/dev/null ); }

mkdir -p "$R/.ai/repo/mesh"
write_decl "$R/.ai/repo/mesh/majordomus.yaml" "" ""
git -C "$R" add .ai/repo/mesh/majordomus.yaml
git -C "$R" commit -qm "initial mesh declaration"

B="$T/wt-b"
git -C "$R" worktree add -q --detach "$B"

# Create identities for Node A and Node B
serve "$R" "$STATE_A"
serve "$B" "$STATE_B"
poll 300 has_url "$R" || { cat "$R.serve.log"; exit 1; }
poll 300 has_url "$B" || { cat "$B.serve.log"; exit 1; }
KEY_A="$(mj "$R" "$STATE_A" mesh identity --format json | jq -r '.identity.public_key // empty')"
KEY_B="$(mj "$B" "$STATE_B" mesh identity --format json | jq -r '.identity.public_key // empty')"
NODE_B="$(mj "$B" "$STATE_B" mesh identity --format json | jq -r '.identity.node_id // empty')"
[ "${#KEY_A}" = 64 ] && [ "${#KEY_B}" = 64 ] || { echo "    missing keys: A='$KEY_A' B='$KEY_B'"; cat "$R.serve.log" "$B.serve.log"; exit 1; }

mj "$R" "$STATE_A" serve stop >/dev/null || true
mj "$B" "$STATE_B" serve stop >/dev/null || true
poll 100 sh -c "! [ -s '$R/.ai/local/state/mcp/server.json' ]" || true
poll 100 sh -c "! [ -s '$B/.ai/local/state/mcp/server.json' ]" || true

# Commit both keys to trust.allow on trunk
write_decl "$R/.ai/repo/mesh/majordomus.yaml" "" "$KEY_A" "$KEY_B"
git -C "$R" commit -qam "allowlist both nodes"
git -C "$B" checkout -q --detach HEAD

# Configure Node A at v0.28.0 (follow: stable) and Node B at v0.26.0 (2 releases behind, follow: stable)
mkdir -p "$R/.ai/local/state/mesh" "$B/.ai/local/state/mesh"
printf '0.28.0\n' > "$R/.ai/local/state/mesh/version"
printf 'stable\n' > "$R/.ai/local/state/mesh/follow"
printf '0.26.0\n' > "$B/.ai/local/state/mesh/version"
printf 'stable\n' > "$B/.ai/local/state/mesh/follow"

serve "$R" "$STATE_A"
poll 300 has_url "$R" || { cat "$R.serve.log"; exit 1; }
URL_A="$(url_of "$R")"

write_decl "$B/.ai/repo/mesh/majordomus.yaml" "$URL_A" "$KEY_A" "$KEY_B"
serve "$B" "$STATE_B"
poll 300 has_url "$B" || { cat "$B.serve.log"; exit 1; }

linked_two() {
  mj "$R" "$STATE_A" mesh peers --format json \
    | jq -e '(.machines | length) == 2 and ([.machines[].runtimes[] | select(.version != "")] | length) == 2'
}
poll 300 linked_two || { echo "    A and B did not link with versions:"; mj "$R" "$STATE_A" mesh peers --format json; cat "$R.serve.log" "$B.serve.log"; exit 1; }

# 1. `mesh peers` prints each runtime's version and follow setting (text and JSON)
expect_exit 0 mj "$R" "$STATE_A" mesh peers
expect_grep 'version 0\.28\.0 follow stable'
expect_grep 'version 0\.26\.0 follow stable'
expect_grep 'rollout +v0\.28\.0 1/2 following \(rolling out\)'

expect_exit 0 mj "$R" "$STATE_A" mesh peers --format json
jq -e '
  .rollout.stable_tag == "v0.28.0" and
  .rollout.stable_rolled_out == false and
  ([.machines[].runtimes[] | {version, follow}] | sort_by(.version)) == [
    {"version": "0.26.0", "follow": "stable"},
    {"version": "0.28.0", "follow": "stable"}
  ]
' <<<"$LAST_OUT" >/dev/null || { echo "    unexpected mesh peers JSON:"; echo "$LAST_OUT"; exit 1; }

# 2. `mesh doctor` names a node more than one release behind the stable pointer
expect_exit 10 mj "$R" "$STATE_A" mesh doctor
expect_grep 'FAIL +rollout'
expect_grep "$NODE_B"
expect_grep 'v0\.26\.0'
expect_grep '2 releases behind stable v0\.28\.0'

# 3. Advance Node B to 0.28.0 -> release is rolled out when every enrolled following node reports it
printf '0.28.0\n' > "$B/.ai/local/state/mesh/version"
# Trigger a status refresh on B and wait for A to observe B's updated version via stream marks
mj "$B" "$STATE_B" mesh peers >/dev/null
rolled_out_now() {
  mj "$R" "$STATE_A" mesh peers --format json \
    | jq -e '.rollout.stable_rolled_out == true'
}
poll 300 rolled_out_now || { echo "    rollout did not converge after Node B updated to 0.28.0:"; mj "$R" "$STATE_A" mesh peers --format json; exit 1; }

expect_exit 0 mj "$R" "$STATE_A" mesh peers
expect_grep 'rollout +v0\.28\.0 2/2 following \(rolled out\)'

expect_exit 0 mj "$R" "$STATE_A" mesh doctor
expect_grep 'ok +rollout +stable v0\.28\.0: 2/2 following node\(s\) reporting \(rolled out\)'
echo "    ok: mesh peers prints version and follow, and mesh doctor names lagging nodes and verifies rollout"
