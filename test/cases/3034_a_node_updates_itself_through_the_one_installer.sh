# claims: installer-verifies-before-installing
# I2720 (ADR 0161): `majordomus self update` moves an installation to the stable release by
# running the published installer again, and decides nothing the installer could get wrong
# twice. Against a complete local release built from this tree and served over a real HTTP
# connection, an installed copy:
#   - reports an upgrade with --check and changes nothing;
#   - installs it with --apply, through the installer, into its own prefix, and its
#     launcher then reports the new version;
#   - is up to date afterwards;
#   - refuses a stable pointer that is older (forward only), a pre-release, and a version
#     the metadata does not list, installing nothing;
#   - keeps working when the installer refuses a download that does not match its metadata;
#   - keeps a superseded tree a running process still uses.
# A checkout's own build is told it is not an installed copy before anything is fetched.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
holder=""
trap '[ -n "$holder" ] && kill "$holder" 2>/dev/null; stop_http' EXIT INT TERM HUP

VERSION="$("$ROOT/scripts/release-version")"
older="0.0.9"
fixture() { # [--version V]
  MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" "$@" > "$T/fixture.log" 2>&1 \
    || { echo "    the fixture release could not be built ($*)"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
}

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
BIN="$HOMEDIR/.local/bin"
# MAJORDOMUS_BIN is the runner's way of naming the build under test; an installed launcher
# honours it too, so it would run that build instead of the installed tree.
run_env() { env -u MAJORDOMUS_SHARE -u MAJORDOMUS_LAUNCHER -u MAJORDOMUS_BIN HOME="$HOMEDIR" \
  MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 "$@"; }
installed() { run_env "$BIN/majordomus" version; }
self_update() { # <flags...> -> $out, $rc
  rc=0; out="$(cd "$T" && run_env "$BIN/majordomus" self update "$@" 2>&1)" || rc=$?
}

# --- a checkout's build is not an installed copy, and no request is made -----------------------
rc=0; out="$(cd "$T" && run_env "$MJB" self update --check 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    a checkout's build answered $rc, not 12: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "not an installed copy" || { echo "    the build was not told it is not installed: $out"; exit 1; }

# --- an older release is installed, and the stable pointer then names a newer one --------------
fixture --version "$older"
run_env sh "$FIX/install.sh" >/dev/null 2>&1 || { echo "    the older fixture did not install"; exit 1; }
[ "$(installed)" = "majordomus $older" ] || { echo "    the older install reports $(installed)"; exit 1; }
fixture
cp "$FIX/releases/latest.json" "$T/latest.good"

self_update --check
[ "$rc" = 0 ] || { echo "    --check exited $rc: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "verdict    upgrade" || { echo "    --check did not report the upgrade: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "stable     $VERSION" || { echo "    --check did not name the stable release: $out"; exit 1; }
[ "$(installed)" = "majordomus $older" ] || { echo "    --check changed the installation"; exit 1; }
self_update --check --format json
printf '%s\n' "$out" | grep -q '"verdict": "upgrade"' || { echo "    the JSON report carries no verdict: $out"; exit 1; }

# --- a running process holds the older tree, and --apply upgrades through the installer --------
old_tree="$HOMEDIR/.local/share/majordomus/versions/$older"
printf '#!/bin/sh\nsleep 300\n' > "$old_tree/libexec/holder"; chmod 755 "$old_tree/libexec/holder"
"$old_tree/libexec/holder" & holder=$!
self_update --apply
[ "$rc" = 0 ] || { echo "    --apply exited $rc: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "applied    $VERSION is installed" || { echo "    --apply did not report the installed version: $out"; exit 1; }
[ "$(installed)" = "majordomus $VERSION" ] || { echo "    after --apply the launcher reports $(installed)"; exit 1; }
[ -d "$old_tree/share" ] || { echo "    the upgrade removed a tree a running process uses"; exit 1; }
kill "$holder" 2>/dev/null; wait "$holder" 2>/dev/null || true; holder=""
ls "$T"/majordomus-self-update-* >/dev/null 2>&1 && { echo "    a downloaded installer was left behind"; exit 1; }

self_update --check
[ "$rc" = 0 ] && printf '%s\n' "$out" | grep -q "verdict    up to date" \
  || { echo "    after the upgrade --check did not say up to date ($rc): $out"; exit 1; }

# --- forward only: a stable pointer older than the installation is refused ---------------------
fixture --version "$older"
self_update --apply
[ "$rc" = 10 ] || { echo "    a downgrade was not refused with 10 ($rc): $out"; exit 1; }
printf '%s\n' "$out" | grep -q "forward only" || { echo "    the downgrade was not explained: $out"; exit 1; }
[ "$(installed)" = "majordomus $VERSION" ] || { echo "    a refused downgrade changed the installation"; exit 1; }

# --- stable only: a pointer that names a pre-release is refused ---------------------------------
sed -e "s/\"version\": \"$older\"/\"version\": \"9.0.0-rc.1\"/" -e "s/\"tag\": \"v$older\"/\"tag\": \"v9.0.0-rc.1\"/" \
  "$FIX/releases/latest.json" > "$T/pre.json"
cp "$T/pre.json" "$FIX/releases/latest.json"; cp "$T/pre.json" "$FIX/releases/v9.0.0-rc.1.json"
self_update --apply
[ "$rc" = 10 ] && printf '%s\n' "$out" | grep -q "pre-release" \
  || { echo "    a pre-release pointer was not refused ($rc): $out"; exit 1; }

# --- only what the metadata lists: a pointer whose tag has no metadata is refused --------------
sed -e "s/\"version\": \"9.0.0-rc.1\"/\"version\": \"9.0.0\"/" -e "s/\"tag\": \"v9.0.0-rc.1\"/\"tag\": \"v9.0.0\"/" \
  "$T/pre.json" > "$FIX/releases/latest.json"
rm -f "$FIX/releases/v9.0.0.json"
self_update --apply
[ "$rc" = 10 ] && printf '%s\n' "$out" | grep -q "does not list" \
  || { echo "    an unlisted version was not refused ($rc): $out"; exit 1; }
[ "$(installed)" = "majordomus $VERSION" ] || { echo "    a refused update changed the installation"; exit 1; }

# --- a download that does not match its metadata leaves the installation working ---------------
newer="9.1.0"
fixture --version "$newer"
name="$(sed -n 's/^    "[^"]*": { "name": "\([^"]*\)".*/\1/p' "$FIX/releases/v$newer.json" | while read -r n; do [ -f "$FIX/$n" ] && echo "$n"; done)"
[ -n "$name" ] || { echo "    the newer fixture has no archive"; exit 1; }
for n in $name; do printf 'tampered' >> "$FIX/$n"; done
self_update --apply
[ "$rc" = 12 ] || { echo "    a tampered archive was not refused with 12 ($rc): $out"; exit 1; }
printf '%s\n' "$out" | grep -q "still is" || { echo "    the refusal did not say the installation still works: $out"; exit 1; }
[ "$(installed)" = "majordomus $VERSION" ] || { echo "    a refused archive changed the installation"; exit 1; }
"$BIN/majordomus" version >/dev/null 2>&1 || { echo "    the installation stopped working"; exit 1; }
exit 0
