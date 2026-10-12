# majordomus-covers: none
# The site offers the release that was published, on every surface at once
# (project.the-site-offers-the-release-it-published).
#
# On 2026-10-12 v0.21.0 was on GitHub and in https://majordomus.dev/releases/latest.json from
# 10:23Z, while the homepage beside the install command offered v0.20.0 and linked its notes:
# the pages read the release record, which reaches master hours after the metadata does, and
# every check read one surface at a time. Held here, with no zola, no network and no token:
#
#   1. the derivation (scripts/site-build --release) picks the published release over an older
#      record, the record over an older or absent publication, copies the published bytes, reads
#      the published branch from git, and refuses a malformed or self-contradicting publication
#   2. every template that names the latest release reads that one derivation, and the site
#      data carries no release of its own
#   3. the live gate (scripts/pages release-live) over a file:// site and a gh on PATH: one
#      release passes; a fresh disagreement passes as in flight; each of the three surfaces
#      naming another release past the window fails; an offer ahead of GitHub fails at once; a
#      reading it cannot take is 12
#   4. the gate is wired: pages-check asks it, the pages workflow runs it after a deploy, the
#      model declares the window, and site-check refuses a build whose surfaces disagree
#   5. pages-check reads the newest deploy under a release-metadata commit instead of exit 12
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
command -v curl >/dev/null 2>&1 || skip "curl is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj1087.XXXXXX")"; trap 'rm -rf "$S"' EXIT
gitq() { git -c user.email=t@example.com -c user.name=t -c core.hooksPath=/dev/null "$@"; }

meta() {   # <version> [published_at]: a release/v1 document as the record projects it
  jq -n --arg v "$1" --arg at "${2:-2026-10-12T10:23:49Z}" \
    '{schema: "release/v1", version: $v, tag: ("v" + $v), channel: "stable", published_at: $at,
      notes_url: ("https://example.invalid/releases/tag/v" + $v), artifacts: {}}'
}
record() {   # <version|"">: the record's projection site-build reads (distribution.json)
  if [ -z "$1" ]; then jq -n '{latest_url: "https://example.invalid/releases/latest.json", latest: null}'
  else jq -n --arg v "$1" '{latest_url: "https://example.invalid/releases/latest.json",
    latest: {tag: ("v" + $v), version: $v, published_at: "2026-10-10T16:46:14Z",
             notes_url: ("https://example.invalid/releases/tag/v" + $v),
             url: ("https://example.invalid/releases/v" + $v + ".json")}}'; fi
}
offer() { "$ROOT/scripts/site-build" --release "$@"; }

# ---------------------------------------------------------------- 1. the one derivation
record 0.20.0 > "$S/dist.json"
meta 0.21.0 > "$S/published.json"
out="$(offer --release-record "$S/dist.json" --release-published "$S/published.json")"
[ "$(printf '%s' "$out" | jq -r '.version + " " + .source + " " + .record + " " + .published')" = "0.21.0 published 0.20.0 0.21.0" ] \
  || { echo "    a release published ahead of its record is not what the site offers:"; echo "$out"; exit 1; }
[ "$(printf '%s' "$out" | jq -r .url)" = "https://example.invalid/releases/v0.21.0.json" ] \
  || { echo "    the offered release's metadata URL is not derived from the model's: $(printf '%s' "$out" | jq -r .url)"; exit 1; }
echo "    published v0.21.0 over record v0.20.0: the site offers v0.21.0"

meta 0.19.1 > "$S/older.json"
out="$(offer --release-record "$S/dist.json" --release-published "$S/older.json")"
[ "$(printf '%s' "$out" | jq -r '.version + " " + .source')" = "0.20.0 record" ] \
  || { echo "    a publication older than the record won:"; echo "$out"; exit 1; }
meta 0.20.0 > "$S/same.json"
[ "$(offer --release-record "$S/dist.json" --release-published "$S/same.json" | jq -r .source)" = record ] \
  || { echo "    the record does not win a tie with the same release"; exit 1; }
