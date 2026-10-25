# majordomus-covers: none
# The pages name the newest pre-release beside the newest stable release, and the installer's
# default stays stable (I3449; the release seat's ruling of 2026-10-21). scripts/site-build
# derives the release the site offers once, into build.json's release block; beside it, it now
# reads the release records themselves for the highest pre-release that is not yanked and is ahead
# of the stable one, with its own notes and an install line that names it. /releases/latest.json
# and the installer's unasked default are not touched: cases 3222, 3179 and 3225 hold them.
#
# What is proved, over fixture records: the offered release stays the stable one; the
# pre-release is the highest by its numeric identifiers (alpha.10 after alpha.2), a yanked one is
# skipped, and one behind the stable release (an rc of a version already released) is not
# offered; its install line asks for it by name; with no pre-release ahead there is none; and the
# homepage template names both channels, the pre-release under its own attribute so that every
# data-release still names the stable one.
. "$ROOT/test/lib.sh"
BUILD="$ROOT/scripts/site-build"
R="$PWD/records"; mkdir -p "$R"
record() { # version channel [yanked]
  printf 'schema: release/v1\nversion: "%s"\ntag: v%s\nchannel: %s\npublished_at: "2026-10-21T12:00:00Z"\nnotes_url: https://example.test/notes/v%s\n' \
    "$1" "$1" "$2" "$1" > "$R/v$1.yaml"
  [ -z "${3:-}" ] || printf 'yanked: true\n' >> "$R/v$1.yaml"
}
record 0.30.2 stable
record 0.30.0-rc.1 prerelease
record 1.0.0-alpha.2 prerelease
record 1.0.0-alpha.10 prerelease
record 1.0.0-alpha.11 prerelease yanked
printf '%s\n' '{"latest":{"tag":"v0.30.2","version":"0.30.2","published_at":"2026-10-19T07:24:08Z","notes_url":"https://example.test/notes/v0.30.2","url":"https://example.test/releases/v0.30.2.json"},"latest_url":"https://example.test/releases/latest.json","install_command":"curl -fsSL https://example.test/install.sh | sh"}' > dist.json
printf '%s\n' '{"schema":"release/v1","version":"0.30.2","tag":"v0.30.2","published_at":"2026-10-19T07:24:08Z","notes_url":"https://example.test/notes/v0.30.2"}' > published.json
offer() { RELEASE_ONLY=1 "$BUILD" --release-record "$PWD/dist.json" --release-records "$R" --release-published "$PWD/published.json"; }

# --- the stable release is offered, the newest pre-release ahead of it beside it
out="$(offer)" || { echo "    site-build could not derive the release"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.version + " " + .tag')" = "0.30.2 v0.30.2" ] \
  || { echo "    the offered release is not the stable one:"; printf '%s\n' "$out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.prerelease.version')" = "1.0.0-alpha.10" ] \
  || { echo "    the pre-release is not the highest unyanked one ahead of stable (alpha.10):"; printf '%s\n' "$out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.prerelease.notes_url')" = "https://example.test/notes/v1.0.0-alpha.10" ] \
  || { echo "    the pre-release does not carry its own notes"; printf '%s\n' "$out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.prerelease.install_command')" = "curl -fsSL https://example.test/install.sh | sh -s -- --version v1.0.0-alpha.10" ] \
  || { echo "    the pre-release's install line does not ask for it by name:"; printf '%s\n' "$out"; exit 1; }

# --- no pre-release ahead of the stable one: none is offered
rm "$R"/v1.0.0-*.yaml
out="$(offer)"
[ "$(printf '%s' "$out" | jq -r '.prerelease')" = null ] \
  || { echo "    a pre-release behind the stable release was offered:"; printf '%s\n' "$out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r '.version')" = 0.30.2 ] || { echo "    the stable release moved"; exit 1; }

# --- the homepage names both channels; every data-release is still the stable one
T="$ROOT/site/templates/index.html"
grep -q 'data-prerelease="{{ offered.prerelease.version }}"' "$T" \
  || { echo "    the homepage does not name the pre-release"; exit 1; }
grep -q 'offered.prerelease.install_command' "$T" || { echo "    the homepage does not show the pre-release's install line"; exit 1; }
! grep -q 'data-release="{{ offered.prerelease' "$T" \
  || { echo "    the pre-release is marked data-release, which site-check holds to the stable one"; exit 1; }

echo "    release: the pages name the newest pre-release beside the stable one; the default stays stable"
