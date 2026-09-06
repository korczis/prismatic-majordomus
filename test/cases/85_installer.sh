# The installer, end to end, against a complete local release: a real archive built from
# this tree, real metadata rendered by the same renderer the site publishes, served over a
# real HTTP connection. Nothing here reaches the internet, and nothing here is mocked
# except the network's address.
#
# What is proved: a first install, an idempotent second, an upgrade, a refused downgrade,
# a pinned version, --init, the PATH hint, and — the half that matters — that every failure
# leaves a working installation working: a wrong digest, a truncated download, an archive
# that escapes its own directory, an archive that carries a link, a missing release, and a
# destination that cannot be written.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || { echo "    skip: no python3 or node to serve a fixture release"; exit 0; }
trap 'stop_http' EXIT INT TERM HUP

VERSION="$("$ROOT/scripts/release-version")"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
  || { rc=$?; echo "    the fixture release could not be built (exit $rc, base=$HTTP_BASE, bin=$MJB)"; grep -v '^2026' "$T/fixture.log" | tail -20 | sed 's/^/      /'; exit 1; }
expect_file "$FIX/releases/latest.json"

HOMEDIR="$T/home"; mkdir -p "$HOMEDIR"
BIN="$HOMEDIR/.local/bin"
install_run() {
  env HOME="$HOMEDIR" \
      MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" "$@" 2>&1
}

