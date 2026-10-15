# claims: installer-verifies-before-installing
# The installer, end to end, against a complete local release: a real archive built from
# this tree, real metadata rendered by the same renderer the site publishes, served over a
# real HTTP connection. Nothing here reaches the internet, and nothing here is mocked
# except the network's address.
#
# What is proved: a first install, an idempotent second, an upgrade, a refused downgrade,
# a pinned version, --init, the PATH hint, an upgrade that keeps a superseded tree a process
# still runs from (and removes it once nothing does), and — the half that matters — that every failure
# leaves a working installation working: a wrong digest, no digest tool or a malformed digest,
# a truncated download, an archive that escapes its own directory, an archive that carries a
# link, a missing release, and a destination that cannot be written. And the closing report
# says what was verified: provenance through the GitHub CLI, or the checksum only (I2165).
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
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

# --- no digest tool, or a digest tool that prints no digest, fails closed ---------------------
# A PATH holding every command this machine has except the two digest tools the installer
# accepts (anything else, whatever it is called, must not be a fallback), then the same PATH
# behind a sha256sum that prints junk.
NODIGEST="$T/nodigest"; mkdir -p "$NODIGEST"
old_ifs=$IFS; IFS=:
for d in $PATH; do
  [ -d "$d" ] || continue
  for f in "$d"/*; do
    b="${f##*/}"
    case "$b" in sha256sum|openssl) continue ;; esac
    if [ -x "$f" ] && [ ! -e "$NODIGEST/$b" ]; then ln -s "$f" "$NODIGEST/$b"; fi
  done
