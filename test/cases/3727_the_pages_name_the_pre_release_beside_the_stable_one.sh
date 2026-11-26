# majordomus-covers: none
# The pages name the newest pre-release beside the newest stable release, and the installer's
# default stays stable (I3449; the release seat's ruling of 2026-10-21). scripts/site-build
# derives the release the site offers once, into build.json's release block; beside it, it reads
# the highest pre-release that is not yanked and is ahead of the stable one, with its own notes
# and an install line that names it, from two readings of one fact (I4531): the release records
# of the tree and the pre-release documents the published branch already serves (releases/v*.json,
# which the release's publish job writes hours before the record lands; until 2026-11-06 the
# records alone were read, and every deploy of master between v1.0.0-alpha.10 and its record put
# alpha.9 back into the hero). /releases/latest.json and the installer's unasked default are not
# touched: cases 3222, 3179 and 3225 hold them.
#
# What is proved, over fixture records and fixture published documents: the offered release stays
# the stable one; the pre-release is the highest by its numeric identifiers (alpha.10 after
# alpha.2), a yanked one is skipped, and one behind the stable release (an rc of a version already
# released) is not offered; its install line asks for it by name; a published pre-release newer
# than every record wins, with its source and where it was read from named; the record wins over
# an older or equal publication; a published document whose record the tree yanked is withdrawn;
# a published pre-release behind the stable release is not offered; a malformed one is refused;
# with no pre-release ahead there is none, and with no record at all the published one is still
# named; and the homepage template names both channels, the pre-release under its own attribute
# so that every data-release still names the stable one.
. "$ROOT/test/lib.sh"
BUILD="$ROOT/scripts/site-build"
R="$PWD/records"; mkdir -p "$R"
P="$PWD/published"; mkdir -p "$P"
record() { # version channel [yanked]
  printf 'schema: release/v1\nversion: "%s"\ntag: v%s\nchannel: %s\npublished_at: "2026-10-21T12:00:00Z"\nnotes_url: https://example.test/notes/v%s\n' \
    "$1" "$1" "$2" "$1" > "$R/v$1.yaml"
  [ -z "${3:-}" ] || printf 'yanked: true\n' >> "$R/v$1.yaml"
}
published() { # version: a pre-release document as gh-pages serves it at releases/v<version>.json
  printf '{"schema":"release/v1","version":"%s","tag":"v%s","channel":"prerelease","published_at":"2026-11-06T17:09:16Z","notes_url":"https://example.test/notes/v%s","artifacts":{}}\n' \
    "$1" "$1" "$1" > "$P/v$1.json"
}
record 0.30.2 stable
record 0.30.0-rc.1 prerelease
record 1.0.0-alpha.2 prerelease
record 1.0.0-alpha.10 prerelease
record 1.0.0-alpha.11 prerelease yanked
printf '%s\n' '{"latest":{"tag":"v0.30.2","version":"0.30.2","published_at":"2026-10-19T07:24:08Z","notes_url":"https://example.test/notes/v0.30.2","url":"https://example.test/releases/v0.30.2.json"},"latest_url":"https://example.test/releases/latest.json","install_command":"curl -fsSL https://example.test/install.sh | sh"}' > dist.json
printf '%s\n' '{"schema":"release/v1","version":"0.30.2","tag":"v0.30.2","published_at":"2026-10-19T07:24:08Z","notes_url":"https://example.test/notes/v0.30.2"}' > published.json
offer() { RELEASE_ONLY=1 "$BUILD" --release-record "$PWD/dist.json" --release-records "$R" --release-published "$PWD/published.json" --release-published-dir "$P"; }
field() { printf '%s' "$out" | jq -r "$1"; }

