# An upgrade says when the macOS application firewall admits an earlier tree and not the one
# just installed (I2961). The firewall admits an executable by its path and every version has
# a path of its own: on 2026-10-17 a hub upgraded to 0.28.0 and restarted answered on its own
# machine, was dropped for every other one, and the install had reported success. The
# installer now asks the firewall after installing and prints the commands that admit the new
# path, and only then: an admitted path, a firewall that is off, a first install and a host
# without the tool say nothing. It never runs them. `socketfilterfw` is replaced through
# MAJORDOMUS_SOCKETFILTERFW, so the judgement is held on any host.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
trap 'stop_http' EXIT INT TERM HUP

VERSION="$("$ROOT/scripts/release-version")"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
  || { rc=$?; echo "    the fixture release could not be built (exit $rc)"; grep -v '^2026' "$T/fixture.log" | tail -20 | sed 's/^/      /'; exit 1; }

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
TREES="$HOMEDIR/.local/share/majordomus/versions"
NEW="$TREES/$VERSION/libexec/majordomus-cli"
OLD="$TREES/0.0.9/libexec/majordomus-cli"

# A socketfilterfw that answers from two files and records every call, so that a change to
# the firewall would be seen.
FW="$T/fw"; mkdir -p "$FW"
cat > "$FW/socketfilterfw" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$FW_DIR/calls"
case "$1" in
  --getglobalstate) cat "$FW_DIR/state" ;;
  --listapps) cat "$FW_DIR/apps" ;;
  *) exit 3 ;;
esac
EOF
chmod +x "$FW/socketfilterfw"
on() { echo 'Firewall is enabled. (State = 1)' > "$FW/state"; }
off() { echo 'Firewall is disabled. (State = 0)' > "$FW/state"; }
apps() { # <path> <Allow|Block> ... -> the --listapps listing
  : > "$FW/apps"; n=0
  while [ $# -ge 2 ]; do
    n=$((n + 1))
    printf '%s : %s \n             (%s incoming connections) \n' "$n" "$1" "$2" >> "$FW/apps"
    shift 2
  done
}
install_run() {
  env HOME="$HOMEDIR" FW_DIR="$FW" MAJORDOMUS_SOCKETFILTERFW="$FW/socketfilterfw" \
      MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" --force "$@" 2>&1
}
says_gap() { printf '%s\n' "$1" | grep -q 'firewall admits an earlier'; }

# ---------------------------------------------------------------- 1. an older tree admitted, the new one not
on; apps "$OLD" Allow "$HOME/some/other/tool" Allow
out="$(install_run)" || { echo "    the install failed: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "installed successfully" || { echo "    no success line: $out"; exit 1; }
says_gap "$out" || { echo "    the firewall gap was not named:"; printf '%s\n' "$out" | tail -12 | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -qF "    $FW/socketfilterfw --add \"$NEW\"" \
  || { echo "    the admitting --add line does not name the new path: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "    $FW/socketfilterfw --unblockapp \"$NEW\"" \
  || { echo "    the admitting --unblockapp line does not name the new path: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'mesh firewall apply' || { echo "    the mesh firewall apply alternative is missing"; exit 1; }
printf '%s\n' "$out" | grep -q 'as root' || { echo "    the commands do not say they need root"; exit 1; }
# it only asked
if grep -vE '^--(getglobalstate|listapps)$' "$FW/calls"; then echo "    the installer changed the firewall"; exit 1; fi

# ---------------------------------------------------------------- 2. the new tree blocked counts as not admitted
apps "$OLD" Allow "$NEW" Block
out="$(install_run)"; says_gap "$out" || { echo "    a blocked new path was taken for admitted"; exit 1; }

# ---------------------------------------------------------------- 3. silence where nothing is wrong
apps "$OLD" Allow "$NEW" Allow
out="$(install_run)"; says_gap "$out" && { echo "    an admitted path was named as a gap"; exit 1; }
apps "$HOME/some/other/tool" Allow
out="$(install_run)"; says_gap "$out" && { echo "    a firewall that never admitted a tree was named as a gap"; exit 1; }
off; apps "$OLD" Allow
out="$(install_run)"; says_gap "$out" && { echo "    a firewall that is off was named as a gap"; exit 1; }
out="$(env MAJORDOMUS_SOCKETFILTERFW="$T/absent" HOME="$HOMEDIR" \
  MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
  sh "$ROOT/site/static/install.sh" --force 2>&1)" || { echo "    the install failed without the tool"; exit 1; }
says_gap "$out" && { echo "    a host without the tool was told of a gap"; exit 1; }
exit 0