done
IFS=$old_ifs
digest_run() { # <PATH> -> $out and $digest_rc
  digest_rc=0
  out="$(env PATH="$1" HOME="$HOMEDIR" \
      MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      /bin/sh "$ROOT/site/static/install.sh" --force 2>&1)" || digest_rc=$?
}
before_launcher="$(sha256_of_file "$BIN/majordomus")"
digest_run "$NODIGEST"
[ "$digest_rc" != 0 ] || { echo "    an install without a digest tool exited 0: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "no way to compute a SHA-256 digest" \
  || { echo "    a missing digest tool was not named: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "Nothing was installed or replaced" \
  || { echo "    a missing digest tool did not say nothing was installed: $out"; exit 1; }
[ "$(sha256_of_file "$BIN/majordomus")" = "$before_launcher" ] \
  || { echo "    an install without a digest tool changed the launcher"; exit 1; }
JUNKDIGEST="$T/junkdigest"; mkdir -p "$JUNKDIGEST"
# 64 characters, none of them hex: a length check alone would accept it, so the hex check
# is held too; either check removed turns the refusal into a digest mismatch.
printf '#!/bin/sh\necho "%s  $1"\n' "$(printf '%064d' 0 | tr 0 z)" > "$JUNKDIGEST/sha256sum"
chmod 755 "$JUNKDIGEST/sha256sum"
digest_run "$JUNKDIGEST:$NODIGEST"
[ "$digest_rc" != 0 ] || { echo "    an install with a malformed digest exited 0: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "the digest of the download could not be computed" \
  || { echo "    a malformed digest was not named: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "Nothing was installed or replaced" \
  || { echo "    a malformed digest did not say nothing was installed: $out"; exit 1; }
[ "$(sha256_of_file "$BIN/majordomus")" = "$before_launcher" ] \
  || { echo "    an install with a malformed digest changed the launcher"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] \
  || { echo "    a digest failure destroyed the working install"; exit 1; }

# --- an archive that escapes its own directory is refused --------------------------------------
serve_malicious() { # <archive built at $T/evil.tar.gz>
  sha="$(sha256_of_file "$T/evil.tar.gz")"
  size="$(wc -c < "$T/evil.tar.gz" | tr -d ' ')"
  cp "$T/evil.tar.gz" "$FIX/$name"
  sed -e "s/\"sha256\": \"[0-9a-f]\{64\}\"/\"sha256\": \"$sha\"/" \
      -e "s/\"size\": [0-9]*/\"size\": $size/" "$T/latest.good" > "$FIX/releases/latest.json"
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
# the tool reads its version from share/version.txt beside it, so that is what is tampered with
sed 's/^version=.*/version=9.9.9/' "$T/mal3/$root/share/version.txt" > "$T/mal3/$root/share/version.txt.new"
mv "$T/mal3/$root/share/version.txt.new" "$T/mal3/$root/share/version.txt"
grep -qx 'version=9.9.9' "$T/mal3/$root/share/version.txt" || { echo "    the tamper did not apply"; exit 1; }
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
printf '%s\n' "$out" | grep -q "no release v0.0.1 is published" || { echo "    a version that was never published was not named: $out"; exit 1; }
# a 404 is the project's answer and is said as one; a transport failure is not collapsed into it
printf '%s\n' "$out" | grep -q "HTTP 404" || { echo "    the pinned miss did not distinguish a 404 from a failure to reach: $out"; exit 1; }
printf '%s\n' "$out" | grep -qv "could not be retrieved" || { echo "    a 404 was reported as a retrieval failure: $out"; exit 1; }

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
# a process still runs from the older tree — a server started before the upgrade — so the
# upgrade must keep that tree: removing it leaves the server serving 404s for its own share/
old_tree="$HOMEDIR/.local/share/majordomus/versions/$older"
printf '#!/bin/sh\nsleep 300\n' > "$old_tree/libexec/holder"; chmod 755 "$old_tree/libexec/holder"
"$old_tree/libexec/holder" & holder=$!
trap 'kill $holder 2>/dev/null; stop_http' EXIT INT TERM HUP
cp "$T/latest.good" "$FIX/releases/latest.json"
out="$(install_run)"
printf '%s\n' "$out" | grep -q "installed successfully" || { echo "    the upgrade did not run: $out"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] || { echo "    the upgrade did not take"; exit 1; }
[ -d "$old_tree/share" ] || { echo "    the upgrade removed a tree a running process uses"; exit 1; }
printf '%s\n' "$out" | grep -q "kept the superseded tree $old_tree" \
  || { echo "    the kept tree was not named: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "$holder .*libexec/holder" || { echo "    the holder was not named: $out"; exit 1; }
# once nothing runs from it, the next install removes it
kill "$holder"; wait "$holder" 2>/dev/null || true
trap 'stop_http' EXIT INT TERM HUP
install_run --force >/dev/null 2>&1 || { echo "    a forced reinstall failed"; exit 1; }
[ -d "$old_tree" ] && { echo "    an unused superseded tree was left behind"; exit 1; }

# --- a destination that cannot be written is named, and sudo is never reached -------------------------
mkdir -p "$T/readonly" && chmod 500 "$T/readonly"
out="$(env HOME="$HOMEDIR" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
        sh "$ROOT/site/static/install.sh" --force --install-dir "$T/readonly/bin" 2>&1 || true)"
printf '%s\n' "$out" | grep -q "never uses sudo" || { echo "    an unwritable destination was not explained: $out"; exit 1; }
chmod 700 "$T/readonly"

# --- a release that does not exist at all ---------------------------------------------------------------
out="$(env HOME="$T/other" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE/nothing-here" MAJORDOMUS_INSECURE_BASE_URL=1 \
        sh "$ROOT/site/static/install.sh" 2>&1 || true)"
printf '%s\n' "$out" | grep -q "no stable release is currently published" || { echo "    a missing release was not named: $out"; exit 1; }

# --- the installer is published and the release is not: the v0.2.0 state, exactly ---------------------------
# For a day and a half this was the repository's public answer. install.sh was served, every
# other release URL was a 404, and the one-line command in the README ended in a diagnostic.
# The behaviour is correct and stays that way: the run must fail, say which of the two things
# is missing, and leave the installation that was already there working and byte-identical.
before_launcher="$(sha256_of_file "$BIN/majordomus")"
before_tree="$(ls "$HOMEDIR/.local/share/majordomus/versions")"
mv "$FIX/releases/latest.json" "$T/latest.withheld"
rc=0
out="$(install_run)" || rc=$?
mv "$T/latest.withheld" "$FIX/releases/latest.json"
[ "$rc" != 0 ] || { echo "    a missing latest.json exited 0"; exit 1; }
printf '%s\n' "$out" | grep -q "no stable release" \
  || { echo "    a withheld latest.json was not named as a missing release: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "Nothing was installed or replaced" \
  || { echo "    the failure did not state that nothing was replaced: $out"; exit 1; }
# and the promise in that sentence is true of the filesystem, not only of the text
[ "$(sha256_of_file "$BIN/majordomus")" = "$before_launcher" ] \
  || { echo "    the launcher changed although nothing was supposed to be replaced"; exit 1; }
[ "$(ls "$HOMEDIR/.local/share/majordomus/versions")" = "$before_tree" ] \
  || { echo "    the installed versions changed although nothing was supposed to be replaced"; exit 1; }
[ "$("$BIN/majordomus" version)" = "majordomus $VERSION" ] \
  || { echo "    the installation that was there no longer runs"; exit 1; }

# --- and with the metadata back, the same command installs ---------------------------------------------------
# The complement of the case above, on the same fixture: the only thing that changed is that
# releases/latest.json exists again.
rm -rf "$T/clean"; mkdir -p "$T/clean"
out="$(env HOME="$T/clean" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
        sh "$ROOT/site/static/install.sh" 2>&1)" \
  || { echo "    the installer failed with the metadata in place: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "installed successfully" || { echo "    the clean install did not report success: $out"; exit 1; }
[ "$("$T/clean/.local/bin/majordomus" version)" = "majordomus $VERSION" ] \
  || { echo "    the clean install did not report $VERSION"; exit 1; }

# --- the closing report says what was verified: provenance, or the checksum only (I2165) -----
# A plain-HTTP fixture is described by no attestation, so the default install above asks
# nobody and says so. A named GitHub CLI is always asked: three stubs stand in for it — one
# that verifies, one that refuses the way GitHub refuses a digest it holds no attestation
# for, and one that is not signed in — and each one's call is recorded.
printf '%s\n' "$out" | grep -q "checksum only — provenance not verified: the release origin is plain HTTP" \
  || { echo "    the install did not say it verified the checksum only: $out"; exit 1; }
GHS="$T/gh-stubs"; mkdir -p "$GHS"
REPO="$(sed -n "s/^MJ_REPOSITORY='\\(.*\\)'\$/\\1/p" "$ROOT/site/static/install.sh")"
[ -n "$REPO" ] || { echo "    the installer names no repository"; exit 1; }
cat > "$GHS/gh-ok" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$GHS/calls"
exit 0
EOF
cat > "$GHS/gh-refuses" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$GHS/calls"
case "\$1" in auth) exit 0 ;; esac
echo "Error: HTTP 404: Not Found (attestations for this digest)" >&2
exit 1
EOF
cat > "$GHS/gh-signed-out" <<'EOF'
#!/bin/sh
case "$1" in auth) exit 1 ;; esac
echo "the stub was asked to verify while signed out" >&2
exit 7
EOF
chmod 0755 "$GHS"/gh-*
prov_run() { # home gh [env...]
  h="$1"; g="$2"; shift 2
  rm -rf "$h"; mkdir -p "$h"
  env HOME="$h" MAJORDOMUS_GH="$g" "$@" \
      MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" 2>&1
}
: > "$GHS/calls"
out="$(prov_run "$T/prov-ok" "$GHS/gh-ok")" || { echo "    the install failed with a verifying CLI: $out"; exit 1; }
printf '%s\n' "$out" | grep -qF "provenance (built by github.com/$REPO) and checksum" \
  || { echo "    a verified attestation was not reported as provenance: $out"; exit 1; }