# numeric, not lexical: 0.10.0 is newer than 0.9.0
record 0.9.0 > "$S/dist9.json"; meta 0.10.0 > "$S/ten.json"
[ "$(offer --release-record "$S/dist9.json" --release-published "$S/ten.json" | jq -r .version)" = 0.10.0 ] \
  || { echo "    versions compare as text: 0.9.0 beat 0.10.0"; exit 1; }
record "" > "$S/none.json"
[ "$(offer --release-record "$S/none.json" --release-published "$S/published.json" | jq -r .version)" = 0.21.0 ] \
  || { echo "    with no record at all the published release is not offered"; exit 1; }
[ "$(offer --release-record "$S/none.json" --release-ref refs/heads/mj1087-no-such-branch)" = null ] \
  || { echo "    with neither reading the site offers something"; exit 1; }
echo "    an older or equal publication leaves the record; numeric order; nothing offers null"

printf '{"schema":"release/v1","version":"0.21.0","tag":"v0.21.1"}\n' > "$S/bad.json"
expect_exit 10 offer --release-record "$S/dist.json" --release-published "$S/bad.json"
expect_grep 'not a release/v1 document naming its own tag'
echo "    a published document that does not name its own tag is refused (exit 10)"

# The published branch, read from git: a repository carrying site-build and a gh-pages branch.
R="$S/repo"; mkdir -p "$R/scripts" "$R/bin" "$R/site/data/registry"
cp "$ROOT/scripts/site-build" "$R/scripts/site-build"; cp -R "$ROOT/lib" "$R/lib"
record 0.20.0 > "$R/site/data/registry/distribution.json"
git init -q -b master "$R"; gitq -C "$R" add -A; gitq -C "$R" commit -qm base
gp="$S/gp"; git init -q -b gh-pages "$gp"; mkdir -p "$gp/releases"
meta 0.21.0 > "$gp/releases/latest.json"; meta 0.21.0 > "$gp/releases/v0.21.0.json"
gitq -C "$gp" add -A; gitq -C "$gp" commit -qm "release: v0.21.0 metadata, published ahead of its record"
published_branch() { git -C "$R" fetch -q "$gp" gh-pages && git -C "$R" update-ref refs/remotes/origin/gh-pages FETCH_HEAD; }
published_branch
out="$("$R/scripts/site-build" --release)"
[ "$(printf '%s' "$out" | jq -r .version)" = 0.21.0 ] || { echo "    origin/gh-pages was not read:"; echo "$out"; exit 1; }
printf '%s' "$out" | jq -e --arg c "$(git -C "$gp" rev-parse HEAD)" '.published_from == ("origin/gh-pages@" + $c)' >/dev/null \
  || { echo "    the build does not name the published commit it read: $(printf '%s' "$out" | jq -r .published_from)"; exit 1; }
meta 0.21.0 2026-10-12T11:00:00Z > "$gp/releases/v0.21.0.json"
gitq -C "$gp" commit -qam "a tag file that differs"
published_branch
expect_exit 10 "$R/scripts/site-build" --release
expect_grep 'releases/latest.json and releases/v0.21.0.json that differ'
echo "    the published branch is read from git, named by its commit, and a stale tag file refused"

# ---------------------------------------------------------------- 2. every surface reads it
TPL="$ROOT/site/templates"
grep -q 'set offered = build.release if build and build.release else false' "$TPL/base.html" \
  || { echo "    base.html does not derive \`offered\` from build.json's release"; exit 1; }
left="$(grep -rnE 'project\.release|dist\.latest' "$TPL" || true)"
[ -z "$left" ] || { echo "    a template still reads another source for the latest release:"; echo "$left"; exit 1; }
grep -q 'data-release="{{ offered.version }}">v{{ offered.version }}' "$TPL/partials/navbar.html" \
  || { echo "    the navbar badge does not read offered, or does not state it as data-release"; exit 1; }
