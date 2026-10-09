# claims: installer-verifies-before-installing
# I2106: the installer chooses a Linux build by what this machine's C library can load.
#
# On 2026-10-09 v0.18.0 was installed on a Jetson TX2 (Ubuntu 18.04, glibc 2.27). The
# installer saw a glibc and chose the aarch64 gnu build, which needs GLIBC_2.32: the shell
# launcher answered `version`, the install reported success, and the hub's executable
# exited with status 2 on its first start. The release also carries a statically linked
# musl build of the same version, which runs there.
#
# The machine is made a Linux one with an old glibc from here: `uname` says Linux aarch64
# and `getconf` answers glibc 2.27, so that the installer resolves the gnu target on any
# host. The fixture's gnu archive then carries an executable that fails as the dynamic
# loader fails on that machine; the musl archive carries a working one. Proved:
#   - the install succeeds with the musl build, says why, and names the missing symbol;
#   - a gnu build that runs is installed as before, with no word about musl (the control);
#   - when neither runs, nothing is installed and the existing installation is untouched.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
trap 'stop_http' EXIT INT TERM HUP
VERSION="$("$ROOT/scripts/release-version")"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
  || { echo "    the fixture release could not be built"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
cp "$FIX/releases/latest.json" "$T/latest.good"

# --- a Linux aarch64 machine whose C library is glibc 2.27 ------------------------------
fake="$T/fakebin"; mkdir -p "$fake"
cat > "$fake/uname" <<'FAKE'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo aarch64 ;; *) echo Linux ;; esac
FAKE
cat > "$fake/getconf" <<'FAKE'
#!/bin/sh
[ "$1" = GNU_LIBC_VERSION ] && { echo "glibc 2.27"; exit 0; }
exit 1
FAKE
chmod +x "$fake/uname" "$fake/getconf"

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
BIN="$HOMEDIR/.local/bin"
install_run() {
  env HOME="$HOMEDIR" PATH="$fake:$PATH" \
      MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" "$@" 2>&1
}
out="$(install_run --dry-run)"
printf '%s\n' "$out" | grep -q 'target              aarch64-unknown-linux-gnu' \
  || skip "this host's C library is not read as glibc, so the gnu target is not chosen: $(printf '%s' "$out" | grep 'C library')"

gnu="majordomus-v$VERSION-aarch64-unknown-linux-gnu"
musl="majordomus-v$VERSION-aarch64-unknown-linux-musl"
expect_file "$FIX/$gnu.tar.gz"
expect_file "$FIX/$musl.tar.gz"

# Replace the native executable inside one archive and record the new digest and size for
# that target only, as a release would have recorded them.
serve_with_native() { # <root> <triple> <script body for libexec/majordomus-cli>
  root=$1 triple=$2 body=$3
  rm -rf "$T/repack"; mkdir -p "$T/repack"
  tar -xzf "$FIX/$root.tar.gz" -C "$T/repack"
  # the fixture packs this machine's target and copies it under every other name, so the
  # tree inside is named for this machine; a real archive's tree is named for its target
  inner="$(ls "$T/repack")"; [ "$inner" = "$root" ] || mv "$T/repack/$inner" "$T/repack/$root"
  printf '#!/bin/sh\n%s\n' "$body" > "$T/repack/$root/libexec/majordomus-cli"
  chmod 0755 "$T/repack/$root/libexec/majordomus-cli"
  ( cd "$T/repack" && tar -czf "$FIX/$root.tar.gz" "$root" )
  sha="$(sha256_of_file "$FIX/$root.tar.gz")"; size="$(wc -c < "$FIX/$root.tar.gz" | tr -d ' ')"
  sed -e "/\"$triple\":/s/\"sha256\": \"[0-9a-f]\{64\}\"/\"sha256\": \"$sha\"/" \
      -e "/\"$triple\":/s/\"size\": [0-9]*/\"size\": $size/" \
      "$FIX/releases/latest.json" > "$T/latest.next" && mv "$T/latest.next" "$FIX/releases/latest.json"
}
LOADER_FAILS="echo \"\$0: /lib/aarch64-linux-gnu/libc.so.6: version \\\`GLIBC_2.32' not found (required by \$0)\" >&2; exit 1"
RUNS="echo 'majordomus $VERSION'"
pristine() { cp "$T/latest.good" "$FIX/releases/latest.json"; MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" >/dev/null 2>&1 || { echo "    the fixture could not be rebuilt"; exit 1; }; cp "$T/latest.good" "$FIX/releases/latest.json"; }

# --- the gnu build cannot be loaded: the musl build is installed, and the reason is said ----
serve_with_native "$gnu" aarch64-unknown-linux-gnu "$LOADER_FAILS"
serve_with_native "$musl" aarch64-unknown-linux-musl "$RUNS"
out="$(install_run)" || { echo "    the install failed although a musl build runs here: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'installed successfully' || { echo "    no success was reported: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'The glibc build cannot run here' || { echo "    the fallback was not explained: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "GLIBC_2.32' not found" || { echo "    the explanation does not name the loader's error: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "Downloading $musl.tar.gz" || { echo "    the musl artifact was not downloaded: $out"; exit 1; }
installed="$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli"
"$installed" --version >/dev/null 2>&1 || { echo "    the installed executable does not start"; exit 1; }
[ "$(cat "$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli")" = "$(printf '#!/bin/sh\n%s' "$RUNS")" ] \
  || { echo "    the installed executable is not the musl build's"; exit 1; }
[ -z "$(find "$HOMEDIR/.local/share/majordomus" -maxdepth 1 -name '.staging.*' 2>/dev/null)" ] \
  || { echo "    a staging directory was left behind"; exit 1; }

# --- the control: a gnu build that runs is installed, and musl is never mentioned ----------
rm -rf "$HOMEDIR"; mkdir -p "$HOMEDIR"; pristine
serve_with_native "$gnu" aarch64-unknown-linux-gnu "echo 'majordomus $VERSION' ; : gnu"
out="$(install_run)" || { echo "    a gnu build that runs was not installed: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'musl' && { echo "    musl was chosen although the gnu build runs: $out"; exit 1; }
grep -q ': gnu' "$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli" \
  || { echo "    the installed executable is not the gnu build's"; exit 1; }

# --- neither runs: nothing is installed, and what was installed keeps working --------------
before="$(sha256_of_file "$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli")"
pristine
serve_with_native "$gnu" aarch64-unknown-linux-gnu "$LOADER_FAILS"
serve_with_native "$musl" aarch64-unknown-linux-musl "echo 'exec format error' >&2; exit 126"
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "the release's executable does not start on this machine" \
  || { echo "    an install whose builds cannot start was not refused: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'Nothing was installed' || { echo "    the refusal does not say nothing was installed: $out"; exit 1; }
[ "$(sha256_of_file "$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli")" = "$before" ] \
  || { echo "    a refused install changed the installed executable"; exit 1; }
exit 0
