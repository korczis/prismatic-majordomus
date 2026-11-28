# majordomus-timeout: 300
# The hub's launchd agent or systemd user unit, and its hourly update, are written by a
# capability from the mesh declaration, never by hand (I4816, ADR 0199).
#
# Until 2026-11-28 every hub unit was a hand-written file, and they drifted: a MacBook's agent
# pointed at a deleted checkout for eleven days, lundra's unit was disabled while a shell hub
# served, and no machine carried the update timer, so every hub served 1.2.0 a day after
# 1.3.0 was published. What this case holds, against a service manager that is a stub on
# PATH and a home under the case's directory, so nothing of the machine is touched:
#
#   1. a repository with no declaration refuses `mesh hub install`: exit 10, nothing written;
#   2. over a declaration naming this machine as a hub, `mesh hub install` plans the units
#      and writes nothing; `--apply` writes them and loads them through the manager; a second
#      `--apply` finds every unit current and loaded and runs nothing;
#   3. `mesh hub` reads every unit back as rendered and loaded, and fails because no hub
#      answers on the declared port;
#   4. a checkout on a branch is refused, because a hub serves a dedicated checkout;
#      and the MCP tool majordomus_mesh_hub answers the same report;
#   5. `mesh hub update` with no release origin and no hub plans nothing it cannot read and
#      exits 10;
#   6. `mesh hub uninstall --apply` stops the units through the manager and removes them.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
export XDG_STATE_HOME="$T/state"
# no release origin answers, and the executable is this build, named on purpose
export MAJORDOMUS_RELEASE_BASE_URL=http://127.0.0.1:9
unset MAJORDOMUS_LAUNCHER

case "$(uname -s)" in
  Darwin) manager=launchd; hub_unit="$T/home/Library/LaunchAgents/dev.majordomus.hub.plist" ;;
  Linux)  manager=systemd; hub_unit="$T/home/.config/systemd/user/majordomus-hub.service" ;;
  *) echo "    (no launchd or systemd on $(uname -s); the crate's unit tests hold this case)"; exit 0 ;;
esac

# ---------------------------------------------------------------- the stub service manager
mkdir -p "$T/bin" "$T/home"
cat > "$T/bin/launchctl" <<'EOF'
#!/bin/sh
echo "launchctl $*" >> "$STUB_LOG"
case "$1" in
  print) [ -f "$STUB_DIR/loaded.$(basename "$2")" ] && { echo "state = running"; exit 0; }; exit 113 ;;
  bootstrap) touch "$STUB_DIR/loaded.$(basename "$3" .plist)" ;;
  bootout) rm -f "$STUB_DIR/loaded.$(basename "$2")" ;;
esac
exit 0
EOF
cat > "$T/bin/systemctl" <<'EOF'
#!/bin/sh
echo "systemctl $*" >> "$STUB_LOG"
shift # --user
case "$1" in
  is-enabled)
    case "$2" in *-update.service) echo static; exit 0 ;; esac
    [ -f "$STUB_DIR/enabled.$2" ] && { echo enabled; exit 0; }; echo disabled; exit 1 ;;
  is-active) [ -f "$STUB_DIR/active.$2" ] && { echo active; exit 0; }; echo inactive; exit 3 ;;
  enable) touch "$STUB_DIR/enabled.$2" ;;
  start|restart) touch "$STUB_DIR/active.$2" ;;
  disable) rm -f "$STUB_DIR/enabled.$3" "$STUB_DIR/active.$3" ;;
esac
exit 0
EOF
printf '#!/bin/sh\necho "loginctl $*" >> "$STUB_LOG"; echo yes\n' > "$T/bin/loginctl"
chmod +x "$T/bin/launchctl" "$T/bin/systemctl" "$T/bin/loginctl"
export STUB_LOG="$T/manager.log" STUB_DIR="$T/home"
: > "$STUB_LOG"
hub() { ( cd "$R" && HOME="$T/home" PATH="$T/bin:$PATH" "$RB" mesh hub "$@" ); }
change() { verb=$1; shift; hub "$verb" --executable "$RB" "$@"; }

