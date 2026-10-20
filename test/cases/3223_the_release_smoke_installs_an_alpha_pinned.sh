# majordomus-covers: none
# The release smoke of a pre-release reads the pre-release's own metadata, installs it pinned,
# and refuses a site whose latest release is the pre-release (I2829). scripts/ci/install-check
# against a local origin: latest.json names a stable release, releases/v1.0.0-alpha.1.json the
# pre-release, and the served install.sh is a stub that records the version it was asked for
# and stops, so nothing is downloaded and no executable is needed.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/install-check"
command -v curl >/dev/null 2>&1 || skip "no curl"

FIX="$T/origin"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture origin"
trap 'stop_http' EXIT INT TERM HUP

meta() { printf '{\n  "schema": "release/v1",\n  "version": "%s",\n  "tag": "v%s"\n}\n' "$1" "$1"; }
meta 0.29.0 > "$FIX/releases/latest.json"
meta 0.29.0 > "$FIX/releases/v0.29.0.json"
meta 1.0.0-alpha.1 > "$FIX/releases/v1.0.0-alpha.1.json"
cat > "$FIX/install.sh" <<STUB
#!/bin/sh
printf '%s\n' "\${MAJORDOMUS_VERSION:-unpinned}" > "$T/asked"
echo "stub installer: stopped before downloading" >&2
exit 3
STUB

gate() { rm -f "$T/asked"; env HOME="$T/home" "$GATE" --base "$HTTP_BASE" "$@" 2>&1; }

# ---------------------------------------------------------------- 1. a pre-release is installed pinned
out="$(gate --expect v1.0.0-alpha.1)" && { echo "    the stub installer cannot succeed, yet the gate passed"; exit 1; }
printf '%s\n' "$out" | grep -q "metadata  $HTTP_BASE/releases/v1.0.0-alpha.1.json" \
  || { echo "    the gate did not read the pre-release's own metadata:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q "OK   install-check  the metadata names v1.0.0-alpha.1" || { echo "    the pre-release was not resolved"; exit 1; }
printf '%s\n' "$out" | grep -q "OK   install-check  the latest release is v0.29.0, not the pre-release v1.0.0-alpha.1" \
  || { echo "    the gate did not check that the latest release is not the pre-release"; exit 1; }
[ "$(cat "$T/asked" 2>/dev/null)" = v1.0.0-alpha.1 ] \
  || { echo "    the advertised command was not run pinned to the pre-release (asked: $(cat "$T/asked" 2>/dev/null || echo nothing))"; exit 1; }

# ---------------------------------------------------------------- 2. a site whose latest is the pre-release is refused
cp "$FIX/releases/v1.0.0-alpha.1.json" "$FIX/releases/latest.json"
out="$(gate --expect v1.0.0-alpha.1)" && { echo "    the gate passed a site whose latest release is the pre-release"; exit 1; }
printf '%s\n' "$out" | grep -q "FAIL install-check  $HTTP_BASE/releases/latest.json names the pre-release v1.0.0-alpha.1; a pre-release is never the latest release" \
  || { echo "    the refusal does not say why:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
[ ! -e "$T/asked" ] || { echo "    the installer ran although the site was refused"; exit 1; }

# The latest pointer must also reject a pre-release other than the one being checked.
meta 1.0.0-alpha.2 > "$FIX/releases/v1.0.0-alpha.2.json"
out="$(gate --expect v1.0.0-alpha.2)" && { echo "    the gate passed a site whose latest is another pre-release"; exit 1; }
printf '%s\n' "$out" | grep -q "FAIL install-check  $HTTP_BASE/releases/latest.json names the pre-release v1.0.0-alpha.1; a pre-release is never the latest release" \
  || { echo "    the other pre-release was accepted as latest:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
[ ! -e "$T/asked" ] || { echo "    the installer ran with another pre-release as latest"; exit 1; }

# ---------------------------------------------------------------- 3. a stable release is checked as before
meta 0.29.0 > "$FIX/releases/latest.json"
out="$(gate --expect v0.29.0)" && { echo "    the stub installer cannot succeed, yet the gate passed"; exit 1; }
printf '%s\n' "$out" | grep -q "metadata  $HTTP_BASE/releases/latest.json" || { echo "    a stable release was not read from latest.json"; exit 1; }
[ "$(cat "$T/asked" 2>/dev/null)" = unpinned ] || { echo "    a stable release was installed pinned (asked: $(cat "$T/asked" 2>/dev/null))"; exit 1; }
out="$(gate --expect v0.30.0)" && { echo "    a release the metadata does not name passed"; exit 1; }
printf '%s\n' "$out" | grep -q "the metadata names v0.29.0, not the release this run was asked to check (v0.30.0)" || { echo "    the stable mismatch is not named"; exit 1; }
echo "    a pre-release is read from its own metadata, installed pinned, and refused when it is the latest release; a stable release is checked through latest.json as before"