grep -q 'hover:underline" data-release="{{ offered.version }}">{{ offered.version }}</a><span aria-hidden' "$TPL/index.html" \
  || { echo "    the hero badge does not state the offer as data-release"; exit 1; }
# No surface chooses "the newest record" by listing files: the highest file name is not the
# latest release the day a prerelease or a withdrawn record tops it (PR #918's finding). The
# record's reading is `latest` in distribution.json, which is Releases::latest_stable.
listed="$(grep -nE 'ls[^|#]*\.ai/repo/releases' "$ROOT/scripts/generate-site-data" "$ROOT/scripts/site-build" "$ROOT/scripts/site-check" || true)"
[ -z "$listed" ] || { echo "    a script lists the release records to choose a version:"; echo "$listed"; exit 1; }
grep -q 'data-trust-key="release"{% if offered %} data-release="{{ offered.version }}"' "$TPL/index.html" \
  || { echo "    the homepage's trust strip does not state the offer as data-release"; exit 1; }
grep -q 'release: \$release' "$ROOT/scripts/generate-site-data" \
  && { echo "    generate-site-data still writes a release of its own into project.json"; exit 1; }
jq -e 'has("release") | not' "$ROOT/site/data/generated/project.json" >/dev/null \
  || { echo "    the committed project.json still carries a release"; exit 1; }
grep -q 'release: \$release\[0\]' "$ROOT/scripts/site-build" \
  || { echo "    site-build does not write the derived release into build.json"; exit 1; }
echo "    every template reads the one derivation; the site data carries no release of its own"

# ---------------------------------------------------------------- 3. the live gate
W="$S/site"; mkdir -p "$W/releases" "$S/bin"
iso() { jq -rn --argjson s "$1" '(now - $s) | floor | todate'; }
site() {   # <homepage> <build.json> <latest.json>: a published site as files
  printf '<html><dl data-trust><div class="p-4" data-trust-key="release" data-release="%s"><dt>latest release</dt><dd><a href="#">%s</a></dd></div></dl></html>\n' "$1" "$1" > "$W/index.html"
  jq -n --arg v "$2" '{schema: 1, commit: "abc", release: {version: $v, source: "published"}}' > "$W/build.json"
  meta "$3" > "$W/releases/latest.json"
}
github() {   # <version> <seconds ago> | fail
  if [ "$1" = fail ]; then printf '#!/bin/sh\necho "HTTP 401" >&2; exit 1\n' > "$S/bin/gh"
  else printf '#!/bin/sh\nprintf "v%s\\t%s\\n"\n' "$1" "$(iso "$2")" > "$S/bin/gh"; fi
  chmod +x "$S/bin/gh"
}
live() { PATH="$S/bin:$PATH" "$ROOT/scripts/pages" release-live --url "file://$W" --window 1800 --repo o/r "$@"; }

site 0.21.0 0.21.0 0.21.0; github 0.21.0 7200
expect_exit 0 live; expect_grep 'name one release, v0.21.0'
echo "    one release on the homepage, latest.json and GitHub: ok"

# 2026-10-12 itself: the homepage behind the metadata and GitHub
site 0.20.0 0.20.0 0.21.0; github 0.21.0 60
expect_exit 0 live; expect_grep 'in flight: GitHub published v0.21.0 [0-9]+s ago, inside the 1800s window'
github 0.21.0 7200
expect_exit 10 live; expect_grep 'beyond the 1800s window, and the homepage offers 0.20.0 while /releases/latest.json names 0.21.0'
echo "    the homepage behind: in flight at one minute, refused at two hours"

site 0.21.0 0.21.0 0.20.0; github 0.21.0 7200
expect_exit 10 live; expect_grep 'latest.json names 0.20.0'
echo "    latest.json behind past the window: refused"

site 0.20.0 0.20.0 0.20.0; github 0.21.0 7200
expect_exit 10 live; expect_grep 'GitHub published v0.21.0'
echo "    GitHub ahead of both past the window: refused"