grep -qE "^attestation verify .*/$name --repo $REPO$" "$GHS/calls" \
  || { echo "    the CLI was not asked about the downloaded archive and this repository:"; cat "$GHS/calls"; exit 1; }
out="$(prov_run "$T/prov-refused" "$GHS/gh-refuses")" || { echo "    a refused attestation failed the default install: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "checksum only — provenance not verified: gh attestation verify refused it: Error: HTTP 404" \
  || { echo "    a refused attestation was not reported: $out"; exit 1; }
out="$(prov_run "$T/prov-signed-out" "$GHS/gh-signed-out")" || { echo "    a signed-out CLI failed the install: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "checksum only — provenance not verified: the GitHub CLI is not signed in" \
  || { echo "    a signed-out CLI was not reported: $out"; exit 1; }
out="$(prov_run "$T/prov-absent" "$T/no-such-gh")" || { echo "    an absent CLI failed the install: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "provenance not verified: the GitHub CLI ($T/no-such-gh) is not installed" \
  || { echo "    an absent CLI was not reported: $out"; exit 1; }
# ... and asked to require it, an install whose provenance is not verified installs nothing
out="$(prov_run "$T/prov-required" "$GHS/gh-refuses" MAJORDOMUS_REQUIRE_PROVENANCE=1)" && {
  echo "    MAJORDOMUS_REQUIRE_PROVENANCE=1 installed an archive whose provenance was refused"; exit 1
}
printf '%s\n' "$out" | grep -q "the archive's provenance could not be verified" \
  || { echo "    the required-provenance refusal did not say why: $out"; exit 1; }
[ ! -e "$T/prov-required/.local/bin/majordomus" ] || { echo "    a refused install left a launcher"; exit 1; }
out="$(prov_run "$T/prov-required-ok" "$GHS/gh-ok" MAJORDOMUS_REQUIRE_PROVENANCE=1)" \
  || { echo "    MAJORDOMUS_REQUIRE_PROVENANCE=1 refused a verified archive: $out"; exit 1; }

# --- nothing was left behind ------------------------------------------------------------------------------
find "$HOMEDIR/.local/share/majordomus" -maxdepth 1 -name '.staging.*' | grep -q . \
  && { echo "    a staging directory survived"; exit 1; }
find "$HOMEDIR/.local/share/majordomus" -maxdepth 1 -name '.lock' | grep -q . \
  && { echo "    the lock survived"; exit 1; }
exit 0
