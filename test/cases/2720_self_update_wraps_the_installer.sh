# majordomus-covers: none
# `majordomus self update` moves an installation to the published stable release by re-running
# the published install.sh, and never by a second implementation of download, verification and
# swap (ADR 0161, I2720). Against a complete local release served over a real HTTP connection,
# as case 85 serves it: an older release is installed by install.sh, then the installed tool
# updates itself.
#
# What is proved: --check names the running and the stable version and that an upgrade applies,
# and writes nothing; --apply runs the installer with this installation's own prefix and
# launcher directory and reads the new version back from the launcher; then nothing applies; a
# downgrade, a pre-release and a version the metadata does not list are each refused with exit
# 10 and their reason, and none of them changes the installation.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
trap 'stop_http' EXIT INT TERM HUP

REAL_VERSION="$("$ROOT/scripts/release-version")"; VERSION="${REAL_VERSION%%-*}"
fixture() {
  MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" "$@" \
    > "$T/fixture.log" 2>&1 \
    || { echo "    the fixture release could not be built ($*)"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
}
fixture
cp "$FIX/releases/latest.json" "$T/latest.good"
# the installer self update fetches is the one this tree publishes, served from the same base
cp "$ROOT/site/static/install.sh" "$FIX/install.sh"
older="0.0.9"
fixture --version "$older"

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
BIN="$HOMEDIR/.local/bin"; PREFIX="$HOMEDIR/.local/share/majordomus"
release_env() {
  env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN HOME="$HOMEDIR" MAJORDOMUS_NO_BUILD=1 \
    MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 "$@"
}
release_env sh "$ROOT/site/static/install.sh" --version "v$older" > "$T/install.log" 2>&1 \
  || { echo "    the older release did not install"; tail -20 "$T/install.log"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $older" ] || { echo "    the older release is not what runs"; exit 1; }
cp "$T/latest.good" "$FIX/releases/latest.json"

# the command needs a repository, as every capability of the executable does: a foreign one
mkdir -p "$T/project" && ( cd "$T/project" && git init -q . )
( cd "$T/project" && release_env "$BIN/majordomus" init >/dev/null 2>&1 ) || { echo "    init failed"; exit 1; }
su() { ( cd "$T/project" && release_env "$BIN/majordomus" self update "$@" 2>&1 ); }

# --- --check reports, and writes nothing --------------------------------------------------------
before="$(ls -laR "$PREFIX" "$BIN")"
rc=0; out="$(su --check)" || rc=$?
[ "$rc" -eq 0 ] || { echo "    --check exited $rc: $out"; exit 1; }
# a path is compared with its repeated slashes collapsed: $T may end in one, the launcher passes
# it on as written, and the executable's own path comes back without it
squash() { sed 's#//*#/#g'; }
for want in "running      v$older" "stable       v$VERSION" "verdict      upgrade" "applies      yes" \
            "installation $PREFIX"; do
  want="$(printf '%s' "$want" | squash)"
  printf '%s\n' "$out" | squash | grep -qF "$want" || { echo "    --check did not say '$want': $out"; exit 1; }
done
[ "$(ls -laR "$PREFIX" "$BIN")" = "$before" ] || { echo "    --check changed the installation"; exit 1; }
# the default is the check
rc=0; out="$(su)" || rc=$?
printf '%s\n' "$out" | grep -qF "verdict      upgrade" || { echo "    the bare command did not report: $out"; exit 1; }
[ "$(ls -laR "$PREFIX" "$BIN")" = "$before" ] || { echo "    the bare command changed the installation"; exit 1; }

# --- --apply installs through install.sh, and the new launcher says so --------------------------
rc=0; out="$(su --apply)" || rc=$?
[ "$rc" -eq 0 ] || { echo "    --apply exited $rc: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "ran          sh " || { echo "    --apply did not name the installer run: $out"; exit 1; }
printf '%s\n' "$out" | squash | grep -qF -- "$(printf '%s' "--version v$VERSION --prefix $PREFIX --install-dir $BIN" | squash)" \
  || { echo "    the installer was not handed this installation's places: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "installed    v$VERSION" || { echo "    no version was read back: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    the upgrade did not take"; exit 1; }
expect_file "$PREFIX/versions/$VERSION/libexec/majordomus-cli"

# --- now nothing applies --------------------------------------------------------------------
rc=0; out="$(su --check)" || rc=$?
[ "$rc" -eq 0 ] || { echo "    a current installation's check exited $rc: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "verdict      current" || { echo "    a current installation was offered something: $out"; exit 1; }
rc=0; out="$(su --apply)" || rc=$?
[ "$rc" -eq 0 ] && ! printf '%s\n' "$out" | grep -qF "ran " || { echo "    --apply on a current installation ran something: $out"; exit 1; }

# --- a downgrade, a pre-release and an unlisted version are refused, and change nothing ---------
before="$(ls -laR "$PREFIX" "$BIN")"
rc=0; out="$(su --apply --version "v$older")" || rc=$?
[ "$rc" -eq 10 ] || { echo "    a downgrade exited $rc, not 10: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "older than the running v$VERSION" || { echo "    the downgrade's reason is missing: $out"; exit 1; }

pre="9.9.9-rc.1"
sed -e "s/\"version\": \"$VERSION\"/\"version\": \"$pre\"/" -e "s/\"tag\": \"v$VERSION\"/\"tag\": \"v$pre\"/" \
  "$T/latest.good" > "$FIX/releases/v$pre.json"
rc=0; out="$(su --apply --version "v$pre")" || rc=$?
[ "$rc" -eq 10 ] || { echo "    a pre-release exited $rc, not 10: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "is a pre-release" || { echo "    the pre-release's reason is missing: $out"; exit 1; }

rc=0; out="$(su --apply --version v0.0.2)" || rc=$?
[ "$rc" -eq 10 ] || { echo "    an unlisted version exited $rc, not 10: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "lists no release v0.0.2" || { echo "    the unlisted version's reason is missing: $out"; exit 1; }
[ "$(ls -laR "$PREFIX" "$BIN")" = "$before" ] || { echo "    a refused update changed the installation"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a refusal moved the installation"; exit 1; }
exit 0