site 0.22.0 0.22.0 0.21.0; github 0.21.0 5
expect_exit 10 live; expect_grep 'offers 0.22.0, which GitHub has not published'
echo "    an offer ahead of GitHub is refused at once, window or not"

site 0.21.0 0.20.0 0.21.0; github 0.21.0 7200
expect_exit 10 live; expect_grep 'the pages and the build that made them disagree'
echo "    a homepage that disagrees with its own build.json: refused"

site 0.21.0 0.21.0 0.21.0; github 0.21.0 7200
expect_exit 10 live --expect v0.22.0; expect_grep 'not v0.22.0'
github fail
expect_exit 12 live; expect_grep "NOT DETERMINED .*GitHub's newest release"
echo "    --expect pins GitHub's release; an unreadable GitHub is 12, never a pass"

# a homepage built before the attribute existed is still read, from its trust card
printf '<div class="p-4" data-trust-key="release"><dt>latest release</dt>\n<dd><a href="x">0.20.0</a></dd></div>\n' > "$W/index.html"
jq -n '{schema: 1, commit: "abc"}' > "$W/build.json"
site_latest() { meta "$1" > "$W/releases/latest.json"; }; site_latest 0.21.0; github 0.21.0 7200
expect_exit 10 live; expect_grep 'homepage offers +0.20.0'
echo "    a page from before the attribute is judged from its trust card, not skipped"

# ---------------------------------------------------------------- 4. wiring
grep -q 'scripts/pages release-live --url "\$URL" --window "\$OWED_AFTER"' "$ROOT/scripts/ci/pages-check" \
  || { echo "    pages-check (the pages-live gate, asked at finish) does not ask release-live"; exit 1; }
awk '/- name: the release it offers/{f=1} f && /run: scripts\/pages release-live/{print "wired"; exit}' "$ROOT/.github/workflows/pages.yml" | grep -q wired \
  || { echo "    the pages workflow does not run release-live after a deploy"; exit 1; }
grep -qE '^  release_window: [0-9]+$' "$ROOT/.ai/repo/ci/pages.yaml" \
  || { echo "    .ai/repo/ci/pages.yaml declares no deploy.release_window"; exit 1; }
grep -q 'a page reads another source than build.json' "$ROOT/scripts/site-check" \
  || { echo "    site-check does not refuse a build whose surfaces disagree"; exit 1; }
grep -q 'as the latest and site/static/releases/latest.json names' "$ROOT/scripts/site-check" \
  || { echo "    site-check does not hold the two committed projections of the latest release together"; exit 1; }
grep -q 'a badge reads another source' "$ROOT/scripts/site-check" \
  || { echo "    site-check does not hold every data-release on the homepage to the one offer"; exit 1; }
echo "    pages-check, the pages workflow, the model and site-check carry it"

# ---------------------------------------------------------------- 5. pages-check under a release
P="$S/pc"; mkdir -p "$P/scripts/ci" "$P/site"
cp "$ROOT/scripts/ci/pages-check" "$P/scripts/ci/pages-check"; printf 'x\n' > "$P/site/index.md"
git init -q -b master "$P"; gitq -C "$P" add -A; gitq -C "$P" commit -qm base
pub="$(git -C "$P" rev-parse HEAD)"
empty="$(git -C "$P" hash-object -t tree /dev/null)"
d="$(printf 'deploy: site from %s\n' "$pub" | gitq -C "$P" commit-tree "$empty")"
r="$(printf 'release: v0.21.0 metadata, published ahead of its record\n' | gitq -C "$P" commit-tree "$empty" -p "$d")"
git -C "$P" update-ref refs/heads/gh-pages "$r"
git -C "$P" remote add origin "$P"; git -C "$P" fetch -q origin 2>/dev/null || true
pc() { ( cd "$P" && scripts/ci/pages-check "$@" ); }
expect_exit 0 pc --offline
expect_grep 'the head of gh-pages is release metadata'
expect_grep "the published commit $pub is on master"
echo "    a release-metadata commit on top of gh-pages is read through, not exit 12"

echo "    the site offers the release it published"