# ---------------------------------------------------------------- 1. no declaration, no hub
R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit 1
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  rm -f .ai/repo/mesh/mesh.yaml
  git add -A && git commit -qm base
) || { echo "    the fixture repository could not be made"; exit 1; }
git -C "$R" checkout -q --detach
rc=0; change install --apply --format json > "$T/none.json" 2> "$T/none.err" || rc=$?
[ "$rc" = 10 ] || { echo "    install over no declaration exited $rc, expected 10"; cat "$T/none.err"; exit 1; }
jq -e '.ok == false and (.refusal | test("not a declared hub"))' "$T/none.json" >/dev/null \
  || { echo "    install over no declaration did not refuse:"; cat "$T/none.json"; exit 1; }
[ ! -e "$hub_unit" ] || { echo "    a refused install wrote $hub_unit"; exit 1; }

# ---------------------------------------------------------------- 2. this machine as a hub
addr="$( cd "$R" && "$RB" mesh firewall --format json 2>/dev/null | jq -r '.plan.local_addresses[0] // empty')"
if [ -z "$addr" ]; then
  echo "    (this machine has no non-loopback IPv4 address; the crate's unit tests hold the rest)"
  exit 0
fi
git -C "$R" checkout -q -
mkdir -p "$R/.ai/repo/mesh"
cat > "$R/.ai/repo/mesh/majordomus.yaml" <<EOF
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: true
multicast:
  enabled: false
tailscale:
  enabled: false
broadcast:
  mode: disabled
rendezvous:
  endpoints:
    - http://$addr:18791
  interval_seconds: 30
trust:
  policy: deny_unknown
EOF
git -C "$R" add .ai/repo/mesh/majordomus.yaml && git -C "$R" commit -qm "a hub declaration"
git -C "$R" checkout -q --detach

change install --format json > "$T/plan.json" 2> "$T/plan.err" || { echo "    the install plan failed"; cat "$T/plan.err"; cat "$T/plan.json"; exit 1; }
jq -e '.mode == "plan" and .port == 18791 and (.steps | length > 0) and all(.steps[]; .ran == false)' "$T/plan.json" >/dev/null \
  || { echo "    the plan is not a plan of the declared port:"; cat "$T/plan.json"; exit 1; }
[ ! -e "$hub_unit" ] || { echo "    a plan wrote $hub_unit"; exit 1; }

change install --apply --format json > "$T/apply.json" 2> "$T/apply.err" \
  || { echo "    install --apply failed"; cat "$T/apply.err"; cat "$T/apply.json"; exit 1; }
[ -f "$hub_unit" ] || { echo "    install --apply did not write $hub_unit"; cat "$T/apply.json"; exit 1; }
grep -q -- '--port' "$hub_unit" && grep -q '18791' "$hub_unit" && grep -q -- 'serve' "$hub_unit" \
  || { echo "    the hub unit does not serve the declared port:"; cat "$hub_unit"; exit 1; }
grep -q 'written by majordomus mesh hub install' "$hub_unit" || { echo "    the unit does not say what wrote it"; exit 1; }
if [ "$manager" = launchd ]; then
  grep -q 'launchctl bootstrap' "$STUB_LOG" || { echo "    nothing was bootstrapped:"; cat "$STUB_LOG"; exit 1; }
  [ -f "$T/home/Library/LaunchAgents/dev.majordomus.hub-update.plist" ] || { echo "    no update agent"; exit 1; }
else
  grep -q 'systemctl --user enable majordomus-hub-update.timer' "$STUB_LOG" || { echo "    the update timer was not enabled:"; cat "$STUB_LOG"; exit 1; }
  [ -f "$T/home/.config/systemd/user/majordomus-hub-update.timer" ] || { echo "    no update timer"; exit 1; }
fi

: > "$STUB_LOG"
change install --apply --format json > "$T/again.json" 2>/dev/null || { echo "    the second install failed"; cat "$T/again.json"; exit 1; }
jq -e '.ok and .steps == [] and all(.units[]; .state == "current" and .loaded)' "$T/again.json" >/dev/null \
  || { echo "    a second install changed something:"; jq '{steps, units}' "$T/again.json"; exit 1; }
