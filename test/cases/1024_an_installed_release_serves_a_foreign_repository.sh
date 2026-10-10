# An installed release is proven to serve, not only to print its version (I2162). The install
# gate, pointed at a local fixture release, installs it, starts the installed MCP launcher in
# a fresh repository that is not this one — without --standalone, so it elects itself that
# repository's shared server — and reads back over stdio and HTTP that it answered
# initialize and tools/list, that the server is ready, and that the repository it names is
# the one it was started in. Then the same gate against an archive whose launcher cannot
# start must fail and say so, and the release smoke must run this gate on the published tag
# with no step that swallows a failure.
#
# Nothing here reaches the internet: the fixture release and the origin are local.
# claims: advertised-install-command-works
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/install-check"
expect_file "$GATE"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v mkfifo >/dev/null 2>&1 || skip "no mkfifo"

FIX="$T/fixture"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture release"
trap 'stop_http' EXIT INT TERM HUP

VERSION="$("$ROOT/scripts/release-version")"
MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-fixture" --out "$FIX" --base-url "$HTTP_BASE" > "$T/fixture.log" 2>&1 \
  || { rc=$?; echo "    the fixture release could not be built (exit $rc)"; tail -20 "$T/fixture.log" | sed 's/^/      /'; exit 1; }
expect_file "$FIX/releases/latest.json"

# --- a working release serves a repository of its own ----------------------------------
out="$(env HOME="$T/home-green" "$GATE" --base "$HTTP_BASE" --expect "v$VERSION" 2>&1)" || {
  echo "    the gate failed against a complete release"
  printf '%s\n' "$out" | sed 's/^/      /'
  exit 1
}
line="$(printf '%s\n' "$out" | grep 'the installed release serves a foreign repository over the shared server' || true)"
[ -n "$line" ] || { echo "    the gate did not start a shared server from the installed release:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
served="$(printf '%s' "$line" | sed -n 's/.*(\([0-9a-f]\{32\}\)).*/\1/p')"
[ -n "$served" ] || { echo "    the gate did not report the repository id the server named: $line"; exit 1; }
own="$(printf '%s' "$(git -C "$ROOT" rev-parse --show-toplevel)" | mj_sha256sum | cut -c1-32)"
[ "$served" != "$own" ] || { echo "    the installed server named this repository ($own), not a foreign one"; exit 1; }

# --- the release it was asked about, and no other -----------------------------------------
out="$(env HOME="$T/home-other" "$GATE" --base "$HTTP_BASE" --expect "v0.0.1" 2>&1)" && {
  echo "    the gate accepted a release other than the one it was asked to check"; exit 1
}
printf '%s\n' "$out" | grep -q "not the release this run was asked to check (v0.0.1)" \
  || { echo "    the gate did not name the release it expected: $out"; exit 1; }

# --- an archive whose launcher cannot start fails the gate ---------------------------------
# Every archive of the fixture is a copy of this machine's; each is repacked with an MCP
# launcher that exits non-zero, and the metadata is told the new digests, so the installer
# accepts the archive and the failure is the launcher's alone.
broken="$T/broken"; mkdir -p "$broken"
for archive in "$FIX"/majordomus-*.tar.gz; do
  name="$(basename "$archive")"
  # the fixture's archives are copies of this machine's, so the directory inside is named
  # for this machine's target, not for the file; it is read from the archive
  top="$(tar -tzf "$archive" | sed -n '1s#/.*##p')"
  [ -n "$top" ] || { echo "    $name holds no top directory"; exit 1; }
  rm -rf "${broken:?}/$top"; tar -xzf "$archive" -C "$broken"
  expect_file "$broken/$top/bin/majordomus-mcp"
  printf '#!/bin/sh\necho "this launcher cannot start" >&2\nexit 3\n' > "$broken/$top/bin/majordomus-mcp"
  chmod 0755 "$broken/$top/bin/majordomus-mcp"
  ( cd "$broken" && tar -czf "$archive" "$top" )
  sha="$(mj_sha256sum < "$archive" | cut -d' ' -f1)"
  size="$(wc -c < "$archive" | tr -d ' ')"
  # the installer reads the metadata line by line, as published: the digest and the size
  # are replaced in place, and the document keeps the shape the site serves
  for meta in "$FIX"/releases/*.json; do
    sed "/\"name\": \"$name\"/{
s/\"sha256\": \"[0-9a-f]*\"/\"sha256\": \"$sha\"/
s/\"size\": [0-9]*/\"size\": $size/
}" "$meta" > "$meta.new" && mv "$meta.new" "$meta"
    [ "$(jq -r --arg n "$name" '.artifacts[] | select(.name == $n) | "\(.sha256) \(.size)"' "$meta")" = "$sha $size" ] \
      || { echo "    $meta could not be told the repacked digest of $name"; exit 1; }
  done
done
out="$(env HOME="$T/home-broken" "$GATE" --base "$HTTP_BASE" --expect "v$VERSION" 2>&1)" && {
  echo "    the gate passed a release whose MCP launcher cannot start"
  printf '%s\n' "$out" | sed 's/^/      /'
  exit 1
}
printf '%s\n' "$out" | grep -q "installed v$VERSION from the published archive" \
  || { echo "    the broken archive was not installed, so the launcher was never the failure: $out"; exit 1; }
printf '%s\n' "$out" | grep -q "did not start a shared server" \
  || { echo "    the gate did not say the installed launcher failed to serve: $out"; exit 1; }

# --- the release smoke runs this gate on the tag it published, and swallows nothing --------
W="$ROOT/.github/workflows/release.yml"
smoke="$(awk '/^  smoke:$/{f=1} f&&/^  [a-z][a-z0-9_-]*:$/&&!/^  smoke:$/{exit} f' "$W")"
[ -n "$smoke" ] || { echo "    release.yml has no smoke job"; exit 1; }
printf '%s\n' "$smoke" | grep -q 'scripts/ci/install-check .*--expect' \
  || { echo "    the release smoke does not run the install gate on the tag it published"; exit 1; }
if printf '%s\n' "$smoke" | grep -nE '\|\| *true' ; then
  echo "    a step of the release smoke swallows its failure (|| true)"; exit 1
fi

echo "    an installed release serves a foreign repository, and a launcher that cannot start fails the gate and the smoke"
