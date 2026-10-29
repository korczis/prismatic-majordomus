# majordomus-covers: none
# The installer installs a pre-release only when it is asked for it by name (I2829). The latest
# release is never a pre-release, so a latest.json that resolves to one is refused before
# anything is downloaded, while `--version v1.0.0-alpha.1` resolves the same metadata from the
# pre-release's own tag file. site/static/install.sh against a local origin, in --dry-run, so
# nothing is downloaded and no executable is needed. The metadata is this repository's own
# published v0.28.0 document with the version rewritten, so its shape is the real one.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || skip "no curl or wget"
SRC="$ROOT/site/static/releases/v0.28.0.json"
expect_file "$SRC"

FIX="$T/origin"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture origin"
trap 'stop_http' EXIT INT TERM HUP
sed 's/0\.28\.0/1.0.0-alpha.1/g' "$SRC" > "$FIX/releases/v1.0.0-alpha.1.json"
grep -q '"tag": "v1.0.0-alpha.1"' "$FIX/releases/v1.0.0-alpha.1.json" || { echo "    the fixture metadata was not rewritten"; exit 1; }

install_run() {
  env HOME="$T/home" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
    sh "$ROOT/site/static/install.sh" --dry-run "$@" 2>&1
}

# ---------------------------------------------------------------- 1. unpinned, a pre-release is refused
cp "$FIX/releases/v1.0.0-alpha.1.json" "$FIX/releases/latest.json"
out="$(install_run)" && { echo "    the installer resolved a pre-release it was not asked for:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q "it resolves to the pre-release v1.0.0-alpha.1, and the latest release is never a pre-release" \
  || { echo "    the refusal does not say why:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q -- "--version v1.0.0-alpha.1" || { echo "    the refusal does not say how to ask for it"; exit 1; }
printf '%s\n' "$out" | grep -q "resolved release" && { echo "    the refused resolution went on to plan an install"; exit 1; }

# ---------------------------------------------------------------- 2. by name, it is resolved
rm "$FIX/releases/latest.json"
out="$(install_run --version v1.0.0-alpha.1)" || { echo "    the installer refused a pre-release asked for by name:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q "resolved release    v1.0.0-alpha.1 (pinned)" \
  || { echo "    the pinned pre-release was not resolved:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
echo "    a latest.json that resolves to a pre-release is refused before anything is downloaded; the same pre-release asked for by name is resolved"