# --- the artifact this machine will download is what the model says it is ----------------
host="$("$MJB" distribution --repo "$ROOT" build --format json 2>/dev/null | sed -n 's/.*"target": "\([^"]*\)".*/\1/p' | head -n 1)"
name="$("$MJB" distribution --repo "$ROOT" artifact --target "$host" --tag "v$VERSION" --format text 2>/dev/null | sed -n 1p)"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-verify" \
  --archive "$FIX/$name" --target "$host" --tag "v$VERSION" --native >/dev/null \
  || { echo "    the fixture artifact is not what the model says it is"; exit 1; }

# --- dry run changes nothing --------------------------------------------------------------
out="$(install_run --dry-run)"
printf '%s\n' "$out" | grep -q "resolved release    v$VERSION" || { echo "    dry run resolved nothing: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "already installed   none" || { echo "    dry run misreported the installation"; exit 1; }
[ -e "$BIN/majordomus" ] && { echo "    a dry run installed something"; exit 1; }

# --- the first install ---------------------------------------------------------------------
out="$(install_run)"
printf '%s\n' "$out" | grep -q "installed successfully" || { echo "    the install did not report success: $out"; exit 1; }
expect_file "$BIN/majordomus"
expect_file "$BIN/majordomus-mcp"
expect_file "$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli"
expect_file "$HOMEDIR/.local/share/majordomus/versions/$VERSION/LICENSE"
[ "$(file_mode "$BIN/majordomus")" = 755 ] || { echo "    the launcher is mode $(file_mode "$BIN/majordomus"), not 755"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    the installed tool reports the wrong version"; exit 1; }
[ "$("$HOMEDIR/.local/share/majordomus/versions/$VERSION/libexec/majordomus-cli" --version)" = "majordomus $VERSION" ] \
  || { echo "    the installed executable reports the wrong version"; exit 1; }
# the PATH hint is given, because this HOME's bin directory is on nobody's PATH
printf '%s\n' "$out" | grep -q 'is not on your PATH' || { echo "    no PATH hint was given"; exit 1; }
# ... and nothing was written to a shell profile
[ -e "$HOMEDIR/.zshrc" ] || [ -e "$HOMEDIR/.bashrc" ] || [ -e "$HOMEDIR/.profile" ] && { echo "    the installer edited a shell profile"; exit 1; }

# --- and it serves MCP without a toolchain -------------------------------------------------
printf '' | env HOME="$HOMEDIR" MAJORDOMUS_NO_BUILD=1 "$BIN/majordomus-mcp" --standalone --repo "$ROOT" >/dev/null 2>&1 \
  || { echo "    the installed MCP launcher could not run without building"; exit 1; }

# --- installing again says so and changes nothing --------------------------------------------
before="$(ls -la "$BIN/majordomus")"
out="$(install_run)"
printf '%s\n' "$out" | grep -q "is already installed" || { echo "    a second install did not say so: $out"; exit 1; }
[ "$(ls -la "$BIN/majordomus")" = "$before" ] || { echo "    a second install rewrote the launcher"; exit 1; }

# --- init, from the installed tool, in a repository that has none ----------------------------
mkdir -p "$T/project" && ( cd "$T/project" && git init -q . )
( cd "$T/project" && install_run --init >/dev/null 2>&1 ) || { echo "    --init failed"; exit 1; }
expect_file "$T/project/.ai/repo/policy.yaml"
# and the hook line it printed names the launcher, not the versioned tree it will replace
mkdir -p "$T/project2" && ( cd "$T/project2" && git init -q . )
hookline="$( cd "$T/project2" && env HOME="$HOMEDIR" "$BIN/majordomus" init 2>&1 | grep 'pre-commit:' || true )"
case "$hookline" in
  *"$BIN/majordomus doctor"*) ;;
  *) echo "    the hook line does not name the launcher, so an upgrade would break it: $hookline"; exit 1 ;;
esac

# --- a wrong digest fails closed, and the working installation keeps working ------------------
cp "$FIX/releases/latest.json" "$T/latest.good"
sed "s/\"sha256\": \"[0-9a-f]\{64\}\"/\"sha256\": \"$(printf 'b%.0s' $(seq 64))\"/" \
  "$T/latest.good" > "$FIX/releases/latest.json"
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "does not match its digest" || { echo "    a wrong digest was accepted: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "nothing was installed" || { echo "    the failure did not say the install was untouched"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a failed install destroyed the working one"; exit 1; }

# --- a truncated download is told apart from a wrong one ---------------------------------------
cp "$T/latest.good" "$FIX/releases/latest.json"
cp "$FIX/$name" "$T/$name.good"
dd if="$T/$name.good" of="$FIX/$name" bs=1024 count=8 2>/dev/null
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "not the size the release records" || { echo "    a truncated download was not named: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a truncated download destroyed the working install"; exit 1; }
cp "$T/$name.good" "$FIX/$name"

# --- an archive that escapes its own directory is refused --------------------------------------
serve_malicious() { # <archive built at $T/evil.tar.gz>
  sha="$(sha256_of_file "$T/evil.tar.gz")"
  size="$(wc -c < "$T/evil.tar.gz" | tr -d ' ')"
  cp "$T/evil.tar.gz" "$FIX/$name"
  sed -e "s/\"sha256\": \"[0-9a-f]\{64\}\"/\"sha256\": \"$sha\"/" \
      -e "s/\"size\": [0-9]*/\"size\": $size/" "$T/latest.good" > "$FIX/releases/latest.json"
}
sha256_of_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else openssl dgst -sha256 "$1" | sed 's/.*= //'; fi
}

mkdir -p "$T/mal/elsewhere" && echo pwned > "$T/mal/elsewhere/evil"
( cd "$T/mal" && tar -czf "$T/evil.tar.gz" elsewhere )
serve_malicious
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "outside" || { echo "    an archive outside its own root was accepted: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a malicious archive destroyed the working install"; exit 1; }

# --- an archive that carries a link is refused ---------------------------------------------------
root="${name%.tar.gz}"
rm -rf "$T/mal2"; mkdir -p "$T/mal2/$root/bin"
cp "$ROOT/bin/majordomus" "$T/mal2/$root/bin/majordomus"
ln -s /etc/passwd "$T/mal2/$root/bin/secrets"
( cd "$T/mal2" && tar -czf "$T/evil.tar.gz" "$root" )
serve_malicious
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "not a file or directory" || { echo "    an archive carrying a link was accepted: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a link archive destroyed the working install"; exit 1; }

# --- an archive whose tool reports another version is refused --------------------------------------
rm -rf "$T/mal3"; mkdir -p "$T/mal3"
tar -xzf "$T/$name.good" -C "$T/mal3"
sed 's/^MJ_VERSION=".*"/MJ_VERSION="9.9.9"/' "$T/mal3/$root/bin/majordomus" > "$T/mal3/$root/bin/majordomus.new"
mv "$T/mal3/$root/bin/majordomus.new" "$T/mal3/$root/bin/majordomus"
chmod 0755 "$T/mal3/$root/bin/majordomus"
( cd "$T/mal3" && tar -czf "$T/evil.tar.gz" "$root" )
serve_malicious
out="$(install_run --force || true)"
printf '%s\n' "$out" | grep -q "reports another version" || { echo "    a mismatched release was accepted: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a mismatched release destroyed the working install"; exit 1; }

# --- restore, then the pinned form ---------------------------------------------------------------
cp "$T/$name.good" "$FIX/$name"
cp "$T/latest.good" "$FIX/releases/latest.json"
out="$(install_run --version "v$VERSION")"
printf '%s\n' "$out" | grep -q "is already installed" || { echo "    a pinned install of the installed version did not say so: $out"; exit 1; }
out="$(install_run --version "v0.0.1" || true)"
printf '%s\n' "$out" | grep -q "no release metadata for" || { echo "    a version that was never published was not named: $out"; exit 1; }

# --- an upgrade, and a refused downgrade ------------------------------------------------------------
older="0.0.9"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" --version "$older" >/dev/null 2>&1 \
  || { echo "    an older fixture release could not be built"; exit 1; }
# latest.json now points at the older release: the installer must refuse to move backwards
out="$(install_run)"
printf '%s\n' "$out" | grep -q "newer than the latest stable release" \
  || { echo "    the installer offered a downgrade: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    a refused downgrade changed the installation"; exit 1; }
# pinned, the older version installs, and the newer one then upgrades it
install_run --version "v$older" >/dev/null 2>&1 || { echo "    a pinned older version did not install"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $older" ] || { echo "    the pinned older version is not what ran"; exit 1; }
[ -d "$HOMEDIR/.local/share/majordomus/versions/$VERSION" ] && { echo "    the superseded tree was left behind"; exit 1; }
cp "$T/latest.good" "$FIX/releases/latest.json"
out="$(install_run)"
printf '%s\n' "$out" | grep -q "installed successfully" || { echo "    the upgrade did not run: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    the upgrade did not take"; exit 1; }

# --- a destination that cannot be written is named, and sudo is never reached -------------------------
mkdir -p "$T/readonly" && chmod 500 "$T/readonly"
out="$(env HOME="$HOMEDIR" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
        sh "$ROOT/site/static/install.sh" --force --install-dir "$T/readonly/bin" 2>&1 || true)"
printf '%s\n' "$out" | grep -q "never uses sudo" || { echo "    an unwritable destination was not explained: $out"; exit 1; }
chmod 700 "$T/readonly"

# --- a release that does not exist at all ---------------------------------------------------------------
out="$(env HOME="$T/other" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE/nothing-here" MAJORDOMUS_INSECURE_BASE_URL=1 \
        sh "$ROOT/site/static/install.sh" 2>&1 || true)"
printf '%s\n' "$out" | grep -q "no stable release is published yet" || { echo "    a missing release was not named: $out"; exit 1; }

# --- nothing was left behind ------------------------------------------------------------------------------
find "$HOMEDIR/.local/share/majordomus" -maxdepth 1 -name '.staging.*' | grep -q . \
  && { echo "    a staging directory survived"; exit 1; }
find "$HOMEDIR/.local/share/majordomus" -maxdepth 1 -name '.lock' | grep -q . \
  && { echo "    the lock survived"; exit 1; }
exit 0