# --- the stable release is offered, the newest pre-release ahead of it beside it
out="$(offer)" || { echo "    site-build could not derive the release"; exit 1; }
[ "$(field '.version + " " + .tag')" = "0.30.2 v0.30.2" ] \
  || { echo "    the offered release is not the stable one:"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.version')" = "1.0.0-alpha.10" ] \
  || { echo "    the pre-release is not the highest unyanked one ahead of stable (alpha.10):"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.notes_url')" = "https://example.test/notes/v1.0.0-alpha.10" ] \
  || { echo "    the pre-release does not carry its own notes"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.install_command')" = "curl -fsSL https://example.test/install.sh | sh -s -- --version v1.0.0-alpha.10" ] \
  || { echo "    the pre-release's install line does not ask for it by name:"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.source + " " + .prerelease.record + " " + (.prerelease.published // "none")')" = "record 1.0.0-alpha.10 none" ] \
  || { echo "    with nothing published the pre-release does not name the record as its source:"; printf '%s\n' "$out"; exit 1; }

# --- 2026-11-06: a pre-release published ahead of its record wins (I4531)
published 1.0.0-alpha.12
out="$(offer)"
[ "$(field '.prerelease.version + " " + .prerelease.source + " " + .prerelease.record + " " + .prerelease.published + " " + .prerelease.published_from')" = "1.0.0-alpha.12 published 1.0.0-alpha.10 1.0.0-alpha.12 dir:published" ] \
  || { echo "    a pre-release published ahead of its record is not the one named:"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.notes_url')" = "https://example.test/notes/v1.0.0-alpha.12" ] \
  || { echo "    the published pre-release does not carry its own notes"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.prerelease.install_command')" = "curl -fsSL https://example.test/install.sh | sh -s -- --version v1.0.0-alpha.12" ] \
  || { echo "    the published pre-release's install line does not ask for it by name:"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.version + " " + .source')" = "0.30.2 record" ] || { echo "    the stable release moved"; exit 1; }

# --- the record wins over an older or equal publication
rm "$P/v1.0.0-alpha.12.json"; published 1.0.0-alpha.3
out="$(offer)"
[ "$(field '.prerelease.version + " " + .prerelease.source + " " + .prerelease.published')" = "1.0.0-alpha.10 record 1.0.0-alpha.3" ] \
  || { echo "    a publication older than the record won:"; printf '%s\n' "$out"; exit 1; }
published 1.0.0-alpha.10
out="$(offer)"
[ "$(field '.prerelease.version + " " + .prerelease.source')" = "1.0.0-alpha.10 record" ] \
  || { echo "    the record does not win a tie with the same pre-release:"; printf '%s\n' "$out"; exit 1; }

# --- a published document whose record the tree yanked is withdrawn
published 1.0.0-alpha.11
out="$(offer)"
[ "$(field '.prerelease.version')" = "1.0.0-alpha.10" ] \
  || { echo "    a pre-release the tree yanked was named from the published branch:"; printf '%s\n' "$out"; exit 1; }

# --- a published pre-release behind the stable release is not offered
rm "$P"/v*.json; published 0.30.1-rc.1
out="$(offer)"
[ "$(field '.prerelease.version + " " + .prerelease.source + " " + (.prerelease.published // "none")')" = "1.0.0-alpha.10 record none" ] \
  || { echo "    a published rc of a version already released stable was offered:"; printf '%s\n' "$out"; exit 1; }

# --- a published document that does not name its own tag is refused
rm "$P"/v*.json
printf '{"schema":"release/v1","version":"9.9.9-rc.1","tag":"v9.9.9-rc.2","channel":"prerelease"}\n' > "$P/v9.9.9-rc.1.json"
expect_exit 10 offer
expect_grep 'dir:published serves a pre-release document \(v9.9.9-rc.2\) that is not a release/v1 document naming its own tag'
rm "$P"/v*.json

# --- no pre-release ahead of the stable one: none is offered; a published one is, with no record at all
rm "$R"/v1.0.0-*.yaml
out="$(offer)"
[ "$(field '.prerelease')" = null ] \
  || { echo "    a pre-release behind the stable release was offered:"; printf '%s\n' "$out"; exit 1; }
[ "$(field '.version')" = 0.30.2 ] || { echo "    the stable release moved"; exit 1; }
published 1.0.0-alpha.12
out="$(offer)"
[ "$(field '.prerelease.version + " " + .prerelease.source + " " + (.prerelease.record // "none")')" = "1.0.0-alpha.12 published none" ] \
  || { echo "    with no pre-release record at all the published one is not named:"; printf '%s\n' "$out"; exit 1; }

# --- the homepage names both channels; every data-release is still the stable one
T="$ROOT/site/templates/index.html"
grep -q 'data-prerelease="{{ offered.prerelease.version }}"' "$T" \
  || { echo "    the homepage does not name the pre-release"; exit 1; }
grep -q 'offered.prerelease.install_command' "$T" || { echo "    the homepage does not show the pre-release's install line"; exit 1; }
! grep -q 'data-release="{{ offered.prerelease' "$T" \
  || { echo "    the pre-release is marked data-release, which site-check holds to the stable one"; exit 1; }

# --- every page's bar names both, each with its role, and the footer names the source as the
# build's (I3449): the navbar and the footer are on every page, so these two partials are what
# "every page" means
N="$ROOT/site/templates/partials/navbar.html"
grep -q 'data-release="{{ offered.version }}" data-version-role="stable"' "$N" \
  || { echo "    the navbar's badge does not name the stable release as stable"; exit 1; }
grep -q 'data-prerelease="{{ offered.prerelease.version }}" data-version-role="prerelease"' "$N" \
  || { echo "    the navbar does not name the pre-release, as the pre-release, beside the stable release"; exit 1; }
grep -q '>pre-release v{{ offered.prerelease.version }}<' "$N" \
  || { echo "    the navbar's pre-release badge does not say it is the pre-release"; exit 1; }
F="$ROOT/site/templates/partials/footer.html"
grep -q 'Built from the source of {{ project.name }} <span[^>]*data-version-role="source" data-source-version="{{ source.source_version }}"' "$F" \
  || { echo "    the footer does not name its version as the source the build was made from"; exit 1; }
grep -q '{% if build %} at commit <a' "$F" || { echo "    the footer does not name the commit beside the source"; exit 1; }

# --- the gate (site-check --versions, check 17c) over fixture builds: the model is build.json's
CHECK="$ROOT/scripts/site-check"
model() { # stable prerelease|- source -> pub/build.json
  mkdir -p pub
  if [ "$2" = - ]; then pre=null; else pre="{\"version\":\"$2\"}"; fi
  printf '{"release":{"version":"%s","prerelease":%s},"source_version":"%s"}\n' "$1" "$pre" "$3" > pub/build.json
}
page() { mkdir -p "pub/$(dirname "$1")"; printf '%s\n' "$2" > "pub/$1"; }
bar_with_both='<span data-release="0.30.2" data-version-role="stable">v0.30.2</span><span data-prerelease="1.0.0-alpha.10" data-version-role="prerelease">pre-release v1.0.0-alpha.10</span>'
foot='<span data-version-role="source" data-source-version="1.0.0-alpha.10">v1.0.0-alpha.10</span>'
mkdir -p site/templates site/data/generated
printf '<p>{{ offered.version }}</p>\n{# 0.6.1 against 0.6.0, a comment #}\n' > site/templates/ok.html
printf '[a]\n# released as v0.9.0\nicon = "<path d=\\"M1.087.706 2.5.5\\"/>"\n' > site/data/kit.toml
printf '{"version":"9.9.9"}\n' > site/data/generated/release.json

# a pre-release newer than the offered release: every page names both in their roles, the
# footer the source, and a /ui snapshot the executable's build, which is of this source
model 0.30.2 1.0.0-alpha.10 1.0.0-alpha.10
page index.html "$bar_with_both $foot"
page docs/index.html "$bar_with_both $foot"
page ui/index.html '<span data-release="1.0.0-alpha.10" data-version-role="build">v1.0.0-alpha.10</span>'
expect_exit 0 "$CHECK" --versions pub pub/build.json site
expect_grep 'OK   versions .*\(3 pages, 7 versions named, 2 source files\)'

# a template carrying a hand-written v9.9.9 is refused, naming the file and the line
printf '<p>\n  <span>v9.9.9</span>\n</p>\n' > site/templates/hand.html
expect_exit 10 "$CHECK" --versions pub pub/build.json site
expect_grep 'FAIL versions +site/templates/hand.html:2: the version v9.9.9 is written by hand'
rm site/templates/hand.html

# a /ui snapshot that names a version build.json does not hold is refused, with the page
page ui/index.html '<span data-release="1.0.0-alpha.9" data-version-role="build">v1.0.0-alpha.9</span>'
expect_exit 10 "$CHECK" --versions pub pub/build.json site
expect_grep 'FAIL versions +/ui/index.html: names 1.0.0-alpha.9 as the build version and pub/build.json holds 1.0.0-alpha.10'
# and so is a badge with no role that names another version than the stable release
page ui/index.html '<span data-release="1.0.0-alpha.10">v1.0.0-alpha.10</span>'
expect_exit 10 "$CHECK" --versions pub pub/build.json site
expect_grep 'FAIL versions +/ui/index.html: names 1.0.0-alpha.10 as the stable version and pub/build.json holds 0.30.2'
rm pub/ui/index.html

# negative: no pre-release ahead of the offered release, one release version shown, passes;
# a page that still names a pre-release then is refused, because the model holds none
rm -r pub; model 0.30.2 - 0.30.3
page index.html '<span data-release="0.30.2" data-version-role="stable">v0.30.2</span><span data-version-role="source" data-source-version="0.30.3">v0.30.3</span>'
expect_exit 0 "$CHECK" --versions pub pub/build.json site
page docs/index.html "$bar_with_both"
expect_exit 10 "$CHECK" --versions pub pub/build.json site
expect_grep 'FAIL versions +/docs/index.html: names 1.0.0-alpha.10 as the prerelease version and pub/build.json holds no prerelease version'
rm pub/docs/index.html
# an unknown role is not a role
page docs/index.html '<span data-release="0.30.2" data-version-role="latest">v0.30.2</span>'
expect_exit 10 "$CHECK" --versions pub pub/build.json site
expect_grep 'data-version-role="latest" is not a role of the release model'

# --- the UI's bar, cut from the site's: the badge names the executable's build, and the
# pre-release, which the executable is not told, is not carried
if mj_runs node; then
  site_bar="<body><nav aria-label=\"Main\"><a href=\"https://x.test/\"><span class=\"b\" data-release=\"0.30.2\" data-version-role=\"stable\" title=\"The stable release the installer gives you\">v0.30.2</span>
      <span class=\"p\" data-prerelease=\"1.0.0-alpha.10\" data-version-role=\"prerelease\" title=\"t\">pre-release v1.0.0-alpha.10</span>
    </a></nav></body>"
  cut="$(SITE_BAR="$site_bar" node --input-type=module -e '
import { fragment } from "'"$ROOT"'/scripts/lib/ui-navbar.mjs";
process.stdout.write(fragment(process.env.SITE_BAR, "https://x.test"));')" \
    || { echo "    the UI's bar could not be cut from the site's"; exit 1; }
  printf '%s' "$cut" | grep -q 'data-release="@MJ_VERSION@" data-version-role="build" title="The version of the executable serving this page">v@MJ_VERSION@<' \
    || { echo "    the UI's badge does not name the executable's build:"; printf '%s\n' "$cut"; exit 1; }
  ! printf '%s' "$cut" | grep -q 'data-prerelease' \
    || { echo "    the UI's bar carries the site's pre-release badge:"; printf '%s\n' "$cut"; exit 1; }
  # a bar whose badge lost its role is refused rather than cut into a bar with no slot
  ! SITE_BAR='<body><nav><a href="https://x.test/"><span data-release="0.30.2">v0.30.2</span></a></nav></body>' \
    node --input-type=module -e '
import { fragment } from "'"$ROOT"'/scripts/lib/ui-navbar.mjs";
fragment(process.env.SITE_BAR, "https://x.test");' 2>/dev/null \
    || { echo "    a badge with no role was cut into a bar"; exit 1; }
fi
grep -q 'data-version-role="build"' "$ROOT/apps/majordomus-cli/src/ui/navbar.html" \
  || { echo "    the bar the executable compiles in does not name its badge as the build's (scripts/ui navbar --write)"; exit 1; }

echo "    release: the pages name the newest pre-release beside the stable one, recorded or published; the default stays stable"
echo "    versions: every surface names its versions from one model, each in its role, and a version typed by hand is refused"