! grep -qE 'bootstrap|enable|start' "$STUB_LOG" || { echo "    a second install ran the manager:"; cat "$STUB_LOG"; exit 1; }

# ---------------------------------------------------------------- 3. status reads it back
rc=0; hub --format json > "$T/status.json" 2> "$T/status.err" || rc=$?
[ "$rc" = 10 ] || { echo "    status with no hub answering exited $rc, expected 10"; cat "$T/status.json"; exit 1; }
jq -e '[.checks[] | select(.name | startswith("unit:"))] | length >= 2 and all(.[]; .level == "pass")' "$T/status.json" >/dev/null \
  || { echo "    status does not read the units back as rendered and loaded:"; jq '.checks' "$T/status.json"; exit 1; }
jq -e '.checks[] | select(.name == "answering") | .level == "fail"' "$T/status.json" >/dev/null \
  || { echo    "    status says a hub answers on 18791:"; jq '.checks' "$T/status.json"; exit 1; }
hub > "$T/status.txt" 2>/dev/null || true
expect_grep '^pass  unit:' "$T/status.txt"

# the MCP tool answers the same report, over a staging home so it asks no service manager
req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
{
  req 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4783","version":"0"}}'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call "{\"name\":\"majordomus_mesh_hub\",\"arguments\":{\"root\":\"$T/mcp-home\"}}"
} > "$T/mcp.in"
rc=0; ( cd "$R" && "$RB" mcp --standalone < "$T/mcp.in" ) > "$T/mcp.out" 2> "$T/mcp.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the MCP server exited $rc"; cat "$T/mcp.err"; exit 1; }
sed -n 2p "$T/mcp.out" | jq -e '.id == 2 and (.result.isError // false | not)' >/dev/null \
  || { echo "    majordomus_mesh_hub did not answer:"; cat "$T/mcp.out"; exit 1; }
sed -n 2p "$T/mcp.out" | jq -r '.result.content[0].text' | jq -e '.port == 18791 and (.checks | length > 3) and (.units | length >= 2)' >/dev/null \
  || { echo "    the MCP answer is not the hub report:"; sed -n 2p "$T/mcp.out"; exit 1; }

# the build is no installer launcher: unnamed, it is refused, because its hub would not
# follow the release
rc=0; hub install --apply --format json > "$T/nolauncher.json" 2>/dev/null || rc=$?
[ "$rc" = 10 ] && jq -e '.refusal | test("launcher")' "$T/nolauncher.json" >/dev/null \
  || { echo "    an executable that is no launcher was not refused (exit $rc):"; cat "$T/nolauncher.json"; exit 1; }

# ---------------------------------------------------------------- 4. a working checkout is refused
git -C "$R" checkout -q -
rc=0; change install --apply --format json > "$T/branch.json" 2>/dev/null || rc=$?
[ "$rc" = 10 ] && jq -e '.refusal | test("on a branch")' "$T/branch.json" >/dev/null \
  || { echo "    a checkout on a branch was not refused (exit $rc):"; cat "$T/branch.json"; exit 1; }
git -C "$R" checkout -q --detach

# ---------------------------------------------------------------- 5. update with nothing to read
rc=0; hub update --executable /nonexistent/majordomus --format json > "$T/update.json" 2>/dev/null || rc=$?
[ "$rc" = 10 ] && jq -e '.mode == "plan" and .decision.decision == "unknown" and .steps == []' "$T/update.json" >/dev/null \
  || { echo "    an update with nothing to read did not plan unknown (exit $rc):"; cat "$T/update.json"; exit 1; }

# ---------------------------------------------------------------- 6. uninstall
: > "$STUB_LOG"
change uninstall --apply --format json > "$T/gone.json" 2>/dev/null || { echo "    uninstall failed"; cat "$T/gone.json"; exit 1; }
[ ! -e "$hub_unit" ] || { echo "    uninstall left $hub_unit"; exit 1; }
grep -qE 'bootout|disable --now' "$STUB_LOG" || { echo "    uninstall did not stop the units:"; cat "$STUB_LOG"; exit 1; }
exit 0
